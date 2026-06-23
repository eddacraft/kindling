//! Integration tests for `kindling-runtime` against a real (in-process) daemon.
//!
//! UDS-specific (the runtime's embedded daemon binds a Unix domain socket), so
//! the suite is Unix-only — mirroring `kindling-client`'s `tests/client.rs`.
#![cfg(unix)]

use std::path::PathBuf;
use std::time::Duration;

use kindling_runtime::types::{ObservationInput, ObservationKind, RetrieveOptions, ScopeIds};
use kindling_runtime::{Runtime, RuntimeConfig, SpawnStrategy};
use kindling_server::{serve, ServerConfig};
use tempfile::TempDir;
use tokio::task::JoinHandle;

const PROJECT: &str = "/tmp/kindling-runtime-test/project-a";

/// The store's canonical schema version (the daemon reports this on health).
fn schema_version_u32() -> u32 {
    kindling_store::schema_version().version as u32
}

fn message_input(content: &str) -> ObservationInput {
    ObservationInput {
        id: None,
        kind: ObservationKind::Message,
        content: content.to_string(),
        provenance: None,
        ts: None,
        scope_ids: ScopeIds {
            session_id: Some("s1".to_string()),
            ..Default::default()
        },
        redacted: None,
    }
}

/// A pre-started daemon on a temp home, used to exercise the attach path.
struct PreStartedDaemon {
    kindling_home: PathBuf,
    _home: TempDir,
    handle: JoinHandle<Result<(), kindling_server::ServerError>>,
}

impl PreStartedDaemon {
    async fn start() -> Self {
        let home = tempfile::tempdir().unwrap();
        let home_path = home.path().to_path_buf();
        let config = ServerConfig {
            socket_path: home_path.join("kindling.sock"),
            kindling_home: home_path.clone(),
            pid_path: home_path.join("kindling.pid"),
            port_path: home_path.join("kindling.port"),
            idle_timeout: Duration::from_secs(3600),
            transport: kindling_server::Transport::default(),
        };
        let socket_path = config.socket_path.clone();
        let handle = tokio::spawn(async move { serve(config).await });
        wait_for_socket(&socket_path).await;
        Self {
            kindling_home: home_path,
            _home: home,
            handle,
        }
    }
}

async fn wait_for_socket(socket_path: &std::path::Path) {
    for _ in 0..400 {
        if socket_path.exists() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("daemon socket never appeared: {}", socket_path.display());
}

/// 1. Cold embedded start: a temp home with no daemon → `Runtime::start` with
/// `Embedded` spawns the in-process daemon, health is OK (schema matches), and
/// the embedded spawner actually ran.
#[tokio::test]
async fn cold_embedded_start_spawns_and_health_ok() {
    let home = tempfile::tempdir().unwrap();
    let config = RuntimeConfig::with_home(home.path(), PROJECT, SpawnStrategy::Embedded);

    assert!(
        !home.path().join("kindling.sock").exists(),
        "socket must not pre-exist"
    );

    let runtime = Runtime::start(config).await.expect("cold embedded start");

    // Health round-trips and the schema version matches the store's.
    let health = runtime.client().health().await.expect("health");
    assert_eq!(health.schema_version, schema_version_u32());

    // The embedded spawner actually started a daemon (it was a cold start).
    assert!(
        runtime.spawned_embedded_daemon(),
        "embedded spawner should have run on a cold start"
    );

    runtime.shutdown().await.expect("shutdown");
}

/// 2. Attach to a pre-running daemon: start a daemon first, then `Runtime::start`
/// with `AttachOnly` on the SAME home/socket → it connects and the spawner is
/// NOT invoked.
#[tokio::test]
async fn attach_only_connects_without_spawning() {
    let daemon = PreStartedDaemon::start().await;

    let config = RuntimeConfig::with_home(
        daemon.kindling_home.clone(),
        PROJECT,
        SpawnStrategy::AttachOnly,
    );

    let runtime = Runtime::start(config)
        .await
        .expect("attach to pre-running daemon");

    let health = runtime.client().health().await.expect("health via attach");
    assert_eq!(health.schema_version, schema_version_u32());

    // AttachOnly: the spawner must never have fired (it would have errored).
    assert!(
        !runtime.spawned_embedded_daemon(),
        "AttachOnly must not invoke the spawner against a running daemon"
    );

    // Shutting down the runtime must NOT stop the externally-managed daemon.
    runtime.shutdown().await.expect("shutdown");
    assert!(
        !daemon.handle.is_finished(),
        "attached daemon must survive runtime shutdown"
    );
}

/// 2b. Embedded against a pre-running daemon also attaches (the client only
/// spawns when the socket does not answer), leaving the spawner unfired.
#[tokio::test]
async fn embedded_attaches_to_existing_daemon_without_spawning() {
    let daemon = PreStartedDaemon::start().await;

    let config = RuntimeConfig::with_home(
        daemon.kindling_home.clone(),
        PROJECT,
        SpawnStrategy::Embedded,
    );

    let runtime = Runtime::start(config).await.expect("embedded attach");
    runtime.client().health().await.expect("health");

    assert!(
        !runtime.spawned_embedded_daemon(),
        "Embedded must attach (not spawn) when a daemon already listens"
    );

    runtime.shutdown().await.expect("shutdown");
    assert!(
        !daemon.handle.is_finished(),
        "pre-existing daemon must survive runtime shutdown when only attached"
    );
}

/// 3. Spooled append round-trip: append via `spooled_client()` → the observation
/// is delivered to the daemon and readable back via `client().retrieve`.
#[cfg(feature = "spool")]
#[tokio::test]
async fn spooled_append_round_trip() {
    use kindling_runtime::AppendOutcome;

    let home = tempfile::tempdir().unwrap();
    let config = RuntimeConfig::with_home(home.path(), PROJECT, SpawnStrategy::Embedded);
    let runtime = Runtime::start(config).await.expect("start");

    let outcome = runtime
        .spooled_client()
        .append_observation(
            message_input("durable needle through the runtime"),
            None,
            None,
        )
        .await
        .expect("spooled append");

    // Daemon is up → delivered straight through (not buffered to the spool).
    assert!(
        matches!(outcome, AppendOutcome::Delivered(_)),
        "expected Delivered, got {outcome:?}"
    );

    // The spool drained to empty (nothing buffered).
    assert_eq!(
        runtime.spooled_client().pending_count().expect("pending"),
        0,
        "no entries should remain spooled after a successful delivery"
    );

    // Read it back through the daemon to prove durable end-to-end emit.
    let result = runtime
        .client()
        .retrieve(RetrieveOptions {
            query: "needle".to_string(),
            scope_ids: ScopeIds {
                session_id: Some("s1".to_string()),
                ..Default::default()
            },
            token_budget: None,
            max_candidates: None,
            include_redacted: None,
        })
        .await
        .expect("retrieve");

    assert!(
        result.candidates.iter().any(
            |c| matches!(&c.entity, kindling_runtime::types::RetrievedEntity::Observation(o)
                if o.content == "durable needle through the runtime")
        ),
        "appended observation must surface via retrieve: {result:#?}"
    );

    runtime.shutdown().await.expect("shutdown");
}

/// 4. `shutdown()` cleanly stops the embedded daemon (the task ends and the
/// socket stops answering).
#[tokio::test]
async fn shutdown_stops_embedded_daemon() {
    let home = tempfile::tempdir().unwrap();
    let socket = home.path().join("kindling.sock");
    let config = RuntimeConfig::with_home(home.path(), PROJECT, SpawnStrategy::Embedded);

    let runtime = Runtime::start(config).await.expect("start");
    assert!(runtime.spawned_embedded_daemon());
    assert!(socket.exists(), "embedded daemon should bind the socket");

    runtime.shutdown().await.expect("shutdown");

    // After abort the daemon no longer answers: a fresh AttachOnly runtime on
    // the same home must fail to connect (the socket is dead / no daemon).
    let attach = RuntimeConfig::with_home(home.path(), PROJECT, SpawnStrategy::AttachOnly);
    let err = Runtime::start(attach)
        .await
        .expect_err("no daemon should answer after shutdown");
    assert!(
        matches!(err, kindling_runtime::RuntimeError::Client(_)),
        "expected a client/unavailable error after shutdown, got {err:?}"
    );
}
