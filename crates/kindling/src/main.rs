//! kindling — single-binary entry point.
//!
//! One artefact, `kindling`, exposes all three surfaces:
//!
//! - `kindling serve [...]` → start the UDS daemon.
//! - `kindling hook <type>` → run a Claude Code hook (stdin JSON → stdout,
//!   always exit 0).
//! - `kindling <cli-verb> [...]` → the CLI verbs (init, log, capsule, status,
//!   search, list, pin, unpin, export, import) — and `serve`.
//!
//! Dispatch: (1) invoked as `kindling-hook` (argv[0] basename) → hook mode,
//! type from argv[1]; (2) `kindling hook <type>` → hook, type from argv[2],
//! intercepted before clap; (3) everything else → the clap CLI dispatch.
//! The hook surface owns its own current-thread Tokio runtime via
//! [`kindling::hook::run_hook`]; this `main` is a plain `fn main() -> ExitCode`.

use std::process::ExitCode;

/// The `argv[0]` basename that triggers the hook drop-in behaviour.
const HOOK_PROG_NAME: &str = "kindling-hook";

fn main() -> ExitCode {
    if invoked_as_hook() {
        return kindling::hook::run_hook(std::env::args().nth(1));
    }

    match std::env::args().nth(1).as_deref() {
        Some("hook") => {
            let type_arg = std::env::args().nth(2);
            if type_arg.is_none() {
                eprintln!("{}", hook_usage());
            }
            kindling::hook::run_hook(type_arg)
        }
        _ => delegate_to_cli(),
    }
}

/// Whether this process was invoked under the `kindling-hook` program name.
fn invoked_as_hook() -> bool {
    let argv0 = std::env::args().next().unwrap_or_default();
    basename(&argv0) == HOOK_PROG_NAME
}

/// The trailing path component of `argv0`, without any directory prefix.
fn basename(argv0: &str) -> &str {
    std::path::Path::new(argv0)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(argv0)
}

/// Run the CLI dispatch and map its `i32` exit code onto an [`ExitCode`].
fn delegate_to_cli() -> ExitCode {
    let code = kindling::cli_main();
    ExitCode::from(code.clamp(0, 255) as u8)
}

/// A short usage line for the `hook` surface.
fn hook_usage() -> &'static str {
    "usage: kindling hook <type>\n  \
     type: session-start | post-tool-use | post-tool-use-failure | \
     user-prompt-submit | subagent-stop | stop | pre-compact\n\
     (reads the Claude Code hook context as JSON on stdin; always exits 0)"
}
