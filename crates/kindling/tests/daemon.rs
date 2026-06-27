//! Daemon-mode smoke tests.
//!
//! 1. `serve_binds_and_idle_shuts_down` — `kindling-cli serve` binds the UDS and
//!    cleanly idle-shuts-down within a short timeout (the deviation from the TS
//!    HTTP `serve`).
//! 2. `via_daemon_log_roundtrip` — with a daemon already running on a temp
//!    socket under a temp `HOME`, `kindling-cli --via-daemon log …` routes the
//!    append through the client to the daemon, and the row lands in the
//!    per-project DB the daemon owns.

mod support;

use std::process::Command;
use std::time::Duration;

use kindling_server::{serve, ServerConfig};
use support::{assert_success, json_stdout, stderr, stdout};

/// `serve` should bind the socket and return cleanly once idle.
#[test]
fn serve_binds_and_idle_shuts_down() {
    let home = tempfile::tempdir().unwrap();
    let socket = home.path().join("k.sock");

    let bin = env!("CARGO_BIN_EXE_kindling");
    let mut child = Command::new(bin)
        .args([
            "serve",
            "--socket",
            socket.to_string_lossy().as_ref(),
            "--kindling-home",
            home.path().to_string_lossy().as_ref(),
            "--idle-timeout",
            "1",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn serve");

    // Wait for the socket to appear (daemon bound).
    let mut bound = false;
    for _ in 0..400 {
        if socket.exists() {
            bound = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(bound, "daemon never bound the socket");

    // It should idle-shut-down on its own (idle timeout 1s) within a bound.
    let status = wait_with_timeout(&mut child, Duration::from_secs(10));
    assert!(status.is_some(), "serve did not idle-shut-down in time");
    assert!(status.unwrap().success(), "serve exited non-zero");
}

/// `--via-daemon log` against a running daemon writes to the daemon's DB.
#[tokio::test(flavor = "multi_thread")]
async fn via_daemon_log_roundtrip() {
    let home = tempfile::tempdir().unwrap();
    // The client resolves its socket as `$HOME/.kindling/kindling.sock`, so the
    // daemon's kindling home must be `<HOME>/.kindling` for the two to meet.
    let home_path = home.path().join(".kindling");
    std::fs::create_dir_all(&home_path).unwrap();
    let socket = home_path.join("kindling.sock"); // default name the client expects

    // Start an in-process daemon rooted at this temp home.
    let config = ServerConfig {
        socket_path: socket.clone(),
        kindling_home: home_path.clone(),
        pid_path: home_path.join("kindling.pid"),
        port_path: home_path.join("kindling.port"),
        idle_timeout: Duration::from_secs(3600),
        transport: kindling_server::Transport::default(),
    };
    let handle = tokio::spawn(async move { serve(config).await });

    // Wait for the socket.
    for _ in 0..400 {
        if socket.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(socket.exists(), "daemon socket never appeared");

    // Run the CLI with HOME pointed at the temp home so the client's default
    // socket path resolves to our daemon. The project root is the CLI process's
    // cwd; the daemon hashes it to a per-project DB under <home>/projects/.
    let bin = env!("CARGO_BIN_EXE_kindling");
    let project_root = std::env::current_dir().unwrap();
    let cli_home = home.path().to_path_buf();
    let out = tokio::task::spawn_blocking({
        let cli_home = cli_home.clone();
        let project_root = project_root.clone();
        move || {
            Command::new(bin)
                .args(["--via-daemon", "log", "daemon hello", "--json"])
                .env("HOME", &cli_home)
                .current_dir(&project_root)
                .output()
                .expect("run cli")
        }
    })
    .await
    .unwrap();

    assert_success(&out);
    let v = json_stdout(&out);
    assert_eq!(v["content"], serde_json::json!("daemon hello"));
    assert_eq!(v["provenance"]["source"], serde_json::json!("cli"));

    // The row landed in the daemon's per-project DB: verify via a `status`
    // against that DB file (in-process, no daemon).
    let project_id = kindling_store::project_id(project_root.to_string_lossy().as_ref());
    let db_path = home_path
        .join("projects")
        .join(&project_id)
        .join("kindling.db");
    assert!(
        db_path.exists(),
        "daemon did not create the project DB: {db_path:?}"
    );

    let status = Command::new(bin)
        .args([
            "status",
            "--json",
            "--db",
            db_path.to_string_lossy().as_ref(),
        ])
        .output()
        .expect("run status");
    let sv: serde_json::Value = serde_json::from_str(&stdout(&status)).unwrap_or_else(|_| {
        panic!(
            "status stdout not JSON: {}\nstderr: {}",
            stdout(&status),
            stderr(&status)
        )
    });
    assert_eq!(sv["counts"]["observations"], serde_json::json!(1));

    handle.abort();
}

/// `--via-daemon <cmd> --db <path>` is rejected for every daemon-backed verb.
///
/// The daemon routes per project root (hashed into a per-project DB), not by an
/// explicit DB file, so honouring `--db` over the daemon is impossible. Rather
/// than silently mis-route to the cwd's project DB, the CLI rejects the
/// combination with a clear, non-zero-exit error mentioning both flags. This
/// rejection happens in `build_client` before any socket connection, so no
/// running daemon is required.
#[test]
fn via_daemon_with_db_is_rejected() {
    let bin = env!("CARGO_BIN_EXE_kindling");
    let db = tempfile::tempdir().unwrap().path().join("explicit.db");
    let db = db.to_string_lossy().into_owned();

    // One representative invocation per daemon-backed verb. Each must fail
    // cleanly (non-zero) with an error naming both `--db` and `--via-daemon`,
    // and must NOT attempt to spawn/contact a daemon.
    let invocations: Vec<Vec<&str>> = vec![
        vec!["--via-daemon", "log", "hello", "--db", &db],
        vec![
            "--via-daemon",
            "capsule",
            "open",
            "--intent",
            "x",
            "--db",
            &db,
        ],
        vec!["--via-daemon", "capsule", "close", "cap_1", "--db", &db],
        vec!["--via-daemon", "search", "query", "--db", &db],
        vec!["--via-daemon", "pin", "observation", "obs_1", "--db", &db],
        vec!["--via-daemon", "unpin", "pin_1", "--db", &db],
        vec!["--via-daemon", "forget", "obs_1", "--db", &db],
    ];

    for args in invocations {
        let out = Command::new(bin).args(&args).output().expect("run cli");
        assert!(
            !out.status.success(),
            "expected non-zero exit for {args:?}, got success\nstdout: {}\nstderr: {}",
            stdout(&out),
            stderr(&out)
        );
        let err = stderr(&out);
        assert!(
            err.contains("--db") && err.contains("--via-daemon"),
            "error for {args:?} should mention both --db and --via-daemon, got: {err}"
        );
    }
}

/// Wait for a child to exit within `timeout`, returning its status (or `None`
/// on timeout, after killing it).
fn wait_with_timeout(
    child: &mut std::process::Child,
    timeout: Duration,
) -> Option<std::process::ExitStatus> {
    let start = std::time::Instant::now();
    loop {
        match child.try_wait().expect("try_wait") {
            Some(status) => return Some(status),
            None => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}
