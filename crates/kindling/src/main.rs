//! kindling — single-binary entry point.
//!
//! One artefact, `kindling`, exposes all three surfaces:
//!
//! - `kindling serve [...]` → start the UDS daemon.
//! - `kindling hook <type>` → run a Claude Code hook (stdin JSON → stdout,
//!   always exit 0).
//! - `kindling <cli-verb> [...]` → the CLI verbs (init, log, capsule, status,
//!   search, list, pin, unpin, export, import) — and `serve`, which is itself
//!   a CLI verb.
//!
//! ## Dispatch
//!
//! 1. **Symlink drop-in.** When the binary is invoked as `kindling-hook` (the
//!    `argv[0]` basename), it behaves as `kindling hook` — i.e.
//!    `kindling-hook <type>` is a drop-in for the old hook script name, taking
//!    the hook type from `argv[1]`.
//! 2. **`hook` subcommand.** `kindling hook <type>` runs the hook with the type
//!    from `argv[2]`. The `hook` surface is intercepted *before* clap, so it is
//!    never parsed by the CLI command tree (and therefore does not appear in
//!    `kindling --help`; see the usage note printed by `hook_usage`).
//! 3. **Everything else** delegates to [`kindling_cli::main`], which parses
//!    `std::env::args()` with clap. clap ignores `argv[0]`, so `serve`, the CLI
//!    verbs, `--help`, and `--version` all route here and parse correctly. The
//!    program name shown in help/errors is the `kindling` `argv[0]`.
//!
//! The hook surface owns its own current-thread Tokio runtime via
//! [`kindling_hook::run_hook`]; this `main` is a plain `fn main() -> ExitCode`
//! with no async runtime of its own.

use std::process::ExitCode;

/// The `argv[0]` basename that triggers the hook drop-in behaviour.
const HOOK_PROG_NAME: &str = "kindling-hook";

fn main() -> ExitCode {
    // (1) Symlink/rename drop-in: invoked as `kindling-hook` → hook mode, with
    //     the hook type taken from argv[1] (mirrors the old `kindling-hook`
    //     binary contract exactly).
    if invoked_as_hook() {
        return kindling_hook::run_hook(std::env::args().nth(1));
    }

    match std::env::args().nth(1).as_deref() {
        // (2) Explicit `kindling hook <type>`: type is argv[2].
        Some("hook") => {
            let type_arg = std::env::args().nth(2);
            if type_arg.is_none() {
                eprintln!("{}", hook_usage());
            }
            kindling_hook::run_hook(type_arg)
        }
        // (3) serve / CLI verbs / --help / --version → the CLI's clap dispatch.
        _ => delegate_to_cli(),
    }
}

/// Whether this process was invoked under the `kindling-hook` program name
/// (basename of `argv[0]`), e.g. via a symlink or rename.
fn invoked_as_hook() -> bool {
    let argv0 = std::env::args().next().unwrap_or_default();
    basename(&argv0) == HOOK_PROG_NAME
}

/// The trailing path component of `argv0`, without any directory prefix. Falls
/// back to the whole string when there is no separator.
fn basename(argv0: &str) -> &str {
    std::path::Path::new(argv0)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(argv0)
}

/// Run the CLI dispatch and map its `i32` exit code onto an [`ExitCode`].
///
/// A non-zero CLI exit must propagate (the release CI smoke-tests both
/// `kindling --version` succeeding and a bad verb failing). Codes are clamped
/// into the `u8` `ExitCode` range; the CLI only ever returns 0 or 1.
fn delegate_to_cli() -> ExitCode {
    let code = kindling_cli::main();
    ExitCode::from(code.clamp(0, 255) as u8)
}

/// A short usage line for the `hook` surface, printed when `kindling hook` is
/// run without a type. `hook` is intercepted before clap so it is absent from
/// `kindling --help`; this keeps the surface discoverable.
fn hook_usage() -> &'static str {
    "usage: kindling hook <type>\n  \
     type: session-start | post-tool-use | post-tool-use-failure | \
     user-prompt-submit | subagent-stop | stop | pre-compact\n\
     (reads the Claude Code hook context as JSON on stdin; always exits 0)"
}
