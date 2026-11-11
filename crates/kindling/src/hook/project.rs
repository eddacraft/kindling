//! Project-root resolution (routing parity with the Node hooks).
//!
//! Replicates `getProjectRoot(cwd)` from
//! `plugins/kindling-claude-code/hooks/lib/init.js`:
//!
//! 1. If `KINDLING_REPO_ROOT` is set AND the resolved cwd is that root or a
//!    path beneath it (component-wise, after normalising the override), use it.
//! 2. Otherwise `git rev-parse --show-toplevel` run in `cwd` (Git repo
//!    overrides isolated; only the trailing newline stripped).
//! 3. Otherwise the canonicalized `cwd`.
//!
//! The resolved root becomes `ClientConfig.project_root`, so the daemon routes
//! to the same per-project DB the Node hooks used and the injection
//! `scopeIds.repoId` matches.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Resolve the project root for `cwd`, matching the Node `getProjectRoot`.
pub fn project_root(cwd: &str) -> String {
    project_root_with(cwd, std::env::var_os("KINDLING_REPO_ROOT"))
}

/// Inner resolution with the `KINDLING_REPO_ROOT` value injected, so tests can
/// exercise the env-guard branch without mutating the (process-global, and
/// therefore racy under parallel tests) environment.
fn project_root_with(cwd: &str, env_root: Option<OsString>) -> String {
    let resolved = resolve(cwd);

    // (1) KINDLING_REPO_ROOT, only if cwd is under it (prevents cross-project
    //     contamination). Compare path components after normalising the override
    //     so a sibling such as `/work/repo-copy` is not treated as inside
    //     `/work/repo`. Return the raw env value, matching the plugin helper.
    if let Some(cached) = env_root {
        let cached = cached.to_string_lossy().into_owned();
        if !cached.is_empty() && cwd_is_under_root(&resolved, &cached) {
            return cached;
        }
    }

    // (2) git rev-parse --show-toplevel, run in cwd.
    if let Some(toplevel) = git_toplevel(cwd) {
        return toplevel;
    }

    // (3) Fallback: resolved cwd.
    resolved
}

/// True when `resolved_cwd` is `env_root` or a directory under it.
///
/// `env_root` is resolved the same way as cwd (absolute, lexically normalised)
/// so a relative override such as `./repo` still matches. [`Path::starts_with`]
/// compares whole components, which a raw string prefix does not.
fn cwd_is_under_root(resolved_cwd: &str, env_root: &str) -> bool {
    let root = resolve(env_root);
    Path::new(resolved_cwd).starts_with(&root)
}

/// `git rev-parse --show-toplevel` in `cwd`; `None` on any failure.
///
/// Must stay git's raw output (no separator normalisation): the project root is
/// hashed into the per-project DB path, so any change would orphan databases.
fn git_toplevel(cwd: &str) -> Option<String> {
    crate::git::repo_root_from(Path::new(cwd))
}

/// Best-effort equivalent of Node's `path.resolve(cwd)`: an absolute,
/// lexically-normalized path string. Node's `resolve` does not touch the
/// filesystem, so we avoid `canonicalize` (which would fail on missing paths
/// and resolve symlinks — neither of which Node does here).
fn resolve(cwd: &str) -> String {
    let path = Path::new(cwd);
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };
    normalize(&absolute)
}

/// Lexically normalize a path (resolve `.` and `..` segments) without touching
/// the filesystem, mirroring the cleanup `path.resolve` performs.
fn normalize(path: &Path) -> String {
    use std::path::Component;
    let mut out: Vec<Component> = Vec::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                if matches!(out.last(), Some(Component::Normal(_))) {
                    out.pop();
                } else if !matches!(
                    out.last(),
                    Some(Component::RootDir) | Some(Component::Prefix(_))
                ) {
                    out.push(component);
                }
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    let mut buf = PathBuf::new();
    for c in out {
        buf.push(c.as_os_str());
    }
    buf.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_root_used_when_cwd_is_under_it() {
        // Inject the env value directly — no process-env mutation, so this is
        // race-free under cargo's parallel test runner.
        assert_eq!(
            project_root_with("/home/u/proj/sub/dir", Some("/home/u/proj".into())),
            "/home/u/proj"
        );
    }

    #[test]
    fn env_root_ignored_when_cwd_outside_it() {
        // cwd is NOT under the cached root → the env guard does not fire. With a
        // non-git temp path the git step also fails, landing on the resolved cwd.
        let root = project_root_with("/tmp/elsewhere/x", Some("/home/u/proj".into()));
        assert_ne!(root, "/home/u/proj");
    }

    #[test]
    fn env_root_ignored_when_empty() {
        // An empty env value is treated as unset (matches the `!is_empty` guard).
        let root = project_root_with("/tmp/elsewhere/x", Some(OsString::new()));
        assert_ne!(root, "");
    }

    #[test]
    fn env_root_ignored_for_sibling_prefix() {
        // `/work/repo-copy` shares a string prefix with `/work/repo` but is not
        // inside it. A raw `starts_with` would wrongly honour the override.
        let root = project_root_with("/work/repo-copy", Some("/work/repo".into()));
        assert_ne!(root, "/work/repo");
    }

    #[test]
    fn env_root_relative_honoured_when_cwd_is_under_it() {
        // Relative overrides are normalised against the process cwd before the
        // containment check, matching the plugin helper's `path.resolve`.
        assert_eq!(project_root_with("repo/src", Some("repo".into())), "repo");
        assert_eq!(
            project_root_with("repo/src", Some("./repo".into())),
            "./repo"
        );
    }

    #[test]
    fn env_root_relative_ignored_for_sibling_prefix() {
        let root = project_root_with("repo-copy", Some("repo".into()));
        assert_ne!(root, "repo");
    }

    #[test]
    fn normalize_collapses_dot_segments() {
        assert_eq!(normalize(Path::new("/a/b/../c/./d")), "/a/c/d");
    }
}
