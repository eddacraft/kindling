//! Connect-budget and launcher-lifecycle regressions for GH-182.
//!
//! The documented `connect_timeout` covers connect, spawn, and retry. Both
//! transports must start that deadline before spawn, cap each connect attempt
//! and poll sleep by the remaining time, and still fail cleanly on a short
//! budget.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use kindling_client::{Client, ClientConfig, ClientError, Spawner, Transport};

fn schema_version_u32() -> u32 {
    kindling_store::schema_version().version as u32
}

const PROJECT: &str = "/tmp/kindling-client-test/connect-budget";

#[cfg(unix)]
fn unreachable_uds_client(
    socket_path: PathBuf,
    connect_timeout: Duration,
    poll_interval: Duration,
    spawn: Spawner,
) -> Client {
    Client::with_config(ClientConfig {
        port_path: socket_path.with_extension("port"),
        socket_path,
        project_root: PROJECT.to_string(),
        expected_schema_version: schema_version_u32(),
        connect_timeout,
        poll_interval,
        spawn,
        transport: Transport::Uds,
        spawn_log_path: None,
    })
}

fn unreachable_tcp_client(
    port_path: PathBuf,
    connect_timeout: Duration,
    poll_interval: Duration,
    spawn: Spawner,
) -> Client {
    Client::with_config(ClientConfig {
        socket_path: port_path.with_extension("sock"),
        port_path,
        project_root: PROJECT.to_string(),
        expected_schema_version: schema_version_u32(),
        connect_timeout,
        poll_interval,
        spawn,
        transport: Transport::Tcp,
        spawn_log_path: None,
    })
}

fn assert_unavailable_within(
    elapsed: Duration,
    ceiling: Duration,
    outcome: Result<impl std::fmt::Debug, ClientError>,
) {
    assert!(
        elapsed < ceiling,
        "connect budget overran: elapsed {elapsed:?} (ceiling {ceiling:?})"
    );
    match outcome {
        Err(ClientError::Unavailable(_)) => {}
        other => panic!("expected Unavailable, got {other:?}"),
    }
}

#[cfg(unix)]
#[tokio::test]
async fn uds_short_budget_does_not_sleep_full_poll_interval() {
    let dir = tempfile::tempdir().unwrap();
    let client = unreachable_uds_client(
        dir.path().join("missing.sock"),
        Duration::from_millis(25),
        Duration::from_millis(200),
        Spawner::custom(|| Ok(())),
    );

    let start = Instant::now();
    let outcome = client.health().await;
    assert_unavailable_within(start.elapsed(), Duration::from_millis(150), outcome);
}

#[tokio::test]
async fn tcp_short_budget_does_not_sleep_full_poll_interval() {
    let dir = tempfile::tempdir().unwrap();
    let client = unreachable_tcp_client(
        dir.path().join("missing.port"),
        Duration::from_millis(25),
        Duration::from_millis(200),
        Spawner::custom(|| Ok(())),
    );

    let start = Instant::now();
    let outcome = client.health().await;
    assert_unavailable_within(start.elapsed(), Duration::from_millis(150), outcome);
}

#[cfg(unix)]
#[tokio::test]
async fn uds_slow_spawn_counts_against_connect_budget() {
    let dir = tempfile::tempdir().unwrap();
    let client = unreachable_uds_client(
        dir.path().join("missing.sock"),
        Duration::from_millis(40),
        Duration::from_millis(300),
        Spawner::custom(|| {
            std::thread::sleep(Duration::from_millis(120));
            Ok(())
        }),
    );

    let start = Instant::now();
    let outcome = client.health().await;
    let elapsed = start.elapsed();
    assert!(
        elapsed >= Duration::from_millis(80),
        "spawn should have run: {elapsed:?}"
    );
    assert_unavailable_within(elapsed, Duration::from_millis(300), outcome);
}

#[tokio::test]
async fn tcp_slow_spawn_counts_against_connect_budget() {
    let dir = tempfile::tempdir().unwrap();
    let client = unreachable_tcp_client(
        dir.path().join("missing.port"),
        Duration::from_millis(40),
        Duration::from_millis(300),
        Spawner::custom(|| {
            std::thread::sleep(Duration::from_millis(120));
            Ok(())
        }),
    );

    let start = Instant::now();
    let outcome = client.health().await;
    let elapsed = start.elapsed();
    assert!(
        elapsed >= Duration::from_millis(80),
        "spawn should have run: {elapsed:?}"
    );
    assert_unavailable_within(elapsed, Duration::from_millis(300), outcome);
}
