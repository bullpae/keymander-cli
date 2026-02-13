//! Single-instance guard with toggle behavior.
//!
//! When the TUI starts:
//!   - If another instance is already running → **kill it** and return `Toggle::KilledExisting`
//!   - If no other instance → acquire the lock and return a `Guard`
//!
//! This enables hotkey-toggle: press once → open, press again → close.

use std::fs;
use std::path::{Path, PathBuf};

/// Result of attempting to acquire the single-instance lock.
pub enum InstanceAction {
    /// No other instance was running. The `Guard` holds the lock;
    /// dropping it cleans up the lock file.
    Acquired(Guard),
    /// Another instance was found and killed. The caller should exit.
    KilledExisting(u32),
}

/// RAII guard — removes the lock file when dropped.
pub struct Guard {
    lock_path: PathBuf,
}

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.lock_path);
    }
}

/// Try to become the single instance, or kill the existing one.
///
/// `lock_path` is the full path to the lock file (e.g. `<data_dir>/kmd.lock`).
pub fn acquire_or_toggle(lock_path: &Path) -> InstanceAction {
    // Ensure parent directory exists
    if let Some(parent) = lock_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    // Read existing lock file
    if let Ok(contents) = fs::read_to_string(lock_path) {
        if let Ok(pid) = contents.trim().parse::<u32>() {
            if pid != std::process::id() && is_process_alive(pid) {
                // Another instance is running — kill it
                kill_process(pid);
                // Clean up stale lock
                let _ = fs::remove_file(lock_path);
                return InstanceAction::KilledExisting(pid);
            }
        }
        // Stale lock file (process dead or invalid) — remove it
        let _ = fs::remove_file(lock_path);
    }

    // Write our PID
    let our_pid = std::process::id();
    let _ = fs::write(lock_path, our_pid.to_string());

    InstanceAction::Acquired(Guard {
        lock_path: lock_path.to_path_buf(),
    })
}

// ── Platform-specific helpers ────────────────────────────────────────────────

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
fn kill_process(pid: u32) {
    unsafe {
        let handle = OpenProcess(1, 0, pid); // PROCESS_TERMINATE = 1
        if !handle.is_null() {
            TerminateProcess(handle, 1);
            CloseHandle(handle);
        }
    }
}

#[cfg(windows)]
unsafe extern "system" {
    fn OpenProcess(desired_access: u32, inherit_handles: i32, process_id: u32) -> *mut std::ffi::c_void;
    fn GetExitCodeProcess(process: *mut std::ffi::c_void, exit_code: *mut u32) -> i32;
    fn TerminateProcess(process: *mut std::ffi::c_void, exit_code: u32) -> i32;
    fn CloseHandle(object: *mut std::ffi::c_void) -> i32;
}

#[cfg(unix)]
fn is_process_alive(pid: u32) -> bool {
    // signal 0 = check existence without actually sending a signal
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

#[cfg(unix)]
fn kill_process(pid: u32) {
    unsafe {
        libc::kill(pid as i32, libc::SIGTERM);
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_acquire_new_instance() {
        let dir = std::env::temp_dir().join("kmd_test_single_instance");
        let _ = fs::create_dir_all(&dir);
        let lock = dir.join("test.lock");
        let _ = fs::remove_file(&lock);

        match acquire_or_toggle(&lock) {
            InstanceAction::Acquired(guard) => {
                // Lock file should exist with our PID
                let contents = fs::read_to_string(&lock).unwrap();
                assert_eq!(contents, std::process::id().to_string());
                drop(guard);
                // Lock file should be cleaned up
                assert!(!lock.exists());
            }
            InstanceAction::KilledExisting(_) => panic!("Should have acquired, not killed"),
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_stale_lock_file_is_replaced() {
        let dir = std::env::temp_dir().join("kmd_test_stale_lock");
        let _ = fs::create_dir_all(&dir);
        let lock = dir.join("test.lock");

        // Write a fake PID that doesn't exist
        fs::write(&lock, "99999999").unwrap();

        match acquire_or_toggle(&lock) {
            InstanceAction::Acquired(guard) => {
                let contents = fs::read_to_string(&lock).unwrap();
                assert_eq!(contents, std::process::id().to_string());
                drop(guard);
            }
            InstanceAction::KilledExisting(_) => panic!("Stale PID should not be killed"),
        }

        let _ = fs::remove_dir_all(&dir);
    }
}
