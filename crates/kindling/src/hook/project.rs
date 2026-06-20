//! Project-root resolution (routing parity with the Node hooks).
//!
//! Replicates `getProjectRoot(cwd)` from
//! `plugins/kindling-claude-code/hooks/lib/init.js`:
//!
//! 1. If `KINDLING_REPO_ROOT` is set AND `resolve(cwd)` starts with it, use it.
//! 2. Otherwise `git rev-parse --show-toplevel` run in `cwd`, trimmed.
//! 3. Otherwise the canonicalized `cwd`.
//!
//! The resolved root becomes `ClientConfig.project_root`, so the daemon routes
//! to the same per-project DB the Node hooks used and the injection
//! `scopeIds.repoId` matches.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

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
    //     contamination, exactly as the Node guard does).
    if let Some(cached) = env_root {
        let cached = cached.to_string_lossy().into_owned();
        if !cached.is_empty() && resolved.starts_with(&cached) {
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

/// `git rev-parse --show-toplevel` in `cwd`, trimmed; `None` on any failure.
fn git_toplevel(cwd: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(cwd)
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
    fn normalize_collapses_dot_segments() {
        assert_eq!(normalize(Path::new("/a/b/../c/./d")), "/a/c/d");
    }
}
