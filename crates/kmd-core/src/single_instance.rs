//! Single-instance guard with toggle behavior.
//!
//! When `kmd` starts:
//!   1. Check for `kmd.lock` — if another instance is alive, signal it to quit
//!   2. The *new* process creates a `kmd.quit` signal file, waits briefly, then exits
//!   3. The *existing* process detects `kmd.quit` in its event loop and exits gracefully
//!
//! Combined with immediate `ShowWindow(SW_HIDE)` on startup, the toggle-off
//! path is nearly invisible — the console window is hidden before it renders.

use std::fs;
use std::path::{Path, PathBuf};

// ── Lock file name constants ─────────────────────────────────────────────────

const LOCK_FILE: &str = "kmd.lock";
const QUIT_SIGNAL: &str = "kmd.quit";

/// How many iterations to wait for the other instance to exit.
const TOGGLE_WAIT_ITERATIONS: u32 = 40;
/// Milliseconds to sleep between each poll.
const TOGGLE_WAIT_MS: u64 = 50;

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
pub struct Guard {
    lock_path: PathBuf,
    quit_signal_path: PathBuf,
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
        let _ = fs::remove_file(&self.lock_path);
        // Also clean up any leftover quit signal
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
    // Ensure directory exists
    let _ = fs::create_dir_all(data_dir);

    let (lock_path, quit_signal_path) = lock_paths(data_dir);

    // Clean up any stale quit signal from a previous crash
    let _ = fs::remove_file(&quit_signal_path);

    // Read existing lock file
    if let Ok(contents) = fs::read_to_string(&lock_path) {
        if let Ok(pid) = contents.trim().parse::<u32>() {
            if pid != std::process::id() && is_process_alive(pid) {
                // ── Another instance is alive — signal it to quit ────────

                // Write the quit signal file
                let _ = fs::write(&quit_signal_path, "quit");

                // Wait for the existing process to exit (up to ~2 seconds)
                for _ in 0..TOGGLE_WAIT_ITERATIONS {
                    std::thread::sleep(std::time::Duration::from_millis(TOGGLE_WAIT_MS));
                    if !is_process_alive(pid) {
                        break;
                    }
                }

                // Clean up if the process didn't remove its lock
                if !is_process_alive(pid) {
                    let _ = fs::remove_file(&lock_path);
                }

                return InstanceAction::SignalledExisting;
            }
        }
        // Stale lock file — remove it
        let _ = fs::remove_file(&lock_path);
    }

    // ── No other instance — acquire the lock ─────────────────────────────

    let our_pid = std::process::id();
    let _ = fs::write(&lock_path, our_pid.to_string());

    InstanceAction::Acquired(Guard {
        lock_path,
        quit_signal_path,
    })
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
        // Clean up from previous runs
        let _ = fs::remove_file(dir.join(LOCK_FILE));
        let _ = fs::remove_file(dir.join(QUIT_SIGNAL));

        match acquire_or_toggle(&dir) {
            InstanceAction::Acquired(guard) => {
                // Lock file should exist with our PID
                let contents = fs::read_to_string(&guard.lock_path).unwrap();
                assert_eq!(contents, std::process::id().to_string());

                // should_quit should be false
                assert!(!guard.should_quit());

                drop(guard);
                // Lock file should be cleaned up
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

        // Write a fake PID that doesn't exist
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

                // Simulate external quit signal
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
}
