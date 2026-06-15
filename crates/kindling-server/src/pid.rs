//! PID-file lock with stale-cleanup.
//!
//! On startup the daemon writes its PID to [`ServerConfig::pid_path`]. Before
//! binding, if a PID file already exists it is checked for liveness:
//!   - **dead PID → stale**: the file is removed and acquisition proceeds,
//!     rewriting the file with the current PID.
//!   - **live PID → another daemon is running**: acquisition fails with
//!     [`ServerError::AlreadyRunning`] (the file is never clobbered).
//!
//! [`PidGuard`] removes the PID file on drop, so a clean shutdown leaves no
//! stale file behind.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::ServerError;

/// Holds the PID-file lock for the lifetime of the daemon. Removes the file on
/// drop (best-effort).
#[derive(Debug)]
pub struct PidGuard {
    path: PathBuf,
}

impl PidGuard {
    /// The PID-file path this guard owns.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PidGuard {
    fn drop(&mut self) {
        // Best-effort: only remove the file if it still holds *our* pid, so we
        // never delete a file a successor process has rewritten.
        if let Ok(contents) = fs::read_to_string(&self.path) {
            if contents.trim().parse::<i32>().ok() == Some(std::process::id() as i32) {
                let _ = fs::remove_file(&self.path);
            }
        }
    }
}

/// Acquire the PID-file lock at `path`, cleaning up a stale file if present.
///
/// Returns a [`PidGuard`] that releases the lock on drop. Errors with
/// [`ServerError::AlreadyRunning`] if a live daemon already holds the file.
pub fn acquire_pid_lock(path: &Path) -> Result<PidGuard, ServerError> {
    if let Some(existing) = read_pid(path)? {
        if process_is_alive(existing) {
            return Err(ServerError::AlreadyRunning(existing));
        }
        // Stale: the owning process is gone. Remove and take over.
        fs::remove_file(path).map_err(|e| ServerError::Pid(format!("removing stale pid: {e}")))?;
    }

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|e| ServerError::Pid(format!("creating pid dir: {e}")))?;
        }
    }

    let pid = std::process::id();
    fs::write(path, pid.to_string())
        .map_err(|e| ServerError::Pid(format!("writing pid file: {e}")))?;

    Ok(PidGuard {
        path: path.to_path_buf(),
    })
}

/// Read and parse the PID from `path`, if the file exists. A malformed file is
/// treated as stale (`Ok(None)` would leave it in place, so we surface a parse
/// failure as "no live owner" by returning `Ok(None)` — the caller then treats
/// a present-but-unparseable file as removable). We return the parsed PID when
/// readable, and `Ok(None)` when the file is absent or unparseable.
fn read_pid(path: &Path) -> Result<Option<i32>, ServerError> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(contents.trim().parse::<i32>().ok()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(ServerError::Pid(format!("reading pid file: {e}"))),
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
fn process_is_alive(_pid: i32) -> bool {
    // Minimal Windows fallback: assume not alive so a stale file never blocks
    // startup. PORT-013 can harden this with OpenProcess if needed.
    false
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
        assert_eq!(written.trim(), std::process::id().to_string());
        drop(guard);
        assert!(!path.exists(), "guard should remove pid file on drop");
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
            written.trim(),
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
}
