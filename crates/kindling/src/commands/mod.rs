//! Per-command handlers. Each module exposes `run`-style functions invoked by
//! the dispatch in `lib.rs`. Handlers print their own output (text or `--json`)
//! and return [`CliResult`](crate::CliResult).

pub mod browse;
pub mod capsule;
pub mod demo;
pub mod export;
pub mod forget;
pub mod init;
pub mod list;
pub mod log;
pub mod pin;
pub mod search;
pub mod serve;
pub mod spool;
pub mod stats;
pub mod status;

use kindling_types::ScopeIds;

/// Build a [`ScopeIds`] from optional `--session` / `--repo` flags, matching the
/// TS commands which set `{ sessionId, repoId }` (leaving the rest undefined).
pub(crate) fn scope_from(session: Option<&str>, repo: Option<&str>) -> ScopeIds {
    ScopeIds {
        session_id: session.map(str::to_string),
        repo_id: repo.map(str::to_string),
        ..Default::default()
    }
}

/// Resolve the effective `repoId` scope for a CLI verb.
///
/// Precedence (deterministic, highest first):
/// 1. an explicit `--repo <id>` is used verbatim;
/// 2. `--all-repos` disables repo scoping entirely;
/// 3. otherwise the enclosing git working-tree root of the current directory
///    (see [`crate::git::workspace_repo_id_from_cwd`]), or no repo scope when the
///    current directory is not inside a git working tree.
///
/// Verbs without an `--all-repos` flag pass `false`. Kept separate from any
/// one command so other verbs (and future search filters) share one rule.
pub(crate) fn resolve_repo(explicit: Option<&str>, all_repos: bool) -> Option<String> {
    resolve_repo_with(explicit, all_repos, crate::git::workspace_repo_id_from_cwd)
}

/// [`resolve_repo`] with the detector injected, so the precedence rules are
/// unit-testable without touching the process cwd.
fn resolve_repo_with(
    explicit: Option<&str>,
    all_repos: bool,
    detect: impl FnOnce() -> Option<String>,
) -> Option<String> {
    if let Some(repo) = explicit {
        return Some(repo.to_string());
    }
    if all_repos {
        return None;
    }
    detect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detected() -> Option<String> {
        Some("/work/project".to_string())
    }

    fn undetectable() -> Option<String> {
        None
    }

    #[test]
    fn explicit_repo_overrides_detection() {
        assert_eq!(
            resolve_repo_with(Some("other"), false, detected),
            Some("other".to_string())
        );
    }

    #[test]
    fn explicit_repo_wins_even_with_all_repos() {
        // clap rejects the combination; the resolver still prefers the explicit
        // value rather than silently widening the scope.
        assert_eq!(
            resolve_repo_with(Some("other"), true, detected),
            Some("other".to_string())
        );
    }

    #[test]
    fn all_repos_disables_detection() {
        assert_eq!(
            resolve_repo_with(None, true, || panic!("must not detect")),
            None
        );
    }

    #[test]
    fn defaults_to_detected_root() {
        assert_eq!(
            resolve_repo_with(None, false, detected),
            Some("/work/project".to_string())
        );
    }

    #[test]
    fn no_repo_scope_outside_git() {
        assert_eq!(resolve_repo_with(None, false, undetectable), None);
    }
}
