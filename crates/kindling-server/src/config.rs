//! Daemon configuration.

use std::path::PathBuf;
use std::time::Duration;

/// Default idle timeout before the daemon shuts itself down (30 minutes).
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// Runtime configuration for [`serve`](crate::serve).
///
/// Construct via [`ServerConfig::new`] for the default home-relative paths, or
/// build the struct directly (tests inject a temp `kindling_home`, a unique
/// `socket_path`, a unique `pid_path`, and a short `idle_timeout`).
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Unix domain socket to bind (`~/.kindling/kindling.sock` by default).
    pub socket_path: PathBuf,
    /// Root of per-project databases (`~/.kindling` by default). Each request's
    /// `X-Kindling-Project` header routes to `kindling_home/projects/<hash>/…`.
    pub kindling_home: PathBuf,
    /// PID file written on startup (`~/.kindling/kindling.pid` by default).
    pub pid_path: PathBuf,
    /// Shut down after this much idle time (no in-flight and no recent
    /// requests). Defaults to [`DEFAULT_IDLE_TIMEOUT`].
    pub idle_timeout: Duration,
}

impl ServerConfig {
    /// Build a config rooted at `kindling_home` with conventional file names.
    pub fn new(kindling_home: PathBuf) -> Self {
        Self {
            socket_path: kindling_home.join("kindling.sock"),
            pid_path: kindling_home.join("kindling.pid"),
            kindling_home,
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
        }
    }

    /// Build a config from the default kindling home (`~/.kindling`).
    ///
    /// Returns `None` when no home directory can be determined (mirrors
    /// [`kindling_store::default_kindling_home`]).
    pub fn from_default_home() -> Option<Self> {
        kindling_store::default_kindling_home().map(Self::new)
    }
}
