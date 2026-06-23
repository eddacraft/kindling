//! End-to-end: SpooledClient outage → flush writes sidecar → `kindling spool status`.

#![cfg(unix)]

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use kindling_client::spool::{AppendOutcome, SpooledClient};
use kindling_client::{Client, ClientConfig, Spawner, Transport};
use kindling_server::{serve, ServerConfig};
use kindling_types::{ObservationInput, ObservationKind, ScopeIds};
use tempfile::TempDir;
use tokio::task::JoinHandle;

const PROJECT: &str = "/tmp/kindling-spool-cli-test/project";

fn message_input(content: &str) -> ObservationInput {
    ObservationInput {
        id: None,
        kind: ObservationKind::Message,
        content: content.to_string(),
        provenance: None,
        ts: None,
        scope_ids: ScopeIds {
            session_id: Some("s1".to_string()),
            repo_id: Some(PROJECT.to_string()),
            ..Default::default()
        },
        redacted: None,
    }
}

fn schema_version_u32() -> u32 {
    kindling_store::schema_version().version as u32
}

struct TestDaemon {
    socket_path: PathBuf,
    _home: TempDir,
    _handle: JoinHandle<Result<(), kindling_server::ServerError>>,
}

impl TestDaemon {
    async fn start() -> Self {
        let home = tempfile::tempdir().unwrap();
        let home_path = home.path().to_path_buf();
        let socket_path = home_path.join("k.sock");
        let config = ServerConfig {
            socket_path: socket_path.clone(),
            kindling_home: home_path.clone(),
            pid_path: home_path.join("k.pid"),
            port_path: home_path.join("k.port"),
            idle_timeout: Duration::from_secs(3600),
            transport: kindling_server::Transport::default(),
        };
        let handle = tokio::spawn(async move { serve(config).await });
        for _ in 0..400 {
            if socket_path.exists() {
                return Self {
                    socket_path,
                    _home: home,
                    _handle: handle,
                };
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("daemon socket never appeared");
    }

    fn client(&self, project_root: &str) -> Client {
        let cfg = ClientConfig {
            socket_path: self.socket_path.clone(),
            port_path: PathBuf::from("unused.port"),
            project_root: project_root.to_string(),
            expected_schema_version: schema_version_u32(),
            connect_timeout: Duration::from_secs(2),
            poll_interval: Duration::from_millis(10),
            spawn: Spawner::custom(|| panic!("spawner must not be called when daemon is running")),
            transport: Transport::default(),
            spawn_log_path: None,
        };
        Client::with_config(cfg)
    }
}

fn down_client(socket_path: PathBuf, project_root: &str) -> Client {
    let cfg = ClientConfig {
        socket_path,
        port_path: PathBuf::from("unused.port"),
        project_root: project_root.to_string(),
        expected_schema_version: schema_version_u32(),
        connect_timeout: Duration::from_millis(150),
        poll_interval: Duration::from_millis(10),
        spawn: Spawner::custom(|| {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "kindling binary not found (simulated daemon-down)",
            ))
        }),
        transport: Transport::default(),
        spawn_log_path: None,
    };
    Client::with_config(cfg)
}

#[tokio::test]
async fn spool_status_cli_reads_sidecar_after_real_flush() {
    let work = tempfile::tempdir().unwrap();
    let socket = work.path().join("nope.sock");
    let spool_path = work.path().join("spool.ndjson");
    let spool_str = spool_path.to_string_lossy().into_owned();

    // 1. Spool while daemon is down — sidecar records connectivity error.
    let down = SpooledClient::new(down_client(socket, PROJECT), spool_path.clone());
    let outcome = down
        .append_observation(message_input("cli e2e probe"), None, None)
        .await
        .expect("spool while down");
    assert!(matches!(outcome, AppendOutcome::Spooled));
    assert_eq!(down.pending_count().unwrap(), 1);
    drop(down);

    // 2. Flush with daemon up — sidecar records successful flush + replay count.
    let daemon = TestDaemon::start().await;
    let spooled = SpooledClient::new(daemon.client(PROJECT), spool_path.clone());
    spooled.flush().await.expect("flush after outage");
    drop(spooled);

    // 3. CLI subprocess reads the spool + sidecar SpooledClient wrote.
    let bin = env!("CARGO_BIN_EXE_kindling");
    let run_cli = || {
        let out = Command::new(bin)
            .args(["spool", "status", "--spool-path", &spool_str, "--json"])
            .output()
            .expect("run kindling spool status");
        assert!(
            out.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice::<serde_json::Value>(&out.stdout).expect("cli json")
    };

    let first = run_cli();
    let second = run_cli();

    if let Ok(scratch) = std::env::var("GROK_SCRATCH") {
        let dir = std::path::Path::new(&scratch);
        let _ = std::fs::create_dir_all(dir);
        let write = |name: &str, v: &serde_json::Value| {
            std::fs::write(
                dir.join(name),
                serde_json::to_string_pretty(v).expect("json"),
            )
            .expect("write scratch artifact");
        };
        write("spool-cli.json", &first);
        write("spool-cli2.json", &second);
        write(
            "spool-cli-help.json",
            &serde_json::json!({ "note": "see spool-cli-help.out from cargo run --help" }),
        );
    }

    assert_eq!(first["pendingCount"], serde_json::json!(0));
    assert_eq!(first["spoolPath"], serde_json::json!(spool_str));
    assert!(
        first["lastFlushTimeMs"].is_number(),
        "flush timestamp: {first:#}"
    );
    assert!(first["lastError"].is_null());
    assert!(
        first["replayAttempts"].as_u64().unwrap_or(0) >= 1,
        "replay attempts: {first:#}"
    );

    assert_eq!(second["pendingCount"], first["pendingCount"]);
    assert_eq!(second["lastFlushTimeMs"], first["lastFlushTimeMs"]);
    assert_eq!(second["replayAttempts"], first["replayAttempts"]);
}
