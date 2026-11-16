//! PID-file lock with stale-cleanup.
//!
//! On startup the daemon writes its PID to [`ServerConfig::pid_path`]. Before
//! binding, if a PID file already exists it is checked for liveness:
//!   - **dead or malformed PID → stale**: the entry is quarantined and
//!     acquisition proceeds, writing a fresh ownership record.
//!   - **live PID → another daemon is running**: acquisition fails with
//!     [`ServerError::AlreadyRunning`] (the file is never clobbered).
//!
//! [`PidGuard`] removes its exact ownership record on drop; a successor's
//! different ownership record is left untouched.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::ServerError;

/// Holds the PID-file lock for the lifetime of the daemon.
#[derive(Debug)]
pub struct PidGuard {
    path: PathBuf,
    ownership: String,
}

impl PidGuard {
    /// The PID-file path this guard owns.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PidGuard {
    fn drop(&mut self) {
        if let Ok(contents) = fs::read_to_string(&self.path) {
            if contents == self.ownership {
                let _ = fs::remove_file(&self.path);
            }
        }
    }
}

/// Acquire the PID-file lock at `path`, cleaning up a stale file if present.
///
/// Returns a [`PidGuard`] that releases ownership on drop. Errors with
/// [`ServerError::AlreadyRunning`] if a live daemon already holds the file.
pub fn acquire_pid_lock(path: &Path) -> Result<PidGuard, ServerError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|e| ServerError::Pid(format!("creating pid dir: {e}")))?;
        }
    }

    let pid = std::process::id();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| ServerError::Pid(format!("clock before epoch: {e}")))?
        .as_nanos();
    let ownership = format!("{pid}\n{nonce}");
    loop {
        match OpenOptions::new().write(true).create_new(true).open(path) {
            Ok(mut file) => {
                file.write_all(ownership.as_bytes())
                    .map_err(|e| ServerError::Pid(format!("writing pid file: {e}")))?;
                return Ok(PidGuard {
                    path: path.to_path_buf(),
                    ownership: ownership.clone(),
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let contents = fs::read_to_string(path)
                    .map_err(|e| ServerError::Pid(format!("reading pid file: {e}")))?;
                let existing = contents
                    .lines()
                    .next()
                    .and_then(|line| line.trim().parse::<i32>().ok());
                if let Some(existing) = existing {
                    if process_is_alive(existing) {
                        return Err(ServerError::AlreadyRunning(existing));
                    }
                }
                // Move the stale entry out of the lock path atomically. A
                // concurrent starter can then either create the original path
                // or fail its own atomic create, but can never be removed by
                // this cleanup.
                let nonce = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|e| ServerError::Pid(format!("clock before epoch: {e}")))?
                    .as_nanos();
                let quarantine = path.with_extension(format!("stale.{pid}.{nonce}"));
                match fs::rename(path, &quarantine) {
                    Ok(()) => {
                        if fs::read_to_string(&quarantine).ok().as_deref()
                            == Some(contents.as_str())
                        {
                            let _ = fs::remove_file(quarantine);
                        } else {
                            // Restore only when the lock path is still free;
                            // never replace a successor that won the race.
                            if let Ok(replacement) = fs::read(&quarantine) {
                                if let Ok(mut file) =
                                    OpenOptions::new().write(true).create_new(true).open(path)
                                {
                                    let _ = file.write_all(&replacement);
                                    let _ = fs::remove_file(&quarantine);
                                }
                            }
                            continue;
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(error) => {
                        return Err(ServerError::Pid(format!("quarantining stale pid: {error}")));
                    }
                }
            }
            Err(error) => return Err(ServerError::Pid(format!("creating pid file: {error}"))),
        }
    }
}

/// Whether a process with `pid` is currently alive.
#[cfg(unix)]
fn process_is_alive(pid: i32) -> bool {
    use nix::sys::signal::kill;
    use nix::unistd::Pid;
    // Signal 0 performs error checking without sending a signal. `Ok` means the
    // process exists (and we may signal it); `EPERM` means it exists but we
    // lack permission — still alive. Any other error means it's gone.
    match kill(Pid::from_raw(pid), None) {
        Ok(()) => true,
        Err(nix::errno::Errno::EPERM) => true,
        Err(_) => false,
    }
}

#[cfg(windows)]
fn process_is_alive(pid: i32) -> bool {
    // `tasklist` is available on supported Windows installations and avoids
    // introducing unsafe Win32 FFI into a crate that forbids unsafe code.
    let Ok(output) = std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .output()
    else {
        return false;
    };
    output.status.success()
        && String::from_utf8_lossy(&output.stdout).contains(&format!("\"{pid}\""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn acquires_when_no_pidfile() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kindling.pid");
        let guard = acquire_pid_lock(&path).expect("should acquire");
        let written = fs::read_to_string(&path).unwrap();
        assert_eq!(
            written.lines().next().unwrap(),
            std::process::id().to_string()
        );
        drop(guard);
        assert!(!path.exists(), "guard should remove its ownership record");
    }

    #[test]
    fn cleans_up_stale_pidfile() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kindling.pid");

        // Write a pidfile with a guaranteed-dead PID. PID 2^31-1 is not a live
        // process on any reasonable system.
        let dead_pid = i32::MAX;
        {
            let mut f = fs::File::create(&path).unwrap();
            write!(f, "{dead_pid}").unwrap();
        }
        assert!(!process_is_alive(dead_pid));

        let guard = acquire_pid_lock(&path).expect("stale pid must not block acquisition");
        let written = fs::read_to_string(&path).unwrap();
        assert_eq!(
            written.lines().next().unwrap(),
            std::process::id().to_string(),
            "pid file should be rewritten with the new (live) pid"
        );
        drop(guard);
    }

    #[test]
    fn live_pidfile_blocks_acquisition() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kindling.pid");

        // Our own PID is definitely alive.
        let me = std::process::id() as i32;
        fs::write(&path, me.to_string()).unwrap();

        let result = acquire_pid_lock(&path);
        assert!(
            matches!(result, Err(ServerError::AlreadyRunning(p)) if p == me),
            "a live pidfile must block acquisition"
        );
        // The live file must be left untouched.
        assert_eq!(fs::read_to_string(&path).unwrap().trim(), me.to_string());
    }

    #[test]
    fn concurrent_acquisition_has_one_owner() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kindling.pid");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let first = {
            let barrier = barrier.clone();
            let path = path.clone();
            std::thread::spawn(move || {
                barrier.wait();
                acquire_pid_lock(&path)
            })
        };
        let second = {
            let barrier = barrier.clone();
            let path = path.clone();
            std::thread::spawn(move || {
                barrier.wait();
                acquire_pid_lock(&path)
            })
        };
        let results = [first.join().unwrap(), second.join().unwrap()];
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(results.iter().filter(|result| result.is_err()).count(), 1);
        drop(results);
    }
}
