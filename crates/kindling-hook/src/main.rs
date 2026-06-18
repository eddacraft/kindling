//! `kindling-hook <type>` — the Claude Code hook entry point.
//!
//! Contract:
//!   - `argv[1]` is the hook type (`session-start`, `post-tool-use`,
//!     `post-tool-use-failure`, `user-prompt-submit`, `subagent-stop`, `stop`,
//!     `pre-compact`).
//!   - stdin is the hook context JSON Claude Code passes.
//!   - on success, the dispatch result (if any) is written to stdout.
//!   - the process **always exits 0**; any error is logged to stderr in the
//!     Node format `[kindling] <Label> error: <msg>` and never blocks Claude
//!     Code.
//!
//! Environment read:
//!   - `KINDLING_REPO_ROOT` — project-root override.
//!   - `KINDLING_MAX_CONTEXT` — recent-observation cap for SessionStart.
//!   - `KINDLING_SOCK` — daemon socket path override (defaults to the standard
//!     `~/.kindling/kindling.sock`). Primarily for tests pointing at an
//!     in-process daemon on a temp socket.
//!
//! The actual run logic lives in the library ([`kindling_hook::run_hook`]) so
//! the umbrella `kindling` binary can reuse it for both `kindling hook <type>`
//! and the `kindling-hook` symlink drop-in (PORT-013).

use std::process::ExitCode;

fn main() -> ExitCode {
    kindling_hook::run_hook(std::env::args().nth(1))
}
