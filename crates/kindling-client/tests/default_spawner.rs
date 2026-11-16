//! Default `Spawner::Command` with a missing daemon binary.
//!
//! This test sets `KINDLING_BIN`, which mutates process-wide state. It lives in
//! its own test binary, as the only test, so nothing else runs alongside it while
//! the environment changes: mutating the environment while other threads read it
//! is unsound (which is why `std::env::set_var` is `unsafe` from edition 2024).
//! Do not add tests to this file.
//!
//! UDS-specific, like `tests/client.rs`, so Unix-only.
#![cfg(unix)]

use std::time::Duration;

use kindling_client::{Client, ClientConfig, ClientError, Spawner, Transport};

/// Default `Spawner::Command` execs the `kindling` binary; when that binary is
/// missing the call must still fail cleanly with Unavailable, not panic or hang.
/// The binary is overridden via `KINDLING_BIN` so the missing-binary path is
/// exercised deterministically, even when a real `kindling` is on `PATH`.
#[test]
fn default_spawner_missing_binary_is_unavailable() {
    let dir = tempfile::tempdir().unwrap();

    // Set before the runtime starts, while this is the only thread that could
    // read the environment. An empty temp dir has no such entry, so
    // `Command::spawn` fails with NotFound.
    std::env::set_var("KINDLING_BIN", dir.path().join("kindling-missing-bin"));

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build runtime");

    let outcome = runtime
        .block_on(async {
            let client = Client::with_config(ClientConfig {
                socket_path: dir.path().join("nonexistent.sock"),
                port_path: dir.path().join("nonexistent.port"),
                project_root: "/tmp/kindling-client-test/project-a".to_string(),
                expected_schema_version: kindling_store::schema_version().version as u32,
                connect_timeout: Duration::from_millis(200),
                poll_interval: Duration::from_millis(10),
                spawn: Spawner::Command, // execs $KINDLING_BIN (absent), not the real binary
                transport: Transport::Uds,
                spawn_log_path: None,
            });
            tokio::time::timeout(Duration::from_secs(3), client.health()).await
        })
        .expect("call must not hang");
    assert!(
        matches!(outcome, Err(ClientError::Unavailable(_))),
        "expected Unavailable, got {outcome:?}"
    );
}
