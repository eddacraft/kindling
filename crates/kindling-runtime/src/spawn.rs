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
//!   captured so the [`Runtime`](crate::Runtime) can stop it on
//!   [`shutdown`](crate::Runtime::shutdown).
//! - [`SpawnStrategy::AttachOnly`] errors when a spawn is required — it never
//!   starts a daemon.
//! - [`SpawnStrategy::External`] execs the real `kindling` binary on `PATH`.

use std::sync::Arc;

use kindling_client::Spawner;

#[cfg(feature = "embedded-daemon")]
use kindling_server::{serve, ServerConfig, ServerError};
#[cfg(feature = "embedded-daemon")]
use std::sync::Mutex;
#[cfg(feature = "embedded-daemon")]
use tokio::task::JoinHandle;

/// Join handle for an in-process `serve` task.
#[cfg(feature = "embedded-daemon")]
pub(crate) type ServerJoinHandle = JoinHandle<Result<(), ServerError>>;

/// A handle to the in-process daemon task, when one was started.
///
/// Shared between the [`Spawner`] closure (which fills it the first time the
/// embedded daemon is actually started) and the [`Runtime`](crate::Runtime)
/// (which aborts it on shutdown). Never drops a live [`JoinHandle`]: that
/// would detach the task and leave a daemon with no owner.
#[cfg(feature = "embedded-daemon")]
#[derive(Clone, Debug)]
pub(crate) struct ServerHandleSlot(Arc<Mutex<Option<ServerJoinHandle>>>);

#[cfg(feature = "embedded-daemon")]
impl ServerHandleSlot {
    pub(crate) fn new() -> Self {
        Self(Arc::new(Mutex::new(None)))
    }

    /// Spawn `serve(config)` unless a live task is already recorded.
    ///
    /// A finished previous handle (idle-out, bind failure) is replaced. A
    /// still-running one is kept: spawning another would either fail to bind
    /// or overwrite the live handle and detach it.
    fn start_serve(&self, config: ServerConfig) -> std::io::Result<()> {
        let mut guard = self.0.lock().map_err(|_| {
            std::io::Error::other("kindling-runtime: embedded server handle slot poisoned")
        })?;
        if guard.as_ref().is_some_and(|h| !h.is_finished()) {
            return Ok(());
        }
        *guard = Some(tokio::spawn(async move { serve(config).await }));
        Ok(())
    }

    /// Abort the stored task, if any, and wait for it to finish.
    ///
    /// A poisoned mutex is treated as "no handle": the only code that holds
    /// this lock is the spawner storing a [`JoinHandle`], which never panics
    /// while holding it. If it somehow did, the daemon idles out on its own.
    pub(crate) async fn abort_and_join(&self) {
        let handle = self.take();
        if let Some(handle) = handle {
            handle.abort();
            let _ = handle.await;
        }
    }

    /// Abort the stored task without awaiting it. Used from [`AbortOnDrop`],
    /// which cannot await.
    fn abort(&self) {
        if let Some(handle) = self.take() {
            handle.abort();
        }
    }

    fn take(&self) -> Option<ServerJoinHandle> {
        self.0.lock().ok().and_then(|mut g| g.take())
    }

    /// Store `handle`, aborting it instead if a live task is already present.
    ///
    /// Returns the incoming handle when it was rejected (already aborted) so
    /// the caller can await it; `None` when it was stored.
    #[cfg(test)]
    fn record(&self, handle: ServerJoinHandle) -> Option<ServerJoinHandle> {
        let Ok(mut guard) = self.0.lock() else {
            handle.abort();
            return Some(handle);
        };
        if guard.as_ref().is_some_and(|h| !h.is_finished()) {
            handle.abort();
            return Some(handle);
        }
        *guard = Some(handle);
        None
    }
}

/// Drops abort any embedded task recorded in [`ServerHandleSlot`].
///
/// [`Runtime::start`](crate::Runtime::start) holds this across the health
/// probe so a failed or cancelled start does not detach the tokio task.
#[cfg(feature = "embedded-daemon")]
#[must_use = "dropping aborts the embedded daemon task"]
pub(crate) struct AbortOnDrop {
    slot: ServerHandleSlot,
    armed: bool,
}

#[cfg(feature = "embedded-daemon")]
impl AbortOnDrop {
    pub(crate) fn new(slot: ServerHandleSlot) -> Self {
        Self { slot, armed: true }
    }

    /// Keep the stored task (the [`Runtime`](crate::Runtime) now owns it).
    pub(crate) fn defuse(mut self) {
        self.armed = false;
    }
}

#[cfg(feature = "embedded-daemon")]
impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        if self.armed {
            self.slot.abort();
        }
    }
}

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
/// Concurrent calls on the same slot (two in-flight client requests racing a
/// cold socket) keep the first live task rather than detaching it.
#[cfg(feature = "embedded-daemon")]
pub(crate) fn embedded_spawner(
    config: ServerConfig,
    slot: ServerHandleSlot,
    flag: SpawnFlag,
) -> Spawner {
    Spawner::custom(move || {
        flag.mark();
        slot.start_serve(config.clone())
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

#[cfg(all(test, feature = "embedded-daemon"))]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use kindling_client::{Client, ClientConfig, Transport};
    use kindling_server::ServerConfig;

    fn pending_server_task(dropped: Arc<AtomicBool>) -> ServerJoinHandle {
        tokio::spawn(async move {
            struct OnDrop(Arc<AtomicBool>);
            impl Drop for OnDrop {
                fn drop(&mut self) {
                    self.0.store(true, Ordering::SeqCst);
                }
            }
            let _on_drop = OnDrop(dropped);
            std::future::pending::<()>().await;
            Ok(())
        })
    }

    fn temp_server_config(home: &std::path::Path) -> ServerConfig {
        ServerConfig {
            socket_path: home.join("kindling.sock"),
            kindling_home: home.to_path_buf(),
            pid_path: home.join("kindling.pid"),
            port_path: home.join("kindling.port"),
            idle_timeout: Duration::from_secs(3600),
            transport: kindling_server::Transport::default(),
        }
    }

    #[tokio::test]
    async fn record_overlapping_live_task_keeps_first_and_aborts_second() {
        let slot = ServerHandleSlot::new();
        let first_dropped = Arc::new(AtomicBool::new(false));
        let second_dropped = Arc::new(AtomicBool::new(false));

        let first = pending_server_task(Arc::clone(&first_dropped));
        let second = pending_server_task(Arc::clone(&second_dropped));
        // Poll both once so their drop guards are live before we abort.
        tokio::task::yield_now().await;

        assert!(
            slot.record(first).is_none(),
            "the first live task must be stored"
        );
        let rejected = slot
            .record(second)
            .expect("overlapping live task must be rejected");
        let _ = rejected.await;

        assert!(
            !first_dropped.load(Ordering::SeqCst),
            "the first live task must stay owned, not aborted"
        );
        assert!(
            second_dropped.load(Ordering::SeqCst),
            "the overlapping task must be aborted rather than detached"
        );

        slot.abort_and_join().await;
        assert!(
            first_dropped.load(Ordering::SeqCst),
            "shutdown must abort the task the slot kept"
        );
    }

    #[tokio::test]
    async fn record_replaces_a_finished_task() {
        let slot = ServerHandleSlot::new();
        let replacement_dropped = Arc::new(AtomicBool::new(false));

        let finished: ServerJoinHandle = tokio::spawn(async { Ok(()) });
        while !finished.is_finished() {
            tokio::task::yield_now().await;
        }
        assert!(
            slot.record(finished).is_none(),
            "a finished handle must be stored so it can be replaced"
        );

        let replacement = pending_server_task(Arc::clone(&replacement_dropped));
        tokio::task::yield_now().await;
        assert!(
            slot.record(replacement).is_none(),
            "a finished previous handle must not prevent a replacement spawn"
        );
        assert!(
            !replacement_dropped.load(Ordering::SeqCst),
            "the replacement task must stay owned"
        );
        slot.abort_and_join().await;
        assert!(replacement_dropped.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn abort_on_drop_cancels_live_task() {
        let slot = ServerHandleSlot::new();
        let dropped = Arc::new(AtomicBool::new(false));
        let handle = pending_server_task(Arc::clone(&dropped));
        tokio::task::yield_now().await;
        assert!(slot.record(handle).is_none());

        drop(AbortOnDrop::new(slot.clone()));
        tokio::task::yield_now().await;

        assert!(
            dropped.load(Ordering::SeqCst),
            "cancelling start must abort the spawned task rather than detach it"
        );
    }

    #[tokio::test]
    async fn defused_abort_on_drop_leaves_task_running() {
        let slot = ServerHandleSlot::new();
        let dropped = Arc::new(AtomicBool::new(false));
        let handle = pending_server_task(Arc::clone(&dropped));
        tokio::task::yield_now().await;
        assert!(slot.record(handle).is_none());

        AbortOnDrop::new(slot.clone()).defuse();
        tokio::task::yield_now().await;

        assert!(
            !dropped.load(Ordering::SeqCst),
            "a successful start must not abort the task it just spawned"
        );
        slot.abort_and_join().await;
    }

    /// Controlled failure: the embedded spawner runs and the daemon answers,
    /// then health fails (schema mismatch). The slot must still own the task
    /// so the start-failure path can abort it instead of detaching.
    #[cfg(unix)]
    #[tokio::test]
    async fn health_failure_after_spawn_still_owns_task() {
        let home = tempfile::tempdir().unwrap();
        let server_config = temp_server_config(home.path());
        let socket = server_config.socket_path.clone();
        let slot = ServerHandleSlot::new();
        let flag = SpawnFlag::new();
        let spawner = embedded_spawner(server_config, slot.clone(), flag.clone());

        let client = Client::with_config(ClientConfig {
            socket_path: socket.clone(),
            port_path: home.path().join("kindling.port"),
            project_root: "/tmp/kindling-runtime-test/project-a".to_string(),
            expected_schema_version: u32::MAX,
            connect_timeout: Duration::from_secs(2),
            poll_interval: Duration::from_millis(5),
            spawn: spawner,
            transport: Transport::Uds,
            spawn_log_path: None,
        });

        let err = client.health().await.expect_err("schema mismatch");
        assert!(
            matches!(err, kindling_client::ClientError::SchemaMismatch { .. }),
            "expected SchemaMismatch, got {err:?}"
        );
        assert!(flag.fired(), "embedded spawner must have run");

        let handle = slot
            .take()
            .expect("failed health must not drop the JoinHandle");
        assert!(
            !handle.is_finished(),
            "the spawned daemon must still be running after a failed probe"
        );
        handle.abort();
        let _ = handle.await;
    }

    /// Two in-flight client health calls on a cold socket both invoke the
    /// spawner. The slot must keep exactly one live task, and aborting it
    /// must stop the daemon.
    #[cfg(unix)]
    #[tokio::test]
    async fn overlapping_client_health_keeps_one_live_task() {
        let home = tempfile::tempdir().unwrap();
        let server_config = temp_server_config(home.path());
        let socket = server_config.socket_path.clone();
        let slot = ServerHandleSlot::new();
        let flag = SpawnFlag::new();
        let spawner = embedded_spawner(server_config, slot.clone(), flag);

        let client = Client::with_config(ClientConfig {
            socket_path: socket.clone(),
            port_path: home.path().join("kindling.port"),
            project_root: "/tmp/kindling-runtime-test/project-a".to_string(),
            expected_schema_version: kindling_client::EXPECTED_SCHEMA_VERSION,
            connect_timeout: Duration::from_secs(2),
            poll_interval: Duration::from_millis(5),
            spawn: spawner,
            transport: Transport::Uds,
            spawn_log_path: None,
        });
        let other = client.clone();

        let (a, b) = tokio::join!(client.health(), other.health());
        a.expect("health a");
        b.expect("health b");

        let handle = slot.take().expect("slot must own the daemon");
        assert!(
            !handle.is_finished(),
            "overlapping spawns must not replace the live task with a finished one"
        );
        handle.abort();
        let _ = handle.await;
        let _ = std::fs::remove_file(&socket);

        let attach = Client::with_config(ClientConfig {
            socket_path: socket,
            port_path: home.path().join("kindling.port"),
            project_root: "/tmp/kindling-runtime-test/project-a".to_string(),
            expected_schema_version: kindling_client::EXPECTED_SCHEMA_VERSION,
            connect_timeout: Duration::from_millis(200),
            poll_interval: Duration::from_millis(5),
            spawn: Spawner::custom(|| {
                Err(std::io::Error::new(
                    std::io::ErrorKind::NotConnected,
                    "attach-only",
                ))
            }),
            transport: Transport::Uds,
            spawn_log_path: None,
        });
        attach
            .health()
            .await
            .expect_err("aborted overlapping spawn must not leave a daemon");
    }
}
