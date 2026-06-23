//! Attach-or-start spawn wiring.
//!
//! The [`Runtime`](crate::Runtime) needs a [`Spawner`] that matches its
//! configured [`SpawnStrategy`](crate::SpawnStrategy). The client already only
//! invokes the spawner when the socket does not answer, so "attach to a running
//! daemon" needs no special case here — every strategy simply decides *what to
//! do when a spawn is actually required*:
//!
//! - [`SpawnStrategy::Embedded`] starts an in-process [`serve`] on a tokio task
//!   (the `cold_spawn_starts_daemon` pattern). The spawned [`JoinHandle`] is
//!   captured so the [`Runtime`] can stop it on
//!   [`shutdown`](crate::Runtime::shutdown).
//! - [`SpawnStrategy::AttachOnly`] errors when a spawn is required — it never
//!   starts a daemon.
//! - [`SpawnStrategy::External`] execs the real `kindling` binary on `PATH`.

use std::sync::Arc;

use kindling_client::Spawner;

#[cfg(feature = "embedded-daemon")]
use kindling_server::{serve, ServerConfig};
#[cfg(feature = "embedded-daemon")]
use std::sync::Mutex;
#[cfg(feature = "embedded-daemon")]
use tokio::task::JoinHandle;

/// A handle to the in-process daemon task, when one was started.
///
/// Shared between the [`Spawner`] closure (which fills it in the first time the
/// embedded daemon is actually started) and the [`Runtime`](crate::Runtime)
/// (which aborts it on shutdown).
#[cfg(feature = "embedded-daemon")]
pub(crate) type ServerHandleSlot =
    Arc<Mutex<Option<JoinHandle<Result<(), kindling_server::ServerError>>>>>;

/// Tracks whether the embedded spawner has actually run. Exposed so callers
/// (and tests) can assert attach-vs-spawn behaviour.
#[derive(Clone, Debug, Default)]
pub(crate) struct SpawnFlag(Arc<std::sync::atomic::AtomicBool>);

impl SpawnFlag {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn mark(&self) {
        self.0.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub(crate) fn fired(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// Build an embedded [`Spawner`] that starts `serve(config)` on a tokio task the
/// first time the client needs a daemon. Records the [`JoinHandle`] in `slot`
/// and flips `flag` when it runs.
///
/// If a daemon is already listening on the configured socket, the client never
/// calls this closure (it connects directly), so no second daemon is started.
#[cfg(feature = "embedded-daemon")]
pub(crate) fn embedded_spawner(
    config: ServerConfig,
    slot: ServerHandleSlot,
    flag: SpawnFlag,
) -> Spawner {
    Spawner::custom(move || {
        flag.mark();
        let cfg = config.clone();
        let handle = tokio::spawn(async move { serve(cfg).await });
        // Record the handle so shutdown can abort it. If a previous handle is
        // present (e.g. a re-spawn after the daemon idled out), drop it — the
        // task it referred to has already exited.
        if let Ok(mut guard) = slot.lock() {
            *guard = Some(handle);
        }
        Ok(())
    })
}

/// Build an attach-only [`Spawner`] that refuses to start a daemon.
///
/// Mirrors `TestDaemon`'s panic-spawner, but returns an error instead of
/// panicking so a missing daemon surfaces as
/// [`ClientError::Unavailable`](kindling_client::ClientError::Unavailable)
/// rather than aborting the process. `flag` is shared so an attach path can be
/// asserted to have *not* spawned.
pub(crate) fn attach_only_spawner(flag: SpawnFlag) -> Spawner {
    Spawner::custom(move || {
        // If this ever runs, the socket was not answering and AttachOnly was
        // asked to start a daemon — that is a configuration error.
        flag.mark();
        Err(std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "kindling-runtime: AttachOnly strategy will not start a daemon \
             (no daemon is listening on the configured socket)",
        ))
    })
}
