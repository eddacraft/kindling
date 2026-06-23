//! Platform-agnostic integration test for the loopback-TCP transport.
//!
//! This is the Linux validation of the Windows code path: the daemon binds an
//! ephemeral `127.0.0.1` port and publishes it to a port file; the client reads
//! the port file, connects over TCP, and speaks the v1 HTTP/1 contract. The
//! same `Transport::Tcp` path is what real Windows uses by default (covered on
//! Windows by the `windows-tcp` CI job), so exercising it here proves the full
//! transport end-to-end without needing a Windows runner for the happy path.

use std::time::Duration;

use kindling_client::{CapsuleType, Client, ClientConfig, ScopeIds, Spawner, Transport};
use kindling_server::{serve, ServerConfig, Transport as ServerTransport};

/// The store's canonical schema version, as `u32` (the client's type).
fn schema_version_u32() -> u32 {
    kindling_store::schema_version().version as u32
}

#[test]
fn tcp_transport_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().to_path_buf();
    let port_path = home.join("kindling.port");

    // Dedicated multi-thread runtime owned by the test so the daemon and the
    // client requests run concurrently.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();

    // Start the daemon on the TCP transport. It binds 127.0.0.1:0 and writes
    // the bound port to `port_path`.
    let server_config = ServerConfig {
        socket_path: home.join("unused.sock"),
        kindling_home: home.clone(),
        pid_path: home.join("kindling.pid"),
        port_path: port_path.clone(),
        idle_timeout: Duration::from_secs(60),
        transport: ServerTransport::Tcp,
    };
    let _server = runtime.spawn(async move { serve(server_config).await });

    // The client discovers the port via the port file. No auto-spawn needed
    // (the server is already running); the connect poll covers the brief window
    // before the daemon publishes its port.
    let client = Client::with_config(ClientConfig {
        socket_path: home.join("unused.sock"),
        port_path,
        project_root: "/proj/a".to_string(),
        expected_schema_version: schema_version_u32(),
        connect_timeout: Duration::from_secs(2),
        poll_interval: Duration::from_millis(10),
        // The daemon is already starting on the test runtime; a no-op spawner
        // lets the connect loop poll for the port file the daemon publishes
        // once it binds (rather than failing on the real-binary spawn path).
        spawn: Spawner::custom(|| Ok(())),
        transport: Transport::Tcp,
        spawn_log_path: None,
    });

    runtime.block_on(async {
        // Health: proves port-file discovery + TCP connect + HTTP/1 exchange.
        let health = client.health().await.expect("health over TCP");
        assert_eq!(health.schema_version, schema_version_u32());

        // A data endpoint requiring the project header, end-to-end over TCP.
        let capsule = client
            .open_capsule(
                CapsuleType::Session,
                "tcp round trip",
                ScopeIds {
                    session_id: Some("s1".to_string()),
                    ..Default::default()
                },
                None,
            )
            .await
            .expect("open_capsule over TCP");
        assert_eq!(capsule.intent, "tcp round trip");
        assert_eq!(capsule.kind, CapsuleType::Session);
    });
}
