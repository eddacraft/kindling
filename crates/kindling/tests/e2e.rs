//! End-to-end tests driving the real `kindling` binary's hook surface against an
//! in-process daemon over a temp Unix socket.
//!
//! Each test:
//!   - starts a `kindling-server` daemon on a temp socket + temp kindling home;
//!   - runs `CARGO_BIN_EXE_kindling hook <type>` with stdin = the hook JSON and
//!     env `KINDLING_SOCK` (socket override) + `KINDLING_REPO_ROOT` (forces the
//!     project root, so DB routing is deterministic) + a pinned `TZ`;
//!   - asserts daemon DB state and/or the binary's stdout, and that every hook
//!     exits 0 (even on malformed stdin).

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use kindling_store::{project_db_path, SqliteKindlingStore};
use kindling_types::{CapsuleStatus, ObservationKind, ScopeIds};
use serde_json::{json, Value};
use tempfile::TempDir;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

/// The fixed project root used by every test. `KINDLING_REPO_ROOT` is set to
/// this and the hook `cwd` is set to this, so `project_root(cwd)` returns it and
/// the daemon routes to a single deterministic per-project DB.
const REPO_ROOT: &str = "/tmp/kindling-hook-e2e/repo";
const SESSION: &str = "sess-e2e";

/// A running in-process daemon on a temp socket with a temp kindling home.
struct Daemon {
    socket_path: PathBuf,
    kindling_home: PathBuf,
    _home: TempDir,
    handle: tokio::task::JoinHandle<Result<(), kindling_server::ServerError>>,
}

impl Daemon {
    async fn start() -> Self {
        let home = tempfile::tempdir().unwrap();
        let home_path = home.path().to_path_buf();
        let socket_path = home_path.join("k.sock");
        let config = kindling_server::ServerConfig {
            socket_path: socket_path.clone(),
            kindling_home: home_path.clone(),
            pid_path: home_path.join("k.pid"),
            idle_timeout: Duration::from_secs(3600),
        };
        let handle = tokio::spawn(async move { kindling_server::serve(config).await });
        wait_for_socket(&socket_path).await;
        Self {
            socket_path,
            kindling_home: home_path,
            _home: home,
            handle,
        }
    }

    /// Open the project DB the daemon routed to for `REPO_ROOT`.
    fn open_store(&self) -> SqliteKindlingStore {
        let path = project_db_path(&self.kindling_home, REPO_ROOT);
        SqliteKindlingStore::open(&path).expect("open project db")
    }

    /// Run the hook binary for `hook_type` with `stdin_json` piped in. Returns
    /// (exit_ok, stdout). Always sets the socket/repo-root/TZ env.
    async fn run_hook(&self, hook_type: &str, stdin_json: &str) -> (bool, String) {
        let exe = env!("CARGO_BIN_EXE_kindling");
        let mut child = Command::new(exe)
            .arg("hook")
            .arg(hook_type)
            .env("KINDLING_SOCK", &self.socket_path)
            .env("KINDLING_REPO_ROOT", REPO_ROOT)
            .env("TZ", "UTC")
            .env_remove("KINDLING_MAX_CONTEXT")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn kindling-hook");

        {
            let mut stdin = child.stdin.take().expect("child stdin");
            stdin
                .write_all(stdin_json.as_bytes())
                .await
                .expect("write stdin");
            // Drop closes stdin so the binary's read_to_end completes.
        }

        let output = child.wait_with_output().await.expect("await hook");
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        (output.status.success(), stdout)
    }

    async fn join(self) {
        // Detach: the daemon stays up for the process; just drop the handle's
        // ownership by aborting at test end.
        self.handle.abort();
    }
}

async fn wait_for_socket(socket_path: &Path) {
    for _ in 0..400 {
        if socket_path.exists() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("daemon socket never appeared: {}", socket_path.display());
}

fn repo_scope() -> ScopeIds {
    ScopeIds {
        repo_id: Some(REPO_ROOT.to_string()),
        ..Default::default()
    }
}

/// (a) Capture hooks land observation rows with the expected shape.
#[tokio::test]
async fn post_tool_use_lands_observation() {
    let daemon = Daemon::start().await;

    let ctx = json!({
        "session_id": SESSION,
        "cwd": REPO_ROOT,
        "tool_name": "Bash",
        "tool_input": { "command": "cargo build --workspace" },
        "tool_result": { "exitCode": 0 },
    });
    let (ok, stdout) = daemon.run_hook("post-tool-use", &ctx.to_string()).await;
    assert!(ok, "hook should exit 0");
    assert!(
        stdout.is_empty(),
        "capture hook prints nothing, got {stdout:?}"
    );

    let store = daemon.open_store();
    let obs = store
        .query_observations(Some(&repo_scope()), None, None, 50)
        .expect("query observations");
    assert_eq!(obs.len(), 1, "exactly one observation should land");
    let o = &obs[0];
    assert_eq!(o.kind, ObservationKind::Command);
    assert_eq!(
        o.content,
        "Tool: Bash\n\n$ cargo build --workspace\n\n{\n  \"exitCode\": 0\n}"
    );
    // repoId is the RAW cwd (Node quirk) — here cwd == REPO_ROOT.
    assert_eq!(o.scope_ids.repo_id.as_deref(), Some(REPO_ROOT));
    assert_eq!(o.scope_ids.session_id.as_deref(), Some(SESSION));
    assert_eq!(o.provenance["toolName"], json!("Bash"));
    assert_eq!(o.provenance["command"], json!("cargo"));
    assert_eq!(o.provenance["exitCode"], json!(0));

    daemon.join().await;
}

/// User-prompt and subagent-stop capture hooks also land rows.
#[tokio::test]
async fn user_prompt_and_subagent_land_observations() {
    let daemon = Daemon::start().await;

    let (ok, _) = daemon
        .run_hook(
            "user-prompt-submit",
            &json!({ "session_id": SESSION, "cwd": REPO_ROOT, "content": "refactor the parser" })
                .to_string(),
        )
        .await;
    assert!(ok);

    let (ok, _) = daemon
        .run_hook(
            "subagent-stop",
            &json!({
                "session_id": SESSION, "cwd": REPO_ROOT,
                "agent_type": "code-reviewer", "output": "LGTM"
            })
            .to_string(),
        )
        .await;
    assert!(ok);

    let store = daemon.open_store();
    let obs = store
        .query_observations(Some(&repo_scope()), None, None, 50)
        .expect("query observations");
    assert_eq!(obs.len(), 2);
    let kinds: Vec<ObservationKind> = obs.iter().map(|o| o.kind).collect();
    assert!(kinds.contains(&ObservationKind::Message));
    assert!(kinds.contains(&ObservationKind::NodeEnd));

    daemon.join().await;
}

/// (b) session-start emits the injection envelope with the daemon-formatted
/// markdown (after seeding an observation so there is something to inject).
#[tokio::test]
async fn session_start_emits_injection_envelope() {
    let daemon = Daemon::start().await;

    // Seed a prior observation so SessionStart has recent activity to inject.
    let (ok, _) = daemon
        .run_hook(
            "post-tool-use",
            &json!({
                "session_id": "prior", "cwd": REPO_ROOT,
                "tool_name": "Read", "tool_input": { "file_path": "/seed.rs" }
            })
            .to_string(),
        )
        .await;
    assert!(ok);

    let (ok, stdout) = daemon
        .run_hook(
            "session-start",
            &json!({ "session_id": SESSION, "cwd": REPO_ROOT }).to_string(),
        )
        .await;
    assert!(ok, "session-start should exit 0");
    assert!(!stdout.is_empty(), "should emit an envelope");

    let v: Value = serde_json::from_str(&stdout).expect("stdout is JSON");
    assert_eq!(v["continue"], json!(true));
    assert_eq!(v["hookSpecificOutput"]["hookEventName"], "SessionStart");
    let ctx = v["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("additionalContext string");
    assert!(
        ctx.contains("Prior Context (from Kindling)"),
        "daemon markdown header present: {ctx:?}"
    );
    assert!(
        ctx.contains("Recent Activity"),
        "recent activity section: {ctx:?}"
    );

    // And the session capsule was opened.
    let store = daemon.open_store();
    let cap = store
        .get_open_capsule_for_session(SESSION)
        .expect("get open capsule")
        .expect("capsule should be open");
    assert_eq!(cap.status, CapsuleStatus::Open);
    assert_eq!(cap.intent, "Claude Code session");

    daemon.join().await;
}

/// session-start with an empty project prints nothing (no context to inject)
/// but still opens the capsule and exits 0.
#[tokio::test]
async fn session_start_empty_prints_nothing_but_opens_capsule() {
    let daemon = Daemon::start().await;

    let (ok, stdout) = daemon
        .run_hook(
            "session-start",
            &json!({ "session_id": SESSION, "cwd": REPO_ROOT }).to_string(),
        )
        .await;
    assert!(ok);
    assert!(
        stdout.is_empty(),
        "nothing to inject → empty stdout, got {stdout:?}"
    );

    let store = daemon.open_store();
    assert!(store
        .get_open_capsule_for_session(SESSION)
        .expect("get open capsule")
        .is_some());

    daemon.join().await;
}

/// (c) stop closes the session's open capsule.
#[tokio::test]
async fn stop_closes_open_capsule() {
    let daemon = Daemon::start().await;

    // Open via session-start.
    let (ok, _) = daemon
        .run_hook(
            "session-start",
            &json!({ "session_id": SESSION, "cwd": REPO_ROOT }).to_string(),
        )
        .await;
    assert!(ok);

    // Stop with a summary.
    let (ok, stdout) = daemon
        .run_hook(
            "stop",
            &json!({ "session_id": SESSION, "cwd": REPO_ROOT, "summary": "did the thing" })
                .to_string(),
        )
        .await;
    assert!(ok);
    assert!(stdout.is_empty());

    let store = daemon.open_store();
    // No longer open.
    assert!(store
        .get_open_capsule_for_session(SESSION)
        .expect("get open capsule")
        .is_none());

    daemon.join().await;
}

/// stop with no open capsule is a no-op success (Node's "session not found").
#[tokio::test]
async fn stop_without_open_capsule_is_ok() {
    let daemon = Daemon::start().await;
    let (ok, stdout) = daemon
        .run_hook(
            "stop",
            &json!({ "session_id": "ghost", "cwd": REPO_ROOT }).to_string(),
        )
        .await;
    assert!(ok, "stop with no capsule still exits 0");
    assert!(stdout.is_empty());
    daemon.join().await;
}

/// pre-compact forwards the daemon envelope (or nothing). Empty project → no
/// envelope, exit 0.
#[tokio::test]
async fn pre_compact_empty_prints_nothing() {
    let daemon = Daemon::start().await;
    let (ok, stdout) = daemon
        .run_hook(
            "pre-compact",
            &json!({ "session_id": SESSION, "cwd": REPO_ROOT }).to_string(),
        )
        .await;
    assert!(ok);
    assert!(
        stdout.is_empty(),
        "nothing to inject → empty, got {stdout:?}"
    );
    daemon.join().await;
}

/// (d) every hook exits 0 even on malformed stdin, and prints nothing.
#[tokio::test]
async fn malformed_stdin_still_exits_zero() {
    let daemon = Daemon::start().await;
    for hook_type in [
        "session-start",
        "post-tool-use",
        "post-tool-use-failure",
        "user-prompt-submit",
        "subagent-stop",
        "stop",
        "pre-compact",
    ] {
        let (ok, stdout) = daemon.run_hook(hook_type, "{ this is not json").await;
        assert!(ok, "{hook_type} must exit 0 on malformed stdin");
        assert!(
            stdout.is_empty(),
            "{hook_type} prints nothing on malformed stdin, got {stdout:?}"
        );
    }
    daemon.join().await;
}

/// An unknown hook type also exits 0 (logged to stderr).
#[tokio::test]
async fn unknown_hook_type_exits_zero() {
    let daemon = Daemon::start().await;
    let (ok, stdout) = daemon
        .run_hook("not-a-hook", &json!({ "cwd": REPO_ROOT }).to_string())
        .await;
    assert!(ok);
    assert!(stdout.is_empty());
    daemon.join().await;
}

/// Latency: a warm hook dispatch (daemon already up) completes well within a
/// generous bound. Logs the measured warm + cold-ish values.
#[tokio::test]
async fn warm_hook_latency_is_bounded() {
    let daemon = Daemon::start().await;

    let ctx = json!({
        "session_id": SESSION, "cwd": REPO_ROOT,
        "tool_name": "Read", "tool_input": { "file_path": "/x.rs" }
    })
    .to_string();

    // First call also pays process spawn + first-connection cost ("cold-ish":
    // the daemon is up, but this is the first hook process).
    let cold_start = Instant::now();
    let (ok, _) = daemon.run_hook("post-tool-use", &ctx).await;
    assert!(ok);
    let cold = cold_start.elapsed();

    // Warm: subsequent calls with the daemon and DB already warm.
    let mut warmest = Duration::from_secs(60);
    for _ in 0..5 {
        let t = Instant::now();
        let (ok, _) = daemon.run_hook("post-tool-use", &ctx).await;
        assert!(ok);
        warmest = warmest.min(t.elapsed());
    }

    eprintln!(
        "[latency] cold-ish first hook process: {cold:?}; warmest of 5 subsequent: {warmest:?}"
    );
    // Generous bound: process spawn + UDS round trip over a debug build. The APS
    // warm target is <10ms for the in-daemon dispatch itself; this measures the
    // whole child process, so we assert a much looser ceiling.
    assert!(
        warmest < Duration::from_secs(2),
        "warm hook process took {warmest:?}, expected < 2s"
    );

    daemon.join().await;
}
