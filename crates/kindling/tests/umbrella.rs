//! Integration tests for the umbrella `kindling` binary — all three surfaces
//! (CLI, hook, serve) plus the `kindling-hook` symlink drop-in, driven through
//! the real executable via `CARGO_BIN_EXE_kindling`.
//!
//! The hook + serve surfaces stand up an in-process `kindling-server` daemon on
//! a temp Unix socket under a temp kindling home (the same harness the
//! `kindling-hook` e2e tests use), then invoke the umbrella with `KINDLING_SOCK`
//! pointed at it and assert the observation lands in the daemon's per-project
//! DB. DB routing is pinned with `KINDLING_REPO_ROOT` so it is deterministic.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use kindling_store::{project_db_path, SqliteKindlingStore};
use kindling_types::{ObservationKind, ScopeIds};
use serde_json::{json, Value};
use tempfile::TempDir;

/// Fixed project root for the hook surfaces (set as both `KINDLING_REPO_ROOT`
/// and the hook `cwd`, so the daemon routes to one deterministic per-project
/// DB).
const REPO_ROOT: &str = "/tmp/kindling-umbrella-e2e/repo";
const SESSION: &str = "sess-umbrella";

/// Path to the umbrella binary under test.
fn kindling_bin() -> &'static str {
    env!("CARGO_BIN_EXE_kindling")
}

// ---------------------------------------------------------------------------
// (1) CLI surface
// ---------------------------------------------------------------------------

/// `kindling --version` → exit 0 and a version line. This is the PORT-010 CI
/// smoke contract for the merged release; assert it explicitly.
#[test]
fn version_exits_zero_with_version_line() {
    let out = Command::new(kindling_bin())
        .arg("--version")
        .output()
        .expect("run kindling --version");
    assert!(
        out.status.success(),
        "`kindling --version` must exit 0, got {:?}",
        out.status.code()
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.starts_with("kindling ") && stdout.trim().len() > "kindling ".len(),
        "expected a `kindling <version>` line, got {stdout:?}"
    );
}

/// A CLI verb routes through clap to the in-process service: `log` then
/// `status --json` against a temp DB returns exit 0 and valid JSON with the row.
#[test]
fn cli_log_then_status_json() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("kindling.db");
    let db = db.to_string_lossy().into_owned();

    let log = Command::new(kindling_bin())
        .args(["log", "umbrella hello", "--db", &db, "--json"])
        .output()
        .expect("run kindling log");
    assert!(
        log.status.success(),
        "log failed: {}",
        String::from_utf8_lossy(&log.stderr)
    );

    let status = Command::new(kindling_bin())
        .args(["status", "--db", &db, "--json"])
        .output()
        .expect("run kindling status");
    assert!(status.status.success(), "status should exit 0");
    let v: Value = serde_json::from_str(&String::from_utf8_lossy(&status.stdout))
        .expect("status stdout is JSON");
    assert_eq!(v["counts"]["observations"], json!(1));
}

/// An unrecognized verb is a clap usage error → non-zero exit.
#[test]
fn cli_bad_verb_exits_nonzero() {
    let out = Command::new(kindling_bin())
        .arg("definitely-not-a-verb")
        .output()
        .expect("run kindling bad verb");
    assert!(
        !out.status.success(),
        "a bad verb must exit non-zero, got {:?}",
        out.status.code()
    );
}

// ---------------------------------------------------------------------------
// (2) + (3) Hook surface + symlink drop-in
// ---------------------------------------------------------------------------

/// `kindling hook post-tool-use` lands an observation in the daemon DB and
/// exits 0.
#[test]
fn hook_subcommand_lands_observation() {
    let daemon = Daemon::start();
    let ctx = post_tool_use_ctx();

    let out = daemon.run_umbrella(kindling_bin(), &["hook", "post-tool-use"], &ctx);
    assert!(out.status.success(), "hook should exit 0");

    assert_single_command_observation(&daemon);
}

/// `kindling hook <bad-type>` never blocks: exit 0 and the error is logged to
/// stderr in the Node format.
#[test]
fn hook_subcommand_bad_type_exits_zero_and_logs() {
    let daemon = Daemon::start();
    let out = daemon.run_umbrella(kindling_bin(), &["hook", "not-a-hook"], "{}");
    assert!(out.status.success(), "bad hook type must still exit 0");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("[kindling]") && stderr.contains("error:"),
        "expected a Node-format error log on stderr, got {stderr:?}"
    );
}

/// Symlink drop-in: a `kindling-hook` symlink → the umbrella binary behaves as
/// `kindling hook`, taking the type from argv[1]. Lands the observation.
#[test]
fn symlink_kindling_hook_behaves_as_hook() {
    let daemon = Daemon::start();

    // Create a `kindling-hook` symlink next to a temp dir pointing at the real
    // umbrella binary, then invoke it directly so argv[0]'s basename is
    // `kindling-hook`.
    let link_dir = tempfile::tempdir().unwrap();
    let link = link_dir.path().join("kindling-hook");
    symlink_or_copy(Path::new(kindling_bin()), &link);

    let ctx = post_tool_use_ctx();
    // Note: no "hook" arg — the symlink name selects hook mode, argv[1] = type.
    let out = daemon.run_umbrella(link.to_string_lossy().as_ref(), &["post-tool-use"], &ctx);
    assert!(out.status.success(), "symlink hook should exit 0");

    assert_single_command_observation(&daemon);
}

// ---------------------------------------------------------------------------
// (4) Serve surface
// ---------------------------------------------------------------------------

/// `kindling serve` (a CLI verb routed through clap) binds the UDS and
/// idle-shuts-down within a short bound, exiting 0.
#[test]
fn serve_binds_and_idle_shuts_down() {
    let home = tempfile::tempdir().unwrap();
    let socket = home.path().join("k.sock");

    let mut child = Command::new(kindling_bin())
        .args([
            "serve",
            "--socket",
            socket.to_string_lossy().as_ref(),
            "--kindling-home",
            home.path().to_string_lossy().as_ref(),
            "--idle-timeout",
            "1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn kindling serve");

    let mut bound = false;
    for _ in 0..400 {
        if socket.exists() {
            bound = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(bound, "daemon never bound the socket");

    let status = wait_with_timeout(&mut child, Duration::from_secs(10));
    assert!(status.is_some(), "serve did not idle-shut-down in time");
    assert!(status.unwrap().success(), "serve exited non-zero");
}

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// A running in-process daemon on a temp socket with a temp kindling home.
struct Daemon {
    socket_path: PathBuf,
    kindling_home: PathBuf,
    _home: TempDir,
    // Held to keep the daemon's runtime alive for the test's lifetime (the
    // server task runs on it); never read directly.
    _runtime: tokio::runtime::Runtime,
    handle: tokio::task::JoinHandle<Result<(), kindling_server::ServerError>>,
}

impl Daemon {
    /// Start a daemon on a dedicated multi-thread runtime owned by the test, so
    /// the (synchronous) `Command` child can talk to it concurrently.
    fn start() -> Self {
        let home = tempfile::tempdir().unwrap();
        let home_path = home.path().to_path_buf();
        let socket_path = home_path.join("k.sock");
        let config = kindling_server::ServerConfig {
            socket_path: socket_path.clone(),
            kindling_home: home_path.clone(),
            pid_path: home_path.join("k.pid"),
            idle_timeout: Duration::from_secs(3600),
        };
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let handle = runtime.spawn(async move { kindling_server::serve(config).await });
        wait_for_socket(&socket_path);
        Self {
            socket_path,
            kindling_home: home_path,
            _home: home,
            _runtime: runtime,
            handle,
        }
    }

    /// Open the project DB the daemon routed to for `REPO_ROOT`.
    fn open_store(&self) -> SqliteKindlingStore {
        let path = project_db_path(&self.kindling_home, REPO_ROOT);
        SqliteKindlingStore::open(&path).expect("open project db")
    }

    /// Run `bin` with `args` and `stdin_json` piped in, with the hook env
    /// (socket override + pinned repo root + TZ) set. Returns the full output.
    fn run_umbrella(&self, bin: &str, args: &[&str], stdin_json: &str) -> std::process::Output {
        use std::io::Write;
        let mut child = Command::new(bin)
            .args(args)
            .env("KINDLING_SOCK", &self.socket_path)
            .env("KINDLING_REPO_ROOT", REPO_ROOT)
            .env("TZ", "UTC")
            .env_remove("KINDLING_MAX_CONTEXT")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn umbrella");
        child
            .stdin
            .take()
            .expect("child stdin")
            .write_all(stdin_json.as_bytes())
            .expect("write stdin");
        child.wait_with_output().expect("await umbrella")
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        // Abort the server task; the owned runtime then drops with the struct,
        // winding down any remaining background tasks.
        self.handle.abort();
    }
}

/// The standard post-tool-use hook context used by the hook + symlink tests.
fn post_tool_use_ctx() -> String {
    json!({
        "session_id": SESSION,
        "cwd": REPO_ROOT,
        "tool_name": "Bash",
        "tool_input": { "command": "cargo build --workspace" },
        "tool_result": { "exitCode": 0 },
    })
    .to_string()
}

/// Assert exactly one `command` observation landed for `REPO_ROOT` with the
/// expected content/scope (shared by the hook subcommand + symlink tests).
fn assert_single_command_observation(daemon: &Daemon) {
    let scope = ScopeIds {
        repo_id: Some(REPO_ROOT.to_string()),
        ..Default::default()
    };
    let store = daemon.open_store();
    let obs = store
        .query_observations(Some(&scope), None, None, 50)
        .expect("query observations");
    assert_eq!(obs.len(), 1, "exactly one observation should land");
    let o = &obs[0];
    assert_eq!(o.kind, ObservationKind::Command);
    assert_eq!(o.scope_ids.session_id.as_deref(), Some(SESSION));
    assert_eq!(o.provenance["toolName"], json!("Bash"));
}

fn wait_for_socket(socket_path: &Path) {
    for _ in 0..400 {
        if socket_path.exists() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("daemon socket never appeared: {}", socket_path.display());
}

/// Symlink `target` → `link`. On platforms/filesystems where symlinking the
/// CARGO_BIN_EXE is unavailable, fall back to a hard copy (argv[0]'s basename is
/// still `kindling-hook`, which is all the drop-in check needs).
fn symlink_or_copy(target: &Path, link: &Path) {
    #[cfg(unix)]
    {
        if std::os::unix::fs::symlink(target, link).is_ok() {
            return;
        }
    }
    std::fs::copy(target, link).expect("copy umbrella binary for drop-in test");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(link, perms).expect("chmod drop-in copy");
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
