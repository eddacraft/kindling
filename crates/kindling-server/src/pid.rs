//! Exclusive daemon ownership via an OS file lock.
//!
//! Ownership is an advisory exclusive lock on a lock file next to
//! [`ServerConfig::pid_path`](crate::ServerConfig::pid_path) (`kindling.pid`
//! → `kindling.lock`), held for the daemon's lifetime. It is `flock` on Unix
//! and `LockFileEx` on Windows, so the kernel guarantees a single owner and
//! releases the lock when the process exits, including on a crash. That means
//! no stale-file cleanup and no liveness check.
//!
//! - The lock file is opened with `create(true)` and never truncated or
//!   deleted. Deleting it would let one starter lock the unlinked inode while
//!   another creates and locks a new file.
//! - The PID is written to `pid_path` only after the lock is held. It is for
//!   observability and the [`ServerError::AlreadyRunning`] message. It is not
//!   the source of truth. On Windows a locked file can't be read by other
//!   processes, which is why the PID lives in a separate file.
//! - [`PidGuard`] removes the PID file on drop, then releases the lock.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};

use crate::error::ServerError;

/// Holds the daemon's ownership lock. Dropping it removes the PID file
/// (best-effort) and releases the lock; the lock file itself stays.
#[derive(Debug)]
pub struct PidGuard {
    path: PathBuf,
    lock: File,
}

impl PidGuard {
    /// The PID-file path this guard owns.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PidGuard {
    fn drop(&mut self) {
        // Remove the PID file while the lock is still held, so no other
        // process can be writing it. Closing the handle would release the
        // lock anyway; unlocking explicitly makes the release point obvious.
        let _ = fs::remove_file(&self.path);
        let _ = self.lock.unlock();
    }
}

/// The lock file guarding `pid_path`: same directory, extension `lock`.
///
/// Give each daemon a `pid_path` whose extension isn't `lock`, and don't put
/// two pid paths that differ only in extension in one directory. Otherwise
/// the lock and PID files, or two daemons' locks, would be the same file.
pub fn lock_path_for(pid_path: &Path) -> PathBuf {
    pid_path.with_extension("lock")
}

/// Acquire exclusive daemon ownership for `pid_path`.
///
/// Returns a [`PidGuard`] that holds the lock until dropped. Errors with
/// [`ServerError::AlreadyRunning`] only when another owner holds the lock (the
/// PID is `None` if its PID file can't be read yet), and with
/// [`ServerError::Pid`] for any other failure.
pub fn acquire_pid_lock(pid_path: &Path) -> Result<PidGuard, ServerError> {
    if let Some(parent) = pid_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|e| ServerError::Pid(format!("creating pid dir: {e}")))?;
        }
    }

    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path_for(pid_path))
        .map_err(|e| ServerError::Pid(format!("opening lock file: {e}")))?;

    match lock.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => {
            return Err(ServerError::AlreadyRunning(read_pid(pid_path)));
        }
        Err(TryLockError::Error(e)) => {
            return Err(ServerError::Pid(format!("locking lock file: {e}")));
        }
    }

    // Build the guard before writing, so a failed write still releases the
    // lock (and removes any partial PID file) on the error path.
    let guard = PidGuard {
        path: pid_path.to_path_buf(),
        lock,
    };
    fs::write(pid_path, std::process::id().to_string())
        .map_err(|e| ServerError::Pid(format!("writing pid file: {e}")))?;
    Ok(guard)
}

/// The owner's PID, if its PID file is present and well-formed. Anything else
/// (missing, mid-write, malformed, unreadable) is an unknown PID. It never
/// makes the owner stale, because only the lock decides ownership.
fn read_pid(pid_path: &Path) -> Option<i32> {
    fs::read_to_string(pid_path).ok()?.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    fn me() -> i32 {
        std::process::id() as i32
    }

    #[test]
    fn acquires_and_writes_pid_when_nothing_exists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kindling.pid");
        let guard = acquire_pid_lock(&path).expect("should acquire");
        assert_eq!(fs::read_to_string(&path).unwrap(), me().to_string());
        assert!(dir.path().join("kindling.lock").exists());
        drop(guard);
        assert!(!path.exists(), "guard should remove the pid file on drop");
        assert!(
            dir.path().join("kindling.lock").exists(),
            "the lock file must never be deleted"
        );
    }

    #[test]
    fn live_owner_blocks_until_its_guard_drops() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kindling.pid");
        let owner = acquire_pid_lock(&path).expect("first owner");

        let blocked = acquire_pid_lock(&path);
        assert!(
            matches!(blocked, Err(ServerError::AlreadyRunning(Some(pid))) if pid == me()),
            "a live owner must block acquisition, got {blocked:?}"
        );
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            me().to_string(),
            "a losing starter must not touch the owner's pid file"
        );

        drop(owner);
        let next = acquire_pid_lock(&path).expect("released lock should be acquirable");
        drop(next);
    }

    #[test]
    fn leftover_files_from_a_crashed_owner_do_not_block_startup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kindling.pid");
        let lock_path = dir.path().join("kindling.lock");
        // A crashed owner leaves both files behind, but the OS released its lock.
        fs::write(&lock_path, b"leftover").unwrap();
        fs::write(&path, i32::MAX.to_string()).unwrap();

        let guard = acquire_pid_lock(&path).expect("leftovers must not block acquisition");
        assert_eq!(fs::read_to_string(&path).unwrap(), me().to_string());
        assert_eq!(
            fs::read(&lock_path).unwrap(),
            b"leftover",
            "the lock file must be opened without truncation"
        );
        drop(guard);
    }

    #[test]
    fn unreadable_owner_pid_is_reported_as_unknown_not_stale() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kindling.pid");
        let owner = acquire_pid_lock(&path).expect("owner");

        fs::write(&path, "not-a-pid").unwrap();
        assert!(matches!(
            acquire_pid_lock(&path),
            Err(ServerError::AlreadyRunning(None))
        ));

        fs::remove_file(&path).unwrap();
        assert!(matches!(
            acquire_pid_lock(&path),
            Err(ServerError::AlreadyRunning(None))
        ));
        drop(owner);
    }

    #[test]
    fn lock_file_failures_are_pid_errors_not_already_running() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kindling.pid");
        // A directory where the lock file should be cannot be opened as a file.
        fs::create_dir(dir.path().join("kindling.lock")).unwrap();
        let result = acquire_pid_lock(&path);
        assert!(matches!(result, Err(ServerError::Pid(_))), "got {result:?}");
    }

    #[test]
    fn concurrent_starters_produce_exactly_one_owner() {
        const STARTERS: usize = 6;
        const ROUNDS: usize = 200;
        for round in 0..ROUNDS {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("kindling.pid");
            let barrier = Arc::new(Barrier::new(STARTERS));
            let results: Vec<_> = (0..STARTERS)
                .map(|_| {
                    let barrier = Arc::clone(&barrier);
                    let path = path.clone();
                    std::thread::spawn(move || {
                        barrier.wait();
                        acquire_pid_lock(&path)
                    })
                })
                .collect::<Vec<_>>()
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect();
            let owners = results.iter().filter(|result| result.is_ok()).count();
            assert_eq!(owners, 1, "round {round}: {owners} owners");
            for result in &results {
                assert!(
                    matches!(result, Ok(_) | Err(ServerError::AlreadyRunning(_))),
                    "round {round}: unexpected loser error {result:?}"
                );
            }
        }
    }
}
