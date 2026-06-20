//! Daemon configuration.

use std::path::PathBuf;
use std::time::Duration;

/// Default idle timeout before the daemon shuts itself down (30 minutes).
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// Which transport the daemon binds and serves the v1 HTTP API on.
///
/// Unix domain sockets are the default (and only authenticated) transport on
/// Unix; Windows has no UDS so it falls back to loopback TCP, publishing the
/// bound ephemeral port in a side-channel file ([`ServerConfig::port_path`]).
///
/// The variant set is platform-gated so each platform's `match` stays
/// exhaustive: `Uds` exists only on Unix, `Tcp` everywhere. The Linux build
/// keeps both so the TCP path can be exercised in tests (and is what real
/// Windows uses by default).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Transport {
    /// Unix domain socket at [`ServerConfig::socket_path`] (Unix only; the
    /// platform default there).
    #[cfg(unix)]
    #[cfg_attr(unix, default)]
    Uds,
    /// Loopback TCP on an ephemeral `127.0.0.1` port published to
    /// [`ServerConfig::port_path`]. The platform default on non-Unix (Windows).
    #[cfg_attr(not(unix), default)]
    Tcp,
}

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
    /// File the TCP port is published to when [`Self::transport`] is
    /// [`Transport::Tcp`] (`~/.kindling/kindling.port` by default). Written
    /// after binding the ephemeral loopback port; removed on shutdown. Unused
    /// for the UDS transport.
    pub port_path: PathBuf,
    /// Shut down after this much idle time (no in-flight and no recent
    /// requests). Defaults to [`DEFAULT_IDLE_TIMEOUT`].
    pub idle_timeout: Duration,
    /// Transport to bind. Defaults to [`Transport::default`] (UDS on Unix, TCP
    /// on Windows).
    pub transport: Transport,
}

impl ServerConfig {
    /// Build a config rooted at `kindling_home` with conventional file names.
    pub fn new(kindling_home: PathBuf) -> Self {
        Self {
            socket_path: kindling_home.join("kindling.sock"),
            pid_path: kindling_home.join("kindling.pid"),
            port_path: kindling_home.join("kindling.port"),
            kindling_home,
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
            transport: Transport::default(),
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
