//! Client configuration and the daemon-spawn abstraction.

use std::ffi::OsString;
use std::io;
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::time::Duration;

/// Lower bound of the daemon wire version (`wireVersion` in `schema/version.json`)
/// this client accepts from `/v1/health` (`Capability.schemaVersion`).
///
/// Sourced at compile time from a vendored copy of `schema/version.json` — the
/// same single source of truth `kindling-store` embeds — so the client, store,
/// and daemon never disagree about the wire/schema contract.
///
/// Kept under the historical `EXPECTED_SCHEMA_VERSION` name as the lower bound
/// of the supported wire range `[EXPECTED_SCHEMA_VERSION, MAX_WIRE_VERSION]`.
pub const EXPECTED_SCHEMA_VERSION: u32 = parse_wire_version(SCHEMA_VERSION_JSON);

/// Upper bound of the daemon wire version (`Capability.schemaVersion`) this
/// client accepts. Additive features never raise `wireVersion`.
pub const MAX_WIRE_VERSION: u32 = EXPECTED_SCHEMA_VERSION;

/// Latest additive schema revision (`"version"` in `schema/version.json`)
/// known to this build.
pub const KNOWN_SCHEMA_REVISION: u32 = parse_schema_revision(SCHEMA_VERSION_JSON);

const _: () = assert!(EXPECTED_SCHEMA_VERSION == 6);
const _: () = assert!(KNOWN_SCHEMA_REVISION == 8);

const SCHEMA_VERSION_JSON: &str = include_str!("../schema/version.json");

/// Extract the integer `"wireVersion"` field from a `version.json` payload.
pub(crate) const fn parse_wire_version(json: &str) -> u32 {
    parse_json_u32(json, b"\"wireVersion\"")
}

/// Extract the integer `"version"` (schema revision) field from a `version.json` payload.
pub(crate) const fn parse_schema_revision(json: &str) -> u32 {
    parse_json_u32(json, b"\"version\"")
}

/// True when `actual` (daemon `Capability.schemaVersion`) falls within the
/// client's accepted wire range `[min_expected, max(min_expected, MAX_WIRE_VERSION)]`.
pub const fn is_supported_wire_version(actual: u32, min_expected: u32) -> bool {
    let max_supported = if MAX_WIRE_VERSION > min_expected {
        MAX_WIRE_VERSION
    } else {
        min_expected
    };
    actual >= min_expected && actual <= max_supported
}

/// Minimal compile-time-friendly extraction of an integer JSON field from
/// `schema/version.json`.
///
/// We avoid pulling `serde_json` into a `const` context (it is not const) by
/// scanning the embedded JSON for the exact key and parsing the integer that
/// follows.
const fn parse_json_u32(json: &str, key: &[u8]) -> u32 {
    let bytes = json.as_bytes();
    let mut i = 0;
    while i + key.len() <= bytes.len() {
        let mut matched = true;
        let mut k = 0;
        while k < key.len() {
            if bytes[i + k] != key[k] {
                matched = false;
                break;
            }
            k += 1;
        }
        if matched {
            // Advance past the key, the colon, and any whitespace.
            let mut j = i + key.len();
            while j < bytes.len() {
                let c = bytes[j];
                if c == b':' || c == b' ' || c == b'\t' || c == b'\n' || c == b'\r' {
                    j += 1;
                } else {
                    break;
                }
            }
            // Parse the integer.
            let mut value: u32 = 0;
            let mut saw_digit = false;
            while j < bytes.len() {
                let c = bytes[j];
                if c.is_ascii_digit() {
                    value = value * 10 + (c - b'0') as u32;
                    saw_digit = true;
                    j += 1;
                } else {
                    break;
                }
            }
            if saw_digit {
                return value;
            }
        }
        i += 1;
    }
    panic!("schema/version.json does not contain the expected integer field");
}

/// Default file name of the daemon socket under the kindling home.
const SOCKET_FILE: &str = "kindling.sock";

/// Default file name of the daemon TCP port file under the kindling home.
const PORT_FILE: &str = "kindling.port";

/// Diagnostic log for auto-spawn / cold-start failures under the kindling home.
const SPAWN_LOG_FILE: &str = "spawn.log";

/// Which transport the client uses to reach the daemon.
///
/// Mirrors `kindling_server::Transport` (each crate keeps its own copy so the
/// client need not depend on the server). `Uds` exists only on Unix; `Tcp`
/// exists everywhere and is the Windows default. Defaults to UDS on Unix and
/// TCP on Windows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Transport {
    /// Unix domain socket at [`ClientConfig::socket_path`] (Unix only; the
    /// platform default there).
    #[cfg(unix)]
    #[cfg_attr(unix, default)]
    Uds,
    /// Loopback TCP; the port is read from [`ClientConfig::port_path`]. The
    /// platform default on non-Unix (Windows).
    #[cfg_attr(not(unix), default)]
    Tcp,
}

/// How the client starts the daemon when it is not already running.
///
/// The default execs the real `kindling` binary (`$KINDLING_BIN`, else
/// `kindling` on `PATH`); tests inject a closure that
/// starts an in-process daemon so cold-spawn can be exercised without the
/// (not-yet-built) binary on `PATH`.
#[derive(Clone, Default)]
pub enum Spawner {
    /// Production path: `kindling serve --daemonize`, using `$KINDLING_BIN`
    /// when set and non-empty, else `kindling` on `PATH`. The launcher is reaped
    /// in the background so an immediate exit cannot linger as a zombie in a
    /// long-lived Unix host; the daemon itself is not awaited on the connect
    /// path. A missing binary surfaces as a spawn error, which the connect
    /// logic maps to [`ClientError::Unavailable`](crate::ClientError::Unavailable).
    #[default]
    Command,
    /// Test/custom path: invoke this closure to start the daemon.
    Custom(Arc<dyn Fn() -> io::Result<()> + Send + Sync>),
}

impl Spawner {
    /// Build a custom spawner from a closure.
    pub fn custom<F>(f: F) -> Self
    where
        F: Fn() -> io::Result<()> + Send + Sync + 'static,
    {
        Spawner::Custom(Arc::new(f))
    }

    /// Run the spawn action once.
    pub(crate) fn spawn(&self) -> io::Result<()> {
        match self {
            Spawner::Command => spawn_kindling_command(),
            Spawner::Custom(f) => f(),
        }
    }
}

/// The daemon binary [`Spawner::Command`] execs: `$KINDLING_BIN` when set and
/// non-empty, otherwise `kindling` resolved on `PATH`. Same rule as the
/// TypeScript client's `resolveBinaryPath`. Read with `var_os` so a non-UTF-8
/// path is honoured rather than silently ignored.
fn kindling_binary() -> OsString {
    binary_from_override(std::env::var_os("KINDLING_BIN"))
}

/// Pure half of [`kindling_binary`], split out so the rule is unit-testable
/// without mutating the process environment.
fn binary_from_override(value: Option<OsString>) -> OsString {
    match value {
        Some(bin) if !bin.is_empty() => bin,
        _ => OsString::from("kindling"),
    }
}

/// `kindling serve --daemonize` with stdio detached and, on Unix, its own
/// process group. The returned child is adopted so a failed launcher is
/// reaped instead of being dropped.
fn spawn_kindling_command() -> io::Result<()> {
    let program = kindling_binary();
    let mut cmd = Command::new(&program);
    cmd.args(["serve", "--daemonize"])
        // Detach the daemon's stdio from ours. Without this the
        // long-lived daemon inherits the spawner's stdout — fatal
        // for a Claude Code hook, whose stdout must carry only the
        // hook's JSON response. Null also lets the spawner exit
        // without the pipe keeping the daemon's fds open.
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // Put the daemon in its own process group so a signal sent to
    // the spawner's group (e.g. Ctrl-C in an interactive shell, or
    // the shell reaping a hook) does not also kill the daemon.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    adopt_child(cmd.spawn()?, &program)
}

/// Reap `child` if it has already exited; otherwise wait for it on a
/// background thread so a later exit cannot linger as a zombie. `program` is
/// the binary that was run, named in the error if it has already failed.
fn adopt_child(mut child: Child, program: &std::ffi::OsStr) -> io::Result<()> {
    match child.try_wait()? {
        Some(status) => child_status_result(status, program),
        None => {
            std::thread::Builder::new()
                .name("kindling-spawn-reaper".to_string())
                .spawn(move || {
                    let _ = child.wait();
                })?;
            Ok(())
        }
    }
}

fn child_status_result(status: ExitStatus, program: &std::ffi::OsStr) -> io::Result<()> {
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "{} serve --daemonize exited immediately ({status})",
            std::path::Path::new(program).display()
        )))
    }
}

impl std::fmt::Debug for Spawner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Spawner::Command => f.write_str("Spawner::Command"),
            Spawner::Custom(_) => f.write_str("Spawner::Custom(..)"),
        }
    }
}

/// Configuration for a [`Client`](crate::Client).
#[derive(Clone, Debug)]
pub struct ClientConfig {
    /// Unix domain socket the daemon listens on
    /// (`~/.kindling/kindling.sock` by default). Used when [`Self::transport`]
    /// is [`Transport::Uds`].
    pub socket_path: PathBuf,
    /// File the daemon publishes its TCP port to
    /// (`~/.kindling/kindling.port` by default). Read when [`Self::transport`]
    /// is [`Transport::Tcp`].
    pub port_path: PathBuf,
    /// Project root string, sent as the `X-Kindling-Project` header on every
    /// data endpoint. The daemon hashes it to route to a per-project DB.
    /// Defaults to the current working directory. Values that are not valid
    /// HTTP header fields return [`crate::ClientError::InvalidRequest`].
    pub project_root: String,
    /// Schema version the client requires the daemon to report from
    /// `/v1/health`. Defaults to [`EXPECTED_SCHEMA_VERSION`].
    pub expected_schema_version: u32,
    /// Total budget for the auto-spawn connect poll (connect + spawn + retry).
    /// Defaults to 1 second.
    pub connect_timeout: Duration,
    /// Interval between socket-connect attempts while polling for the daemon.
    /// Defaults to 10ms.
    pub poll_interval: Duration,
    /// How to start the daemon when it is not running. Defaults to
    /// [`Spawner::Command`]: `kindling serve --daemonize`, using `$KINDLING_BIN`
    /// when set and non-empty.
    pub spawn: Spawner,
    /// Transport to reach the daemon. Defaults to [`Transport::default`] (UDS
    /// on Unix, TCP on Windows).
    pub transport: Transport,
    /// Override for the spawn-failure diagnostic log path. When `None`,
    /// [`effective_spawn_log_path`](Self::effective_spawn_log_path) uses
    /// `~/.kindling/spawn.log`.
    pub spawn_log_path: Option<PathBuf>,
}

impl ClientConfig {
    /// Build a default config: `~/.kindling/kindling.sock`, project root from
    /// the current directory, the compiled schema version, a 1s connect budget,
    /// a 10ms poll interval, and the real binary spawner.
    ///
    /// Errors only if neither the kindling home nor the current directory can
    /// be determined.
    pub fn defaults() -> io::Result<Self> {
        let socket_path = default_socket_path().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "could not determine kindling home (no HOME/USERPROFILE)",
            )
        })?;
        let port_path = default_port_path().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "could not determine kindling home (no HOME/USERPROFILE)",
            )
        })?;
        let project_root = std::env::current_dir()?.to_string_lossy().into_owned();
        Ok(Self {
            socket_path,
            port_path,
            project_root,
            expected_schema_version: EXPECTED_SCHEMA_VERSION,
            connect_timeout: Duration::from_secs(1),
            poll_interval: Duration::from_millis(10),
            spawn: Spawner::default(),
            transport: Transport::default(),
            spawn_log_path: None,
        })
    }

    /// Path for spawn-failure diagnostics (`~/.kindling/spawn.log` by default).
    pub fn effective_spawn_log_path(&self) -> Option<PathBuf> {
        self.spawn_log_path.clone().or_else(default_spawn_log_path)
    }
}

/// Default daemon socket path: `~/.kindling/kindling.sock`.
///
/// Replicates `kindling_store::default_kindling_home`'s HOME/USERPROFILE logic
/// locally so the client need not depend on `kindling-store` (which pulls
/// rusqlite and would defeat the crate's thinness goal).
pub fn default_socket_path() -> Option<PathBuf> {
    kindling_home_dir().map(|home| home.join(SOCKET_FILE))
}

/// Default daemon TCP port file path: `~/.kindling/kindling.port`.
///
/// Mirrors [`default_socket_path`] (same HOME/USERPROFILE resolution) but points
/// at the side-channel file the TCP-transport daemon publishes its bound port
/// to.
pub fn default_port_path() -> Option<PathBuf> {
    kindling_home_dir().map(|home| home.join(PORT_FILE))
}

/// Default spawn-failure log: `~/.kindling/spawn.log`.
pub fn default_spawn_log_path() -> Option<PathBuf> {
    kindling_home_dir().map(|home| home.join(SPAWN_LOG_FILE))
}

/// Resolve `~/.kindling` from `HOME` / `USERPROFILE` (mirrors socket path logic).
fn kindling_home_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var_os("USERPROFILE").filter(|v| !v.is_empty()))?;
    Some(PathBuf::from(home).join(".kindling"))
}

/// Append a timestamped spawn-failure line to `path`. Best-effort: errors are
/// ignored so logging never masks the original connect failure.
pub(crate) fn append_spawn_log(path: &std::path::Path, detail: &str) {
    use std::io::Write;

    let _ = (|| -> io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let ts = spawn_log_timestamp_ms();
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        writeln!(file, "[{ts}] {detail}")?;
        file.flush()?;
        Ok(())
    })();
}

fn spawn_log_timestamp_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kindling_bin_override_falls_back_to_path_when_unset_or_empty() {
        assert_eq!(binary_from_override(None), OsString::from("kindling"));
        assert_eq!(
            binary_from_override(Some(OsString::new())),
            OsString::from("kindling")
        );
        assert_eq!(
            binary_from_override(Some(OsString::from("/opt/kindling/bin/kindling"))),
            OsString::from("/opt/kindling/bin/kindling")
        );
    }

    #[test]
    fn expected_schema_version_parses_to_six_and_revision_to_seven() {
        assert_eq!(EXPECTED_SCHEMA_VERSION, 6);
        assert_eq!(MAX_WIRE_VERSION, 6);
        assert_eq!(KNOWN_SCHEMA_REVISION, 8);
    }

    #[test]
    fn parse_trap_reads_wire_version_when_first_version_is_seven() {
        let json = r#"{
  "version": 7,
  "minReader": 6,
  "wireVersion": 6,
  "minCompatible": 1,
  "ftsTokenizer": "porter unicode61"
}"#;
        assert_eq!(parse_wire_version(json), 6);
        assert_eq!(parse_schema_revision(json), 7);
    }

    #[test]
    fn wire_version_range_accepts_six_and_rejects_five_and_seven() {
        assert!(!is_supported_wire_version(5, EXPECTED_SCHEMA_VERSION));
        assert!(is_supported_wire_version(6, EXPECTED_SCHEMA_VERSION));
        assert!(!is_supported_wire_version(7, EXPECTED_SCHEMA_VERSION));
    }

    #[cfg(unix)]
    #[test]
    fn already_exited_failed_launcher_is_an_error() {
        let mut child = Command::new("false").spawn().expect("spawn false");
        let status = child.wait().expect("wait for false");
        assert!(!status.success());
        // The error names the binary that actually ran, which can be a
        // `$KINDLING_BIN` override rather than `kindling` on `PATH`.
        let err = adopt_child(child, std::ffi::OsStr::new("/opt/custom/kindling"))
            .expect_err("failed launcher must surface");
        assert!(
            err.to_string()
                .contains("/opt/custom/kindling serve --daemonize exited immediately"),
            "unexpected error: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn already_exited_successful_launcher_is_ok() {
        let mut child = Command::new("true").spawn().expect("spawn true");
        let status = child.wait().expect("wait for true");
        assert!(status.success());
        adopt_child(child, std::ffi::OsStr::new("true"))
            .expect("successful launcher is already reaped");
    }

    #[cfg(unix)]
    #[test]
    fn failed_launcher_is_reaped() {
        let child = Command::new("sh")
            .args(["-c", "exit 7"])
            .spawn()
            .expect("spawn exiting shell");
        let pid = child.id();
        let _ = adopt_child(child, std::ffi::OsStr::new("sh"));
        assert_reaped(pid);
    }

    #[cfg(unix)]
    #[test]
    fn live_child_is_not_killed_and_is_reaped_after_exit() {
        let child = Command::new("sh")
            .args(["-c", "exec sleep 60"])
            .spawn()
            .expect("spawn sleep");
        let pid = child.id();
        adopt_child(child, std::ffi::OsStr::new("sh")).expect("live child is adopted");

        // The daemon path must not wait() in the foreground; the process stays up.
        std::thread::sleep(Duration::from_millis(50));
        let state = process_state(pid);
        assert!(
            state.as_deref().is_some_and(|s| !s.starts_with('Z')),
            "live child should still be running, got {state:?}"
        );

        let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
        assert_reaped(pid);
    }

    #[cfg(unix)]
    fn process_state(pid: u32) -> Option<String> {
        let output = Command::new("ps")
            .args(["-o", "state=", "-p", &pid.to_string()])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let state = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if state.is_empty() {
            None
        } else {
            Some(state)
        }
    }

    #[cfg(unix)]
    fn assert_reaped(pid: u32) {
        for _ in 0..100 {
            match process_state(pid) {
                None => return,
                Some(state) if state.starts_with('Z') => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Some(_) => {
                    // Still running (or exiting); give the reaper a moment.
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }
        let leftover = process_state(pid);
        assert!(
            leftover.is_none(),
            "pid {pid} was not reaped; ps state={leftover:?}"
        );
    }
}
