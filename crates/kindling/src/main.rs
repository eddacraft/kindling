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
//!
//! argv is read as OS strings. `std::env::args()` panics on non-UTF-8 bytes,
//! which would skip the hook's always-zero handler.

use std::ffi::{OsStr, OsString};
use std::process::ExitCode;

/// The `argv[0]` basename that triggers the hook drop-in behaviour.
const HOOK_PROG_NAME: &str = "kindling-hook";

fn main() -> ExitCode {
    if invoked_as_hook() {
        return kindling::hook::run_hook(nth_arg_os(1).map(os_to_string));
    }

    match nth_arg_os(1).as_deref() {
        Some(arg) if arg == "hook" => {
            let type_arg = nth_arg_os(2).map(os_to_string);
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
    let argv0 = std::env::args_os().next().unwrap_or_default();
    basename(&argv0) == OsStr::new(HOOK_PROG_NAME)
}

/// The trailing path component of `argv0`, without any directory prefix.
fn basename(argv0: &OsStr) -> &OsStr {
    std::path::Path::new(argv0).file_name().unwrap_or(argv0)
}

/// The `n`th process argument as an [`OsString`], without requiring UTF-8.
fn nth_arg_os(n: usize) -> Option<OsString> {
    std::env::args_os().nth(n)
}

/// Lossily convert an OS argument so [`kindling::hook::run_hook`] can parse it.
/// Invalid UTF-8 becomes the replacement character and is then rejected as an
/// unknown hook type — still via the always-zero path.
fn os_to_string(arg: OsString) -> String {
    arg.into_string()
        .unwrap_or_else(|arg| arg.to_string_lossy().into_owned())
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
