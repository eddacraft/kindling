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
//!   - `KINDLING_REPO_ROOT` — project-root override (see [`project_root`]).
//!   - `KINDLING_MAX_CONTEXT` — recent-observation cap for SessionStart.
//!   - `KINDLING_SOCK` — daemon socket path override (defaults to the standard
//!     `~/.kindling/kindling.sock`). Primarily for tests pointing at an
//!     in-process daemon on a temp socket.

use std::process::ExitCode;

use kindling_client::{Client, ClientConfig};
use kindling_hook::{dispatch, project_root, HookInput, HookType};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    // Run the whole hook; on ANY error, log and still exit 0.
    if let Err((label, message)) = run().await {
        eprintln!("[kindling] {label} error: {message}");
    }
    ExitCode::SUCCESS
}

/// The fallible body. On error returns `(log_label, message)` for the Node-style
/// stderr line. The label is the hook's label when known, else `"hook"`.
async fn run() -> Result<(), (&'static str, String)> {
    // argv[1] = hook type.
    let type_arg = std::env::args().nth(1);
    let Some(type_arg) = type_arg else {
        return Err(("hook", "missing hook type argument".to_string()));
    };
    let hook_type = HookType::parse(&type_arg).map_err(|e| ("hook", e.to_string()))?;
    let label = hook_type.log_label();

    // Read all of stdin.
    let mut buf = Vec::new();
    let mut stdin = tokio::io::stdin();
    stdin
        .read_to_end(&mut buf)
        .await
        .map_err(|e| (label, format!("reading stdin: {e}")))?;

    // Parse the hook context. An empty stdin is treated as an empty object.
    let input: HookInput = if buf.iter().all(u8::is_ascii_whitespace) {
        HookInput::default()
    } else {
        serde_json::from_slice(&buf).map_err(|e| (label, format!("failed to parse stdin: {e}")))?
    };

    // Resolve the project root for DB routing and build the client.
    let cwd = input.cwd_or_process();
    let root = project_root(&cwd);
    let client = build_client(root).map_err(|e| (label, e))?;

    // Dispatch and print any returned stdout JSON.
    let output = dispatch(hook_type, &input, &client)
        .await
        .map_err(|e| (label, e.to_string()))?;
    if let Some(s) = output {
        let mut stdout = tokio::io::stdout();
        stdout
            .write_all(s.as_bytes())
            .await
            .map_err(|e| (label, format!("writing stdout: {e}")))?;
        stdout
            .flush()
            .await
            .map_err(|e| (label, format!("flushing stdout: {e}")))?;
    }
    Ok(())
}

/// Build a [`Client`] routed at `project_root`, honouring the `KINDLING_SOCK`
/// socket override when set.
fn build_client(project_root: String) -> Result<Client, String> {
    let mut config = ClientConfig::defaults().map_err(|e| format!("client config: {e}"))?;
    config.project_root = project_root;
    if let Some(sock) = std::env::var_os("KINDLING_SOCK") {
        config.socket_path = sock.into();
    }
    Ok(Client::with_config(config))
}
