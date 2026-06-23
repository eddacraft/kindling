//! kindling-runtime — the anvil-first integration facade.
//!
//! One Cargo dependency that bundles daemon startup, client wiring, and durable
//! emit for Rust downstreams (chiefly **anvil**) that want **one binary** and
//! **daemon semantics** without a separate `kindling` install.
//!
//! The runtime composes the existing crates — it does not fork their wire
//! shapes:
//!
//! - [`kindling-client`](kindling_client) for the HTTP-over-UDS surface
//!   (re-exported as [`Client`]).
//! - the opt-in `spool` layer for durable emit (re-exported as
//!   [`SpooledClient`]).
//! - [`kindling-server`](kindling_server) started in-process on a tokio task
//!   (the `embedded-daemon` feature).
//! - [`kindling-types`](kindling_types) re-exported as [`types`] (and the
//!   common shapes at the crate root) so consumers need no separate dep.
//!
//! kindling stays **mechanism, not policy**: the runtime owns process lifecycle
//! and client wiring; it does not encode anvil governance.
//!
//! # Quickstart
//!
//! ```no_run
//! # #[cfg(all(feature = "embedded-daemon", feature = "spool"))]
//! # async fn run() -> Result<(), kindling_runtime::RuntimeError> {
//! use kindling_runtime::{Runtime, RuntimeConfig};
//! use kindling_runtime::types::{ObservationInput, ObservationKind, ScopeIds};
//!
//! // Embedded daemon (default), durable spooled emit, default ~/.kindling home.
//! let runtime = Runtime::start(RuntimeConfig::embedded("/path/to/my/project")).await?;
//!
//! // Durable append: reaches the daemon, or buffers to the spool on outage.
//! let input = ObservationInput {
//!     id: None,
//!     kind: ObservationKind::Message,
//!     content: "gate evaluated: pass".to_string(),
//!     provenance: None,
//!     ts: None,
//!     scope_ids: ScopeIds::default(),
//!     redacted: None,
//! };
//! runtime
//!     .spooled_client()
//!     .append_observation(input, None, None)
//!     .await
//!     .expect("durable append");
//!
//! runtime.shutdown().await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Attach-or-start
//!
//! [`Runtime::start`] never *pre-emptively* starts a daemon. It builds a client
//! for the configured socket; the client only spawns when the socket does not
//! answer. So if a daemon (the CLI, a Claude Code hook, or another runtime) is
//! already listening on the same socket, the runtime **attaches** to it and the
//! embedded spawner is not invoked.

#![forbid(unsafe_code)]

#[cfg(feature = "client")]
mod spawn;

use std::path::PathBuf;

/// Domain types, re-exported so consumers need no separate `kindling-types`
/// dependency.
pub use kindling_types as types;

// The common domain shapes at the crate root for ergonomic access.
pub use kindling_types::{
    Capsule, CapsuleStatus, CapsuleType, Id, Observation, ObservationInput, ObservationKind, Pin,
    RetrieveOptions, RetrieveResult, ScopeIds,
};

#[cfg(feature = "client")]
pub use kindling_client::{Client, ClientConfig, ClientError, Spawner, Transport};

#[cfg(feature = "spool")]
pub use kindling_client::spool::{AppendOutcome, FlushReport, SpoolError, SpooledClient};

#[cfg(feature = "client")]
use std::time::Duration;

/// How the runtime obtains a running daemon when the socket is not answering.
///
/// Note that *all* strategies attach to an already-running daemon on the
/// configured socket — the variant only decides what happens when a spawn is
/// actually required.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SpawnStrategy {
    /// Start an in-process [`kindling-server`](kindling_server) on a tokio task
    /// (requires the `embedded-daemon` feature). The blessed anvil path: one
    /// binary, no `kindling` on `PATH`. This is the default.
    #[default]
    Embedded,
    /// Exec the real `kindling` binary on `PATH` (requires the `external-spawn`
    /// feature). For hosts that ship the CLI separately.
    External,
    /// Never spawn. Attach to an already-running daemon or fail with
    /// [`ClientError::Unavailable`](kindling_client::ClientError::Unavailable).
    /// For tests and hosts that manage the daemon externally.
    AttachOnly,
}

/// Configuration for a [`Runtime`].
#[derive(Clone, Debug)]
pub struct RuntimeConfig {
    /// Root of the per-project databases and the daemon's socket/pid/port files
    /// (`~/.kindling` by default). Using the default layout shares the DB with
    /// the `kindling` CLI and Claude Code hooks.
    pub kindling_home: PathBuf,
    /// Project root string, sent as the `X-Kindling-Project` header on every
    /// data endpoint for per-project DB routing.
    pub project_root: String,
    /// Path to the durable-emit spool file. Defaults to `<home>/spool.ndjson`
    /// when `None`. Only meaningful with the `spool` feature.
    pub spool_path: Option<PathBuf>,
    /// How to obtain the daemon. Defaults to [`SpawnStrategy::Embedded`].
    pub spawn: SpawnStrategy,
}

impl RuntimeConfig {
    /// Build a config rooted at the default kindling home (`~/.kindling`) for
    /// `project_root`, with the default [`SpawnStrategy`].
    ///
    /// Errors only if no home directory can be determined.
    pub fn from_default_home(project_root: impl Into<String>) -> Result<Self, RuntimeError> {
        let kindling_home = default_kindling_home()
            .ok_or_else(|| RuntimeError::Config("could not determine kindling home".into()))?;
        Ok(Self {
            kindling_home,
            project_root: project_root.into(),
            spool_path: None,
            spawn: SpawnStrategy::default(),
        })
    }

    /// Build an embedded-daemon config rooted at the default home for
    /// `project_root` (the common anvil case). Panics on a missing home only via
    /// [`from_default_home`](Self::from_default_home)'s error — prefer that for
    /// fallible construction.
    pub fn embedded(project_root: impl Into<String>) -> Self {
        let kindling_home = default_kindling_home().unwrap_or_else(|| PathBuf::from(".kindling"));
        Self {
            kindling_home,
            project_root: project_root.into(),
            spool_path: None,
            spawn: SpawnStrategy::Embedded,
        }
    }

    /// Build a config rooted at an explicit `kindling_home` (tests / isolated
    /// hosts) with the given [`SpawnStrategy`].
    pub fn with_home(
        kindling_home: impl Into<PathBuf>,
        project_root: impl Into<String>,
        spawn: SpawnStrategy,
    ) -> Self {
        Self {
            kindling_home: kindling_home.into(),
            project_root: project_root.into(),
            spool_path: None,
            spawn,
        }
    }

    /// The effective spool path: the configured one, or `<home>/spool.ndjson`.
    pub fn effective_spool_path(&self) -> PathBuf {
        self.spool_path
            .clone()
            .unwrap_or_else(|| self.kindling_home.join("spool.ndjson"))
    }

    #[cfg(feature = "client")]
    fn socket_path(&self) -> PathBuf {
        self.kindling_home.join("kindling.sock")
    }

    #[cfg(feature = "client")]
    fn port_path(&self) -> PathBuf {
        self.kindling_home.join("kindling.port")
    }
}

/// Errors from [`Runtime`] operations.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    /// Invalid or unresolvable configuration (e.g. no home directory).
    #[error("runtime config error: {0}")]
    Config(String),

    /// The requested [`SpawnStrategy`] needs a feature that was not compiled in.
    #[error("runtime feature error: {0}")]
    Feature(String),

    /// A client-side failure surfaced while wiring or probing the daemon.
    #[cfg(feature = "client")]
    #[error(transparent)]
    Client(#[from] kindling_client::ClientError),
}

/// The schema version this runtime expects the daemon to report, taken from the
/// client's compiled-in constant.
#[cfg(feature = "client")]
fn expected_schema_version() -> u32 {
    kindling_client::EXPECTED_SCHEMA_VERSION
}

/// Resolve `~/.kindling` from the environment without depending on
/// `kindling-store` (which pulls rusqlite). Mirrors the client's HOME logic.
fn default_kindling_home() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var_os("USERPROFILE").filter(|v| !v.is_empty()))?;
    Some(PathBuf::from(home).join(".kindling"))
}

/// A composed kindling integration: a client (and durable-emit spooled client)
/// over a daemon the runtime either started in-process or attached to.
///
/// Owns the embedded server task handle (when [`SpawnStrategy::Embedded`]), so
/// [`shutdown`](Self::shutdown) can stop it.
#[cfg(feature = "client")]
#[derive(Debug)]
pub struct Runtime {
    client: Client,
    #[cfg(feature = "spool")]
    spooled: SpooledClient,
    /// The strategy this runtime was started with (for diagnostics).
    strategy: SpawnStrategy,
    /// Slot holding the embedded daemon's task handle, if one was started.
    #[cfg(feature = "embedded-daemon")]
    server_handle: spawn::ServerHandleSlot,
    /// Whether the embedded spawner actually ran (false ⇒ attached to a
    /// pre-existing daemon).
    spawn_flag: spawn::SpawnFlag,
}

#[cfg(feature = "client")]
impl Runtime {
    /// Start the runtime: build a client for the configured socket and wire the
    /// [`SpawnStrategy`]. Probes the daemon with a `health` call (which triggers
    /// attach-or-spawn) so a started runtime is immediately usable.
    pub async fn start(config: RuntimeConfig) -> Result<Self, RuntimeError> {
        let spawn_flag = spawn::SpawnFlag::new();

        #[cfg(feature = "embedded-daemon")]
        let server_handle: spawn::ServerHandleSlot =
            std::sync::Arc::new(std::sync::Mutex::new(None));

        let spawner = build_spawner(&config, spawn_flag.clone(), {
            #[cfg(feature = "embedded-daemon")]
            {
                server_handle.clone()
            }
            #[cfg(not(feature = "embedded-daemon"))]
            {
                ()
            }
        })?;

        let client_config = ClientConfig {
            socket_path: config.socket_path(),
            port_path: config.port_path(),
            project_root: config.project_root.clone(),
            expected_schema_version: expected_schema_version(),
            connect_timeout: Duration::from_secs(5),
            poll_interval: Duration::from_millis(10),
            spawn: spawner,
            transport: Transport::default(),
        };

        let client = Client::with_config(client_config);

        // Probe: this triggers attach-or-spawn. A running daemon answers without
        // the spawner firing; otherwise the strategy decides what happens.
        client.health().await?;

        #[cfg(feature = "spool")]
        let spooled = SpooledClient::new(client.clone(), config.effective_spool_path());

        Ok(Self {
            client,
            #[cfg(feature = "spool")]
            spooled,
            strategy: config.spawn,
            #[cfg(feature = "embedded-daemon")]
            server_handle,
            spawn_flag,
        })
    }

    /// Borrow the underlying daemon client for reads and non-spooled ops
    /// (health, retrieve, capsules, pins, …).
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Borrow the durable-emit spooled client. Append through this for
    /// outage-resilient writes.
    #[cfg(feature = "spool")]
    pub fn spooled_client(&self) -> &SpooledClient {
        &self.spooled
    }

    /// The [`SpawnStrategy`] this runtime was started with.
    pub fn strategy(&self) -> &SpawnStrategy {
        &self.strategy
    }

    /// Whether this runtime started an embedded daemon (`true`) or attached to a
    /// pre-existing one (`false`). Only meaningful for
    /// [`SpawnStrategy::Embedded`].
    pub fn spawned_embedded_daemon(&self) -> bool {
        self.spawn_flag.fired()
    }

    /// Stop the runtime, aborting the embedded daemon task if this runtime
    /// started one. Attached daemons (started elsewhere) are left running.
    pub async fn shutdown(self) -> Result<(), RuntimeError> {
        #[cfg(feature = "embedded-daemon")]
        {
            let handle = self.server_handle.lock().ok().and_then(|mut g| g.take());
            if let Some(handle) = handle {
                handle.abort();
                // Best-effort await of the aborted task; ignore the Cancelled
                // join error.
                let _ = handle.await;
            }
        }
        Ok(())
    }
}

/// Build the [`Spawner`] for `config`'s [`SpawnStrategy`], honouring the
/// compiled feature set.
#[cfg(feature = "client")]
fn build_spawner(
    config: &RuntimeConfig,
    spawn_flag: spawn::SpawnFlag,
    #[cfg(feature = "embedded-daemon")] server_handle: spawn::ServerHandleSlot,
    #[cfg(not(feature = "embedded-daemon"))] _server_handle: (),
) -> Result<Spawner, RuntimeError> {
    match config.spawn {
        SpawnStrategy::Embedded => {
            #[cfg(feature = "embedded-daemon")]
            {
                let server_config = kindling_server::ServerConfig {
                    socket_path: config.socket_path(),
                    kindling_home: config.kindling_home.clone(),
                    pid_path: config.kindling_home.join("kindling.pid"),
                    port_path: config.port_path(),
                    // Long idle timeout: the runtime owns the lifecycle and
                    // stops the daemon on shutdown, so it must not idle out
                    // underneath a live Runtime.
                    idle_timeout: Duration::from_secs(60 * 60),
                    transport: kindling_server::Transport::default(),
                };
                Ok(spawn::embedded_spawner(
                    server_config,
                    server_handle,
                    spawn_flag,
                ))
            }
            #[cfg(not(feature = "embedded-daemon"))]
            {
                let _ = spawn_flag;
                Err(RuntimeError::Feature(
                    "SpawnStrategy::Embedded requires the `embedded-daemon` feature".into(),
                ))
            }
        }
        SpawnStrategy::External => {
            #[cfg(feature = "external-spawn")]
            {
                let _ = spawn_flag;
                Ok(Spawner::Command)
            }
            #[cfg(not(feature = "external-spawn"))]
            {
                let _ = spawn_flag;
                Err(RuntimeError::Feature(
                    "SpawnStrategy::External requires the `external-spawn` feature".into(),
                ))
            }
        }
        SpawnStrategy::AttachOnly => Ok(spawn::attach_only_spawner(spawn_flag)),
    }
}
