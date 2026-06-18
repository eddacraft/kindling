//! Per-command handlers. Each module exposes `run`-style functions invoked by
//! the dispatch in `lib.rs`. Handlers print their own output (text or `--json`)
//! and return [`CliResult`](crate::CliResult).

pub mod capsule;
pub mod export;
pub mod init;
pub mod list;
pub mod log;
pub mod pin;
pub mod search;
pub mod serve;
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
