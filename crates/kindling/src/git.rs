//! Git workspace detection.
//!
//! The CLI uses this to default the `repoId` scope to the enclosing git
//! working tree, so `kindling search` (and `log` / `capsule open`) are scoped
//! to the current project without a `--repo` flag. Detection shells out to
//! `git rev-parse --show-toplevel`, the same probe the Claude Code hook uses to
//! derive its project root, so CLI scopes line up with hook-captured data
//! (on Windows the CLI default is converted to native separators; see
//! [`workspace_repo_id_from_cwd`]).
//!
//! Detection always fails open: no `git` binary, a non-git directory or any
//! other error yields `None`, and callers then apply no repo filter.

use std::path::Path;
use std::process::Command;

/// Git environment variables that redirect discovery at a different
/// repository than the one containing `current_dir`. Cleared so a parent
/// process cannot make `rev-parse --show-toplevel` report another checkout.
const GIT_REPO_OVERRIDE_VARS: &[&str] = &["GIT_DIR", "GIT_WORK_TREE"];

/// The git working-tree root containing `dir`, or `None` if `dir` is not
/// inside a git working tree (or `git` is unavailable).
///
/// Inside a linked worktree this is the worktree's own root, not the main
/// checkout. The path is exactly what git prints (absolute, symlinks
/// resolved), with only the trailing newline stripped so leading or trailing
/// whitespace in the path is preserved.
pub fn repo_root_from(dir: &Path) -> Option<String> {
    show_toplevel(dir, &[])
}

/// Run `git rev-parse --show-toplevel` in `dir`.
///
/// `overrides` are applied to the child first, then [`isolate_git_discovery`]
/// clears `GIT_DIR` / `GIT_WORK_TREE`. Tests use that order to inject those
/// variables without mutating the process environment (`unsafe_code` is
/// forbidden in this crate).
fn show_toplevel(dir: &Path, overrides: &[(&str, &Path)]) -> Option<String> {
    let mut cmd = Command::new("git");
    cmd.args(["rev-parse", "--show-toplevel"]).current_dir(dir);
    for (key, value) in overrides {
        cmd.env(key, value);
    }
    isolate_git_discovery(&mut cmd);
    parse_show_toplevel(cmd.output().ok()?)
}

fn isolate_git_discovery(cmd: &mut Command) {
    for var in GIT_REPO_OVERRIDE_VARS {
        cmd.env_remove(var);
    }
}

fn parse_show_toplevel(output: std::process::Output) -> Option<String> {
    if !output.status.success() {
        return None;
    }
    let root = String::from_utf8(output.stdout).ok()?;
    let stripped = strip_trailing_git_newline(&root);
    if stripped.is_empty() {
        None
    } else {
        Some(stripped.to_string())
    }
}

/// Strip the single trailing newline Git appends to `--show-toplevel`
/// (`\n`, `\r\n`, or `\r`). Unlike [`str::trim`], this leaves spaces, tabs,
/// and any other path characters intact.
fn strip_trailing_git_newline(s: &str) -> &str {
    s.strip_suffix("\r\n")
        .or_else(|| s.strip_suffix('\n'))
        .or_else(|| s.strip_suffix('\r'))
        .unwrap_or(s)
}

/// The default CLI `repoId` for the process's current working directory: the
/// enclosing git root in the host's native path format.
///
/// git prints forward slashes on Windows (`C:/Users/me/proj`), but the Claude
/// Code hook records its raw cwd (`C:\Users\me\proj`) as an observation's
/// `repoId`, so the root is converted to native separators to match. This is
/// deliberately separate from [`repo_root_from`]: the hook's project-root
/// routing hashes git's raw output, and changing that would orphan existing
/// per-project databases.
pub fn workspace_repo_id_from_cwd() -> Option<String> {
    let cwd = std::env::current_dir().ok()?;
    repo_root_from(&cwd).map(|root| native_separators(&root, cfg!(windows)))
}

/// Convert `/` to `\` when `windows` is true; otherwise return `path`
/// unchanged. Pure (the platform is a parameter) so it is testable everywhere.
fn native_separators(path: &str, windows: bool) -> String {
    if windows {
        path.replace('/', "\\")
    } else {
        path.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Run `git` in `dir` with an isolated identity, panicking on failure.
    fn git(dir: &Path, args: &[&str]) {
        let mut cmd = Command::new("git");
        cmd.args(["-c", "user.name=kindling", "-c", "user.email=k@example.com"])
            .args([
                "-c",
                "commit.gpgsign=false",
                "-c",
                "init.defaultBranch=main",
            ])
            .args(args)
            .current_dir(dir);
        isolate_git_discovery(&mut cmd);
        let status = cmd.output().expect("git is installed").status;
        assert!(status.success(), "git {args:?} failed in {}", dir.display());
    }

    fn canonical(path: &Path) -> String {
        std::fs::canonicalize(path)
            .unwrap()
            .to_string_lossy()
            .into_owned()
    }

    fn init_repo() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        git(&repo, &["init", "-q"]);
        (dir, repo)
    }

    #[test]
    fn native_separators_converts_on_windows() {
        assert_eq!(
            native_separators("C:/Users/me/proj", true),
            "C:\\Users\\me\\proj"
        );
        // Already-native and UNC-style paths are left in backslash form.
        assert_eq!(native_separators("C:\\Users\\me", true), "C:\\Users\\me");
        assert_eq!(
            native_separators("//server/share/proj", true),
            "\\\\server\\share\\proj"
        );
    }

    #[test]
    fn native_separators_is_identity_elsewhere() {
        assert_eq!(native_separators("/home/me/proj", false), "/home/me/proj");
        assert_eq!(
            native_separators("C:/odd/but/kept", false),
            "C:/odd/but/kept"
        );
    }

    #[test]
    fn detects_root_at_repo_root() {
        let (_dir, repo) = init_repo();
        assert_eq!(repo_root_from(&repo), Some(canonical(&repo)));
    }

    #[test]
    fn detects_root_from_nested_directory() {
        let (_dir, repo) = init_repo();
        let nested = repo.join("a").join("b");
        std::fs::create_dir_all(&nested).unwrap();
        assert_eq!(repo_root_from(&nested), Some(canonical(&repo)));
    }

    #[test]
    fn non_git_directory_yields_none() {
        let dir = tempfile::tempdir().unwrap();
        // Guard against the temp dir itself sitting inside a git checkout on
        // an unusual host: only assert when git agrees it is outside one.
        let plain = dir.path().join("plain");
        std::fs::create_dir(&plain).unwrap();
        let mut probe = Command::new("git");
        probe
            .args(["rev-parse", "--is-inside-work-tree"])
            .current_dir(&plain);
        isolate_git_discovery(&mut probe);
        let probe = probe.output().unwrap();
        if probe.status.success() {
            return;
        }
        assert_eq!(repo_root_from(&plain), None);
    }

    #[test]
    fn missing_directory_yields_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(repo_root_from(&dir.path().join("does-not-exist")), None);
    }

    #[test]
    fn linked_worktree_reports_its_own_root() {
        let (dir, repo) = init_repo();
        git(&repo, &["commit", "-q", "--allow-empty", "-m", "init"]);
        let wt = dir.path().join("wt");
        git(
            &repo,
            &["worktree", "add", "-q", "-b", "side", wt.to_str().unwrap()],
        );
        let nested = wt.join("sub");
        std::fs::create_dir(&nested).unwrap();

        assert_eq!(repo_root_from(&wt), Some(canonical(&wt)));
        assert_eq!(repo_root_from(&nested), Some(canonical(&wt)));
        assert_ne!(repo_root_from(&wt), repo_root_from(&repo));
    }

    #[test]
    fn strip_trailing_git_newline_preserves_path_whitespace() {
        assert_eq!(strip_trailing_git_newline("/tmp/repo \n"), "/tmp/repo ");
        assert_eq!(strip_trailing_git_newline(" /tmp/repo\r\n"), " /tmp/repo");
        assert_eq!(strip_trailing_git_newline("/tmp/repo\r"), "/tmp/repo");
        assert_eq!(strip_trailing_git_newline("/tmp/repo"), "/tmp/repo");
        assert_eq!(strip_trailing_git_newline(""), "");
        // `trim()` would collapse these; we must not.
        assert_ne!(
            strip_trailing_git_newline("/tmp/repo \n"),
            "/tmp/repo \n".trim()
        );
    }

    #[test]
    fn git_work_tree_override_does_not_select_another_checkout() {
        let (_dir_a, repo_a) = init_repo();
        let (_dir_b, repo_b) = init_repo();

        // Characterization: without isolation, GIT_WORK_TREE redirects
        // `--show-toplevel` at the other work tree.
        let polluted = Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .current_dir(&repo_a)
            .env("GIT_WORK_TREE", &repo_b)
            .output()
            .unwrap();
        assert_eq!(
            strip_trailing_git_newline(&String::from_utf8(polluted.stdout).unwrap()),
            canonical(&repo_b)
        );

        assert_eq!(
            show_toplevel(&repo_a, &[("GIT_WORK_TREE", repo_b.as_path())]),
            Some(canonical(&repo_a))
        );
    }

    #[test]
    fn git_dir_override_does_not_select_another_checkout() {
        let (_dir_a, repo_a) = init_repo();
        let (_dir_b, repo_b) = init_repo();
        let nested = repo_a.join("sub");
        std::fs::create_dir(&nested).unwrap();
        let git_dir_b = repo_b.join(".git");

        // GIT_DIR alone makes a nested cwd report itself as the toplevel.
        let polluted = Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .current_dir(&nested)
            .env("GIT_DIR", &git_dir_b)
            .output()
            .unwrap();
        assert_eq!(
            strip_trailing_git_newline(&String::from_utf8(polluted.stdout).unwrap()),
            canonical(&nested)
        );

        assert_eq!(
            show_toplevel(&nested, &[("GIT_DIR", git_dir_b.as_path())]),
            Some(canonical(&repo_a))
        );
    }

    #[test]
    fn git_dir_and_work_tree_overrides_do_not_select_another_checkout() {
        let (_dir_a, repo_a) = init_repo();
        let (_dir_b, repo_b) = init_repo();
        let git_dir_b = repo_b.join(".git");

        assert_eq!(
            show_toplevel(
                &repo_a,
                &[
                    ("GIT_DIR", git_dir_b.as_path()),
                    ("GIT_WORK_TREE", repo_b.as_path()),
                ],
            ),
            Some(canonical(&repo_a))
        );
    }

    #[test]
    fn git_dir_override_does_not_make_a_plain_directory_look_like_a_repo() {
        let dir = tempfile::tempdir().unwrap();
        let plain = dir.path().join("plain");
        std::fs::create_dir(&plain).unwrap();
        let (_dir_b, repo_b) = init_repo();
        let mut probe = Command::new("git");
        probe
            .args(["rev-parse", "--is-inside-work-tree"])
            .current_dir(&plain);
        isolate_git_discovery(&mut probe);
        if probe.output().unwrap().status.success() {
            return;
        }

        let git_dir_b = repo_b.join(".git");
        let polluted = Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .current_dir(&plain)
            .env("GIT_DIR", &git_dir_b)
            .output()
            .unwrap();
        assert!(polluted.status.success());
        assert_eq!(
            show_toplevel(&plain, &[("GIT_DIR", git_dir_b.as_path())]),
            None
        );
    }

    #[test]
    fn linked_worktree_ignores_git_overrides() {
        let (dir, repo) = init_repo();
        git(&repo, &["commit", "-q", "--allow-empty", "-m", "init"]);
        let wt = dir.path().join("wt");
        git(
            &repo,
            &["worktree", "add", "-q", "-b", "side", wt.to_str().unwrap()],
        );
        let (_dir_b, repo_b) = init_repo();
        let nested = wt.join("sub");
        std::fs::create_dir(&nested).unwrap();

        let git_dir_b = repo_b.join(".git");
        assert_eq!(
            show_toplevel(
                &nested,
                &[
                    ("GIT_DIR", git_dir_b.as_path()),
                    ("GIT_WORK_TREE", repo_b.as_path()),
                ],
            ),
            Some(canonical(&wt))
        );
    }

    /// Unix filesystems can store leading/trailing spaces in a directory name.
    /// `str::trim` would drop them from Git's `--show-toplevel` output.
    #[cfg(unix)]
    #[test]
    fn preserves_whitespace_in_repo_path() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo ");
        std::fs::create_dir(&repo).unwrap();
        let canonical_root = canonical(&repo);
        if canonical_root.trim() == canonical_root {
            // Filesystem or canonicalize folded the trailing space; nothing to prove.
            return;
        }
        git(&repo, &["init", "-q"]);
        let root = repo_root_from(&repo).expect("detected");
        assert_eq!(root, canonical_root);
        assert_ne!(root.as_str(), root.trim());
    }
}
