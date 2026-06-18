//! Hook error type.

use kindling_client::ClientError;
use thiserror::Error;

/// Errors a hook dispatch can produce. The binary logs these to stderr and
/// **always exits 0** — a hook must never block Claude Code.
#[derive(Debug, Error)]
pub enum HookError {
    /// `argv[1]` was not one of the seven known hook type strings.
    #[error("unknown hook type: {0}")]
    UnknownHookType(String),

    /// Stdin was not valid JSON.
    #[error("failed to parse stdin: {0}")]
    ParseStdin(String),

    /// A daemon call failed. Conflict (409) on capsule open is handled inside
    /// the dispatcher (treated as success) and never surfaces here.
    #[error(transparent)]
    Client(#[from] ClientError),

    /// Serializing the stdout envelope failed (should never happen).
    #[error("failed to serialize output: {0}")]
    Serialize(String),
}
