//! Client configuration and the daemon-spawn abstraction.

use std::io;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

/// The canonical schema version this client expects the daemon to report.
///
/// Sourced at compile time from the repo-root `schema/version.json` — the same
/// single source of truth `kindling-store` embeds — so the client, store, and
/// daemon never disagree about the wire/schema contract.
///
/// # cargo publish caveat
///
/// The `include_str!` path reaches outside the crate directory
/// (`../../../schema/version.json`). A future `cargo publish` of this crate
/// will need a copy step that stages `schema/version.json` inside the crate
/// before packaging (same caveat as PORT-003 in `kindling-store`). Tracked by
/// PORT-014.
pub const EXPECTED_SCHEMA_VERSION: u32 = parse_schema_version();

const SCHEMA_VERSION_JSON: &str = include_str!("../../../schema/version.json");

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
                Command::new("kindling")
                    .args(["serve", "--daemonize"])
                    .spawn()?;
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
    /// (`~/.kindling/kindling.sock` by default).
    pub socket_path: PathBuf,
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
        let project_root = std::env::current_dir()?.to_string_lossy().into_owned();
        Ok(Self {
            socket_path,
            project_root,
            expected_schema_version: EXPECTED_SCHEMA_VERSION,
            connect_timeout: Duration::from_secs(1),
            poll_interval: Duration::from_millis(10),
            spawn: Spawner::default(),
        })
    }
}

/// Default daemon socket path: `~/.kindling/kindling.sock`.
///
/// Replicates `kindling_store::default_kindling_home`'s HOME/USERPROFILE logic
/// locally so the client need not depend on `kindling-store` (which pulls
/// rusqlite and would defeat the crate's thinness goal).
pub fn default_socket_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var_os("USERPROFILE").filter(|v| !v.is_empty()))?;
    Some(PathBuf::from(home).join(".kindling").join(SOCKET_FILE))
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
