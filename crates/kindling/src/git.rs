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

/// The git working-tree root containing `dir`, or `None` if `dir` is not
/// inside a git working tree (or `git` is unavailable).
///
/// Inside a linked worktree this is the worktree's own root, not the main
/// checkout. The path is exactly what git prints (absolute, symlinks
/// resolved), trimmed of the trailing newline.
pub fn repo_root_from(dir: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let root = String::from_utf8(output.stdout).ok()?;
    let trimmed = root.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
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
        let status = Command::new("git")
            .args(["-c", "user.name=kindling", "-c", "user.email=k@example.com"])
            .args([
                "-c",
                "commit.gpgsign=false",
                "-c",
                "init.defaultBranch=main",
            ])
            .args(args)
            .current_dir(dir)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .output()
            .expect("git is installed")
            .status;
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
        let probe = Command::new("git")
            .args(["rev-parse", "--is-inside-work-tree"])
            .current_dir(&plain)
            .output()
            .unwrap();
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
}
