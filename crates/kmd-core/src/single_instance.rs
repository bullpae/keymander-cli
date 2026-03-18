//! Single-instance guard with toggle behavior.
//!
//! When `kmd` starts:
//!   1. Check for `kmd.lock` — if another instance is alive, signal it to quit
//!   2. The *new* process creates a `kmd.quit` signal file, waits briefly, then exits
//!   3. The *existing* process detects `kmd.quit` in its event loop and exits gracefully
//!
//! On Unix, `flock(2)` is used for reliable instance detection.
//! PID-based checks alone are unreliable because zombie processes and PID
//! recycling cause false positives on macOS/Linux.

use std::fs;
use std::path::{Path, PathBuf};

// ── Lock file name constants ─────────────────────────────────────────────────

const LOCK_FILE: &str = "kmd.lock";
const QUIT_SIGNAL: &str = "kmd.quit";

/// How many iterations to wait for the other instance to exit.
const TOGGLE_WAIT_ITERATIONS: u32 = 40;
/// Milliseconds to sleep between each poll.
const TOGGLE_WAIT_MS: u64 = 50;
/// Ignore ultra-fast repeated launches to avoid accidental immediate toggle-off.
const RECENT_LOCK_DEBOUNCE_MS: u64 = 700;

/// Result of attempting to acquire the single-instance lock.
pub enum InstanceAction {
    /// No other instance was running. The `Guard` holds the lock;
    /// dropping it cleans up the lock file.
    Acquired(Guard),
    /// Another instance was found and signalled to quit.
    /// The caller should exit silently.
    SignalledExisting,
}

/// RAII guard — removes the lock file when dropped.
/// On Unix, holds an open file descriptor with `flock` to guarantee
/// the OS releases the lock even on crash/SIGKILL.
pub struct Guard {
    lock_path: PathBuf,
    quit_signal_path: PathBuf,
    _lock_file: Option<fs::File>,
}

impl Guard {
    /// Check whether an external process has requested us to quit.
    /// Call this on every tick in the event loop.
    pub fn should_quit(&self) -> bool {
        self.quit_signal_path.exists()
    }

    /// Consume the quit signal (delete the file).
    pub fn consume_quit_signal(&self) {
        let _ = fs::remove_file(&self.quit_signal_path);
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        // lock file 삭제 (flock은 _lock_file Drop 시 자동 해제)
        let _ = fs::remove_file(&self.lock_path);
        let _ = fs::remove_file(&self.quit_signal_path);
    }
}

/// Resolve the lock and signal file paths from the data directory.
pub fn lock_paths(data_dir: &Path) -> (PathBuf, PathBuf) {
    (data_dir.join(LOCK_FILE), data_dir.join(QUIT_SIGNAL))
}

/// Try to become the single instance, or signal the existing one to quit.
///
/// `data_dir` is the kmd data directory (e.g. `~/.local/share/kmd`).
pub fn acquire_or_toggle(data_dir: &Path) -> InstanceAction {
    let _ = fs::create_dir_all(data_dir);

    let (lock_path, quit_signal_path) = lock_paths(data_dir);

    // Clean up any stale quit signal from a previous crash
    let _ = fs::remove_file(&quit_signal_path);

    #[cfg(unix)]
    {
        if let Some(action) = acquire_or_toggle_flock(&lock_path, &quit_signal_path) {
            return action;
        }
        // flock 실패 시 PID 기반 폴백
    }

    acquire_or_toggle_pid(&lock_path, &quit_signal_path)
}

// ── Unix: flock 기반 (좀비/PID 재활용에 안전) ────────────────────────────────

#[cfg(unix)]
fn try_flock_nonblocking(file: &fs::File) -> bool {
    use std::os::unix::io::AsRawFd;
    unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) == 0 }
}

#[cfg(unix)]
fn acquire_or_toggle_flock(
    lock_path: &PathBuf,
    quit_signal_path: &PathBuf,
) -> Option<InstanceAction> {
    use std::io::{Seek, Write};

    let lock_file = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(lock_path)
        .ok()?;

    if try_flock_nonblocking(&lock_file) {
        // flock 획득 → 다른 인스턴스 없음 (또는 이전 인스턴스 크래시)
        let mut f = &lock_file;
        let _ = lock_file.set_len(0);
        let _ = f.seek(std::io::SeekFrom::Start(0));
        let _ = write!(f, "{}", std::process::id());
        return Some(InstanceAction::Acquired(Guard {
            lock_path: lock_path.clone(),
            quit_signal_path: quit_signal_path.clone(),
            _lock_file: Some(lock_file),
        }));
    }

    // flock 실패 → 다른 인스턴스가 확실히 살아있음
    if is_recent_lock(lock_path, RECENT_LOCK_DEBOUNCE_MS) {
        return Some(InstanceAction::SignalledExisting);
    }

    let _ = fs::write(quit_signal_path, "quit");

    for _ in 0..TOGGLE_WAIT_ITERATIONS {
        std::thread::sleep(std::time::Duration::from_millis(TOGGLE_WAIT_MS));
        if try_flock_nonblocking(&lock_file) {
            // 상대방이 종료됨 → flock 해제 후 정리
            use std::os::unix::io::AsRawFd;
            unsafe { libc::flock(lock_file.as_raw_fd(), libc::LOCK_UN) };
            let _ = fs::remove_file(lock_path);
            break;
        }
    }

    Some(InstanceAction::SignalledExisting)
}

// ── PID 기반 (Windows / Unix 폴백) ──────────────────────────────────────────

fn acquire_or_toggle_pid(lock_path: &PathBuf, quit_signal_path: &PathBuf) -> InstanceAction {
    if let Ok(contents) = fs::read_to_string(lock_path) {
        if let Ok(pid) = contents.trim().parse::<u32>() {
            if pid != std::process::id() && is_process_alive(pid) {
                if is_recent_lock(lock_path, RECENT_LOCK_DEBOUNCE_MS) {
                    return InstanceAction::SignalledExisting;
                }

                let _ = fs::write(quit_signal_path, "quit");

                for _ in 0..TOGGLE_WAIT_ITERATIONS {
                    std::thread::sleep(std::time::Duration::from_millis(TOGGLE_WAIT_MS));
                    if !is_process_alive(pid) {
                        break;
                    }
                }

                if !is_process_alive(pid) {
                    let _ = fs::remove_file(lock_path);
                }

                return InstanceAction::SignalledExisting;
            }
        }
        let _ = fs::remove_file(lock_path);
    }

    let our_pid = std::process::id();
    let _ = fs::write(lock_path, our_pid.to_string());

    InstanceAction::Acquired(Guard {
        lock_path: lock_path.clone(),
        quit_signal_path: quit_signal_path.clone(),
        _lock_file: None,
    })
}

fn is_recent_lock(lock_path: &Path, threshold_ms: u64) -> bool {
    let Ok(meta) = fs::metadata(lock_path) else {
        return false;
    };
    let Ok(modified) = meta.modified() else {
        return false;
    };
    let Ok(elapsed) = modified.elapsed() else {
        return false;
    };
    elapsed.as_millis() <= threshold_ms as u128
}

// ── Platform-specific process helpers ────────────────────────────────────────

#[cfg(windows)]
fn is_process_alive(pid: u32) -> bool {
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const STILL_ACTIVE: u32 = 259;

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }
        let mut exit_code: u32 = 0;
        let ok = GetExitCodeProcess(handle, &mut exit_code);
        CloseHandle(handle);
        ok != 0 && exit_code == STILL_ACTIVE
    }
}

#[cfg(windows)]
unsafe extern "system" {
    fn OpenProcess(
        desired_access: u32,
        inherit_handles: i32,
        process_id: u32,
    ) -> *mut std::ffi::c_void;
    fn GetExitCodeProcess(process: *mut std::ffi::c_void, exit_code: *mut u32) -> i32;
    fn CloseHandle(object: *mut std::ffi::c_void) -> i32;
}

#[cfg(unix)]
fn is_process_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_acquire_new_instance() {
        let dir = std::env::temp_dir().join("kmd_test_single_instance_v2");
        let _ = fs::create_dir_all(&dir);
        let _ = fs::remove_file(dir.join(LOCK_FILE));
        let _ = fs::remove_file(dir.join(QUIT_SIGNAL));

        match acquire_or_toggle(&dir) {
            InstanceAction::Acquired(guard) => {
                let contents = fs::read_to_string(&guard.lock_path).unwrap();
                assert_eq!(contents, std::process::id().to_string());

                assert!(!guard.should_quit());

                drop(guard);
                assert!(!dir.join(LOCK_FILE).exists());
            }
            InstanceAction::SignalledExisting => panic!("Should have acquired, not signalled"),
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_stale_lock_is_replaced() {
        let dir = std::env::temp_dir().join("kmd_test_stale_lock_v2");
        let _ = fs::create_dir_all(&dir);
        let _ = fs::remove_file(dir.join(QUIT_SIGNAL));

        // 존재하지 않는 PID
        fs::write(dir.join(LOCK_FILE), "99999999").unwrap();

        match acquire_or_toggle(&dir) {
            InstanceAction::Acquired(guard) => {
                let contents = fs::read_to_string(&guard.lock_path).unwrap();
                assert_eq!(contents, std::process::id().to_string());
                drop(guard);
            }
            InstanceAction::SignalledExisting => panic!("Stale PID should not be signalled"),
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_quit_signal_detection() {
        let dir = std::env::temp_dir().join("kmd_test_quit_signal");
        let _ = fs::create_dir_all(&dir);
        let _ = fs::remove_file(dir.join(LOCK_FILE));
        let _ = fs::remove_file(dir.join(QUIT_SIGNAL));

        match acquire_or_toggle(&dir) {
            InstanceAction::Acquired(guard) => {
                assert!(!guard.should_quit());

                fs::write(&guard.quit_signal_path, "quit").unwrap();
                assert!(guard.should_quit());

                guard.consume_quit_signal();
                assert!(!guard.should_quit());

                drop(guard);
            }
            InstanceAction::SignalledExisting => panic!("Should have acquired"),
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_is_recent_lock_fresh_file_returns_true() {
        let dir = std::env::temp_dir().join("kmd_test_recent_lock_fresh");
        let _ = fs::create_dir_all(&dir);
        let lock = dir.join("fresh.lock");
        fs::write(&lock, "test").unwrap();

        assert!(
            is_recent_lock(&lock, 5000),
            "방금 생성한 파일은 recent로 판정되어야 함"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_is_recent_lock_old_file_returns_false() {
        let dir = std::env::temp_dir().join("kmd_test_recent_lock_old");
        let _ = fs::create_dir_all(&dir);
        let lock = dir.join("old.lock");
        fs::write(&lock, "test").unwrap();

        // mtime을 2초 전으로 설정
        let two_sec_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(2);
        let _ = filetime::set_file_mtime(&lock, filetime::FileTime::from_system_time(two_sec_ago));

        assert!(
            !is_recent_lock(&lock, RECENT_LOCK_DEBOUNCE_MS),
            "2초 전 파일은 700ms debounce를 초과하여 recent가 아님"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn test_flock_prevents_zombie_false_positive() {
        // flock 기반이면 좀비 PID가 있어도 잠금 획득 가능
        let dir = std::env::temp_dir().join("kmd_test_flock_zombie");
        let _ = fs::create_dir_all(&dir);
        let _ = fs::remove_file(dir.join(LOCK_FILE));
        let _ = fs::remove_file(dir.join(QUIT_SIGNAL));

        // 현재 프로세스의 PID로 lock 파일 생성 (flock 없이)
        fs::write(dir.join(LOCK_FILE), "1").unwrap();

        // flock을 못 잡고 있으므로 새 인스턴스가 획득할 수 있어야 함
        match acquire_or_toggle(&dir) {
            InstanceAction::Acquired(guard) => {
                let contents = fs::read_to_string(&guard.lock_path).unwrap();
                assert_eq!(contents, std::process::id().to_string());
                drop(guard);
            }
            InstanceAction::SignalledExisting => {
                panic!("flock 없는 stale lock은 acquire 되어야 함")
            }
        }

        let _ = fs::remove_dir_all(&dir);
    }
}
