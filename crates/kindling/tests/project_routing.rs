//! Project routing parity between the CLI and the hooks (GH-205).
//!
//! The hooks resolve the project root as `KINDLING_REPO_ROOT` (when cwd is
//! inside it), else the Git toplevel, else cwd, and hash that into
//! `<kindling home>/projects/<hash>/kindling.db`. These tests run the real
//! binary with no `--db` under a temp `HOME` and assert the CLI — embedded and
//! `--via-daemon` — resolves the same database from a repository root and from
//! a nested subdirectory.

mod support;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use support::{assert_success, json_stdout, stdout};
use tempfile::TempDir;

/// A temp `HOME` plus a Git repository with a nested subdirectory.
struct Workspace {
    _dir: TempDir,
    home: PathBuf,
    repo: PathBuf,
    nested: PathBuf,
}

impl Workspace {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(dir.path()).unwrap();
        let home = base.join("home");
        let repo = base.join("repo");
        let nested = repo.join("src").join("deep");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&nested).unwrap();
        let out = git(&repo).args(["init", "-q"]).output().expect("git");
        assert!(out.status.success(), "git init failed");
        Self {
            _dir: dir,
            home,
            repo,
            nested,
        }
    }

    /// The kindling home the CLI derives from `HOME`.
    fn kindling_home(&self) -> PathBuf {
        self.home.join(".kindling")
    }

    /// What the hooks hash for this repository: Git's raw toplevel output.
    fn git_root(&self) -> String {
        let out = git(&self.nested)
            .args(["rev-parse", "--show-toplevel"])
            .output()
            .expect("git");
        assert!(out.status.success());
        String::from_utf8(out.stdout)
            .unwrap()
            .trim_end()
            .to_string()
    }

    fn db_for_root(&self, root: &str) -> PathBuf {
        kindling_store::project_db_path(&self.kindling_home(), root)
    }

    /// Run the CLI in `cwd` with no `--db` and a clean routing environment.
    fn run(&self, cwd: &Path, args: &[&str], envs: &[(&str, &str)]) -> Output {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_kindling"));
        cmd.args(args)
            .current_dir(cwd)
            .env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            .env_remove("KINDLING_DB_PATH")
            .env_remove("KINDLING_REPO_ROOT")
            .env_remove("KINDLING_SOCK")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE");
        for (key, value) in envs {
            cmd.env(key, value);
        }
        cmd.output().expect("run kindling")
    }

    /// `status --json` in `cwd`, returning the database path it resolved.
    fn status_db(&self, cwd: &Path, envs: &[(&str, &str)]) -> PathBuf {
        let out = self.run(cwd, &["status", "--json"], envs);
        assert_success(&out);
        let v = json_stdout(&out);
        PathBuf::from(v["database"]["path"].as_str().expect("database.path"))
    }
}

fn git(cwd: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.current_dir(cwd)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE");
    cmd
}

#[test]
fn embedded_root_and_subdir_resolve_the_hook_project_db() {
    let ws = Workspace::new();
    let expected = ws.db_for_root(&ws.git_root());

    assert_eq!(ws.status_db(&ws.repo, &[]), expected);
    assert_eq!(ws.status_db(&ws.nested, &[]), expected);
}

#[test]
fn embedded_root_and_subdir_see_the_same_data() {
    let ws = Workspace::new();

    assert_success(&ws.run(&ws.repo, &["log", "written at the root"], &[]));
    assert_success(&ws.run(&ws.nested, &["log", "written in a subdir"], &[]));

    for cwd in [&ws.repo, &ws.nested] {
        let out = ws.run(cwd, &["search", "written", "--json"], &[]);
        assert_success(&out);
        let text = stdout(&out);
        assert!(
            text.contains("written at the root") && text.contains("written in a subdir"),
            "search from {cwd:?} should see both writes, got: {text}"
        );
    }

    // No database was created for the subdirectory's own hash.
    let nested_db = ws.db_for_root(&ws.nested.to_string_lossy());
    assert!(!nested_db.exists(), "subdir DB was created: {nested_db:?}");
}

#[test]
fn kindling_db_path_still_wins_from_a_subdir() {
    let ws = Workspace::new();
    let explicit = ws.home.join("explicit.db");
    let explicit_str = explicit.to_string_lossy().into_owned();

    let resolved = ws.status_db(&ws.nested, &[("KINDLING_DB_PATH", &explicit_str)]);
    assert_eq!(resolved, explicit);
}

#[test]
fn kindling_repo_root_still_wins_when_cwd_is_inside_it() {
    let ws = Workspace::new();
    // A non-Git tree, so the override (not Git) decides the root.
    let outer = ws.home.join("outer");
    let inner = outer.join("inner");
    std::fs::create_dir_all(&inner).unwrap();
    let outer_str = outer.to_string_lossy().into_owned();

    let resolved = ws.status_db(&inner, &[("KINDLING_REPO_ROOT", &outer_str)]);
    assert_eq!(resolved, ws.db_for_root(&outer_str));
}

#[test]
fn non_git_directory_still_routes_by_cwd() {
    let ws = Workspace::new();
    let plain = ws.home.join("plain");
    std::fs::create_dir_all(&plain).unwrap();

    let resolved = ws.status_db(&plain, &[]);
    assert_eq!(resolved, ws.db_for_root(&plain.to_string_lossy()));
}

/// `--via-daemon` from a subdirectory routes to the repository's project DB.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn via_daemon_subdir_routes_to_the_hook_project_db() {
    use std::time::Duration;

    let ws = Workspace::new();
    let kindling_home = ws.kindling_home();
    std::fs::create_dir_all(&kindling_home).unwrap();
    let socket = kindling_home.join("kindling.sock");
    let config = kindling_server::ServerConfig {
        socket_path: socket.clone(),
        kindling_home: kindling_home.clone(),
        pid_path: kindling_home.join("kindling.pid"),
        port_path: kindling_home.join("kindling.port"),
        idle_timeout: Duration::from_secs(3600),
        transport: kindling_server::Transport::default(),
    };
    let handle = tokio::spawn(async move { kindling_server::serve(config).await });
    for _ in 0..400 {
        if socket.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(socket.exists(), "daemon socket never appeared");

    let expected = ws.db_for_root(&ws.git_root());
    let nested_db = ws.db_for_root(&ws.nested.to_string_lossy());
    let ws = std::sync::Arc::new(ws);
    let (from_subdir, from_root) = tokio::task::spawn_blocking({
        let ws = ws.clone();
        move || {
            let sub = ws.run(
                &ws.nested,
                &["--via-daemon", "log", "daemon subdir write", "--json"],
                &[],
            );
            let root = ws.run(
                &ws.repo,
                &["--via-daemon", "log", "daemon root write", "--json"],
                &[],
            );
            (sub, root)
        }
    })
    .await
    .unwrap();
    handle.abort();
    let _ = handle.await;

    assert_success(&from_subdir);
    assert_success(&from_root);

    // The embedded CLI, from either directory, sees both daemon writes: the
    // two modes resolve the same project database.
    for cwd in [&ws.repo, &ws.nested] {
        let out = ws.run(cwd, &["search", "daemon", "--json"], &[]);
        assert_success(&out);
        let text = stdout(&out);
        assert!(
            text.contains("daemon subdir write") && text.contains("daemon root write"),
            "embedded search from {cwd:?} should see both daemon writes, got: {text}"
        );
    }

    assert!(
        expected.exists(),
        "daemon did not write the repo project DB: {expected:?}"
    );
    assert!(
        !nested_db.exists(),
        "daemon wrote a subdir DB: {nested_db:?}"
    );
}
