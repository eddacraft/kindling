//! Client configuration and the daemon-spawn abstraction.

use std::io;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

/// The canonical schema version this client expects the daemon to report.
///
/// Sourced at compile time from a vendored copy of `schema/version.json` — the
/// same single source of truth `kindling-store` embeds — so the client, store,
/// and daemon never disagree about the wire/schema contract.
///
/// The vendored copy (`crates/kindling-client/schema/version.json`) lives
/// inside the crate directory so `cargo publish` packages it. It is kept in
/// lock-step with the repo-root canonical `schema/version.json` by
/// `scripts/sync-vendored-schema.sh`, enforced by the `vendored-schema` CI
/// drift gate.
pub const EXPECTED_SCHEMA_VERSION: u32 = parse_schema_version();

const SCHEMA_VERSION_JSON: &str = include_str!("../schema/version.json");

/// Minimal compile-time-friendly extraction of the integer `"version"` field
/// from `schema/version.json`.
///
/// We avoid pulling `serde_json` into a `const` context (it is not const) by
/// scanning the embedded JSON for the `"version"` key and parsing the integer
/// that follows. The format is a hand-maintained, stable contract file, so a
/// targeted scan is sufficient and keeps this dependency-free and `const`.
const fn parse_schema_version() -> u32 {
    let bytes = SCHEMA_VERSION_JSON.as_bytes();
    let key = b"\"version\"";
    let mut i = 0;
    while i + key.len() <= bytes.len() {
        // Match the `"version"` key.
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
    panic!("schema/version.json does not contain an integer \"version\" field");
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
/// The default execs the real `kindling` binary; tests inject a closure that
/// starts an in-process daemon so cold-spawn can be exercised without the
/// (not-yet-built) binary on `PATH`.
#[derive(Clone, Default)]
pub enum Spawner {
    /// Production path: `kindling serve --daemonize`, detached (not awaited).
    /// The `kindling` binary is PORT-013; until it exists this path simply
    /// surfaces a clean spawn error, which the connect logic maps to
    /// [`ClientError::Unavailable`](crate::ClientError::Unavailable).
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
            Spawner::Command => {
                let mut cmd = Command::new("kindling");
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
                cmd.spawn()?;
                Ok(())
            }
            Spawner::Custom(f) => f(),
        }
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
    /// Defaults to the current working directory.
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
    /// How to start the daemon when it is not running. Defaults to the real
    /// `kindling serve --daemonize` binary.
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
    fn expected_schema_version_parses_to_five() {
        // schema/version.json currently pins version 5.
        assert_eq!(EXPECTED_SCHEMA_VERSION, 5);
    }
}
