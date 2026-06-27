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

/// Send one blocking HTTP/1.1 request over loopback TCP and return the response
/// status code. Deliberately raw (no client crate) so the test controls exactly
/// which headers are present — in particular whether the bearer token is sent.
fn raw_status(
    port: u16,
    method: &str,
    path: &str,
    auth: Option<&str>,
    project: Option<&str>,
    body: &str,
) -> u16 {
    use std::io::{Read, Write};
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connect");
    let mut req = format!(
        "{method} {path} HTTP/1.1\r\nhost: kindling.local\r\n\
         content-type: application/json\r\nconnection: close\r\n"
    );
    if let Some(p) = project {
        req.push_str(&format!("x-kindling-project: {p}\r\n"));
    }
    if let Some(a) = auth {
        req.push_str(&format!("authorization: Bearer {a}\r\n"));
    }
    req.push_str(&format!("content-length: {}\r\n\r\n{}", body.len(), body));
    stream.write_all(req.as_bytes()).expect("write request");
    let mut resp = Vec::new();
    stream.read_to_end(&mut resp).expect("read response");
    let text = String::from_utf8_lossy(&resp);
    let status_line = text.lines().next().unwrap_or("");
    status_line
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or_else(|| panic!("no status in response: {text:?}"))
}

/// Poll a side-channel file until it has content (the daemon writes the port
/// after bind and the token before bind), within a short budget.
fn wait_for_file(path: &std::path::Path) -> String {
    for _ in 0..200 {
        if let Ok(s) = std::fs::read_to_string(path) {
            if !s.trim().is_empty() {
                return s.trim().to_string();
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("file never appeared: {}", path.display());
}

/// The loopback-TCP transport requires a per-daemon bearer secret on every
/// non-health request; `X-Kindling-Project` alone is never authorization
/// (KINTEG-010).
#[test]
fn tcp_transport_requires_bearer_secret() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().to_path_buf();
    let port_path = home.join("kindling.port");
    let token_path = home.join("kindling.token");

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();

    let server_config = ServerConfig {
        socket_path: home.join("unused.sock"),
        kindling_home: home.clone(),
        pid_path: home.join("kindling.pid"),
        port_path: port_path.clone(),
        idle_timeout: Duration::from_secs(60),
        transport: ServerTransport::Tcp,
    };
    let _server = runtime.spawn(async move { serve(server_config).await });

    let port: u16 = wait_for_file(&port_path).parse().expect("port is a number");
    let token = wait_for_file(&token_path);

    let body = r#"{"kind":"message","content":"hi","scopeIds":{"sessionId":"s1"}}"#;

    // No bearer token → 401, even with a project header.
    assert_eq!(
        raw_status(
            port,
            "POST",
            "/v1/observations",
            None,
            Some("/proj/a"),
            body
        ),
        401,
        "data request without the secret must be rejected"
    );

    // A bogus project header without the secret still cannot read another
    // project's memory — the project header is routing, not authorization.
    assert_eq!(
        raw_status(
            port,
            "POST",
            "/v1/observations/list",
            None,
            Some("/proj/victim"),
            r#"{"scopeIds":{}}"#
        ),
        401,
        "project header alone must not authorize access"
    );

    // The correct bearer token is accepted.
    assert_eq!(
        raw_status(
            port,
            "POST",
            "/v1/observations",
            Some(&token),
            Some("/proj/a"),
            body
        ),
        201,
        "data request with the secret must be accepted"
    );

    // A wrong token is rejected.
    assert_eq!(
        raw_status(
            port,
            "POST",
            "/v1/observations",
            Some("deadbeef"),
            Some("/proj/a"),
            body
        ),
        401,
        "a wrong secret must be rejected"
    );

    // Health needs no token (the contract-drift probe stays open).
    assert_eq!(
        raw_status(port, "GET", "/v1/health", None, None, ""),
        200,
        "health must not require the secret"
    );

    // The token file is owner-only on Unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&token_path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "token file must be 0600");
    }
}

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
