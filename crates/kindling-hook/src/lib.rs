//! Claude Code hook handlers (Rust).
//!
//! Reads a Claude Code hook context from stdin (JSON), dispatches to the
//! kindling daemon via [`kindling_client`], and writes the correct stdout JSON.
//! Each hook is a fresh, short-lived process; the binary **always exits 0** so a
//! hook can never block Claude Code (errors are logged to stderr instead).
//!
//! This crate is split into a library (testable [`dispatch`]) and a binary
//! (`kindling-hook`, `src/main.rs`) so integration tests can pipe JSON through
//! the real executable via `CARGO_BIN_EXE_kindling-hook`. The umbrella
//! `kindling hook <type>` wiring lands in PORT-013; here `argv[1]` is the hook
//! type string.
//!
//! # Fidelity to the Node hooks
//!
//! The capture mapping (content, provenance, scope) is a byte-for-byte port of
//! `packages/kindling-adapter-claude-code/src/claude-code/{mapping,provenance,
//! events}.ts`, using the adapter filter port in [`filter`] (see that module
//! for why it does **not** reuse `kindling-filter`). Known fidelity gaps vs the
//! Node path are documented on the relevant functions and in the task report:
//!   - captured observations are **not** attached to the session capsule (the
//!     daemon model appends with `capsuleId: None`);
//!   - `scopeIds.repoId` is the **raw cwd** (the Node `mapping.ts` quirk),
//!     while the *database* is routed by the git-toplevel project root;
//!   - `Input keys` / `inputKeys` ordering follows the deserialized JSON object
//!     key order.

mod error;
mod filter;
mod input;
mod mapping;
mod project;

pub use error::HookError;
pub use input::{HookInput, HookType};
pub use kindling_types::ObservationInput;
pub use project::project_root;

/// Pure capture mapping: turn a hook context into the [`ObservationInput`] that
/// would be appended, with no daemon involved. Returns `None` for hook types
/// that do not append an observation (`session-start`, `pre-compact`, `stop`)
/// or when a capture hook has nothing to store (an empty user prompt).
///
/// Exposed for byte-for-byte parity tests against the Node adapter fixtures;
/// [`dispatch`] uses the same mapping internally before appending.
pub fn map_capture(hook_type: HookType, input: &HookInput) -> Option<ObservationInput> {
    match hook_type {
        HookType::PostToolUse => Some(mapping::map_tool_use(input)),
        HookType::PostToolUseFailure => {
            let mut input = input.clone();
            let resolved_error = input
                .tool_error
                .clone()
                .filter(|s| !s.is_empty())
                .or_else(|| input.error.clone().filter(|s| !s.is_empty()))
                .unwrap_or_else(|| "Unknown error".to_string());
            input.tool_error = Some(resolved_error);
            Some(mapping::map_tool_use(&input))
        }
        HookType::UserPromptSubmit => {
            let content = input
                .content
                .as_deref()
                .filter(|s| !s.is_empty())
                .or(input.prompt.as_deref())
                .unwrap_or("");
            if content.trim().is_empty() {
                None
            } else {
                Some(mapping::map_user_prompt(input, content))
            }
        }
        HookType::SubagentStop => Some(mapping::map_subagent_stop(input)),
        HookType::SessionStart | HookType::PreCompact | HookType::Stop => None,
    }
}

use std::process::ExitCode;

use kindling_client::{Client, ClientConfig, ClientError, CloseCapsuleBody};
use kindling_types::{CapsuleType, ScopeIds};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Run a Claude Code hook end-to-end, owning a current-thread Tokio runtime.
///
/// This is the reusable entry point shared by the `kindling-hook` binary and
/// the umbrella `kindling hook <type>` / `kindling-hook` (symlink) dispatch. It:
///   - parses `type_arg` (the hook type string, normally `argv[1]`),
///   - reads the hook context JSON from stdin (empty stdin → empty object),
///   - resolves the project root and builds a daemon [`Client`],
///   - dispatches and writes any stdout JSON,
///   - logs ANY error to stderr in the Node format `[kindling] <Label> error:
///     <msg>` and **always returns [`ExitCode::SUCCESS`]** so a hook can never
///     block Claude Code.
///
/// Environment read (unchanged from the original binary):
///   - `KINDLING_REPO_ROOT` — project-root override (see [`project_root`]),
///   - `KINDLING_MAX_CONTEXT` — recent-observation cap for SessionStart,
///   - `KINDLING_SOCK` — daemon socket path override.
pub fn run_hook(type_arg: Option<String>) -> ExitCode {
    // Build a dedicated current-thread runtime. A failure here is itself
    // logged in the hook's never-block contract and still exits 0.
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("[kindling] hook error: building runtime: {e}");
            return ExitCode::SUCCESS;
        }
    };

    if let Err((label, message)) = runtime.block_on(run_hook_inner(type_arg)) {
        eprintln!("[kindling] {label} error: {message}");
    }
    ExitCode::SUCCESS
}

/// The fallible async body of [`run_hook`]. On error returns `(log_label,
/// message)` for the Node-style stderr line. The label is the hook's label when
/// known, else `"hook"`.
async fn run_hook_inner(type_arg: Option<String>) -> Result<(), (&'static str, String)> {
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

/// The hook-event name Claude Code expects in the SessionStart injection
/// envelope.
const SESSION_START_EVENT: &str = "SessionStart";
/// The hook-event name for the PreCompact injection envelope.
const PRE_COMPACT_EVENT: &str = "PreCompact";
/// Default intent for the session capsule. Mirrors the adapter's
/// `defaultIntent`.
const SESSION_INTENT: &str = "Claude Code session";
/// Default recent-observation cap when `KINDLING_MAX_CONTEXT` is unset.
const DEFAULT_MAX_CONTEXT: u32 = 10;

/// Dispatch a parsed hook context to the daemon and return the stdout JSON
/// string to print, or `None` to print nothing.
///
/// Never panics on daemon/content errors that the caller should swallow; those
/// surface as [`HookError`] for the binary to log before exiting 0. The one
/// exception is a `409 Conflict` from opening an already-open session capsule,
/// which this function treats as success.
pub async fn dispatch(
    hook_type: HookType,
    input: &HookInput,
    client: &Client,
) -> Result<Option<String>, HookError> {
    match hook_type {
        HookType::SessionStart => session_start(input, client).await,
        HookType::PreCompact => pre_compact(client).await,
        HookType::PostToolUse | HookType::PostToolUseFailure => {
            post_tool_use(hook_type, input, client).await
        }
        HookType::UserPromptSubmit => user_prompt_submit(input, client).await,
        HookType::SubagentStop => subagent_stop(input, client).await,
        HookType::Stop => stop(input, client).await,
    }
}

/// SessionStart: open the session capsule (tolerating an already-open one),
/// then return the daemon-assembled injection envelope when there is context.
async fn session_start(input: &HookInput, client: &Client) -> Result<Option<String>, HookError> {
    let session_id = session_start_session_id(input);
    let project_root = client.config().project_root.clone();

    let scope = ScopeIds {
        session_id: Some(session_id),
        repo_id: Some(project_root),
        ..Default::default()
    };

    // Open the capsule. A 409 means a capsule is already open for this session
    // (e.g. a re-fired SessionStart) — treat as success, never error.
    match client
        .open_capsule(CapsuleType::Session, SESSION_INTENT, scope, None)
        .await
    {
        Ok(_) => {}
        Err(ClientError::Api { status: 409, .. }) => {}
        Err(e) => return Err(e.into()),
    }

    // Inject prior context. `KINDLING_MAX_CONTEXT` (default 10) caps recency.
    let max_results = max_context_from_env();
    let ctx = client.session_start_context(Some(max_results)).await?;
    match ctx {
        Some(markdown) => Ok(Some(injection_envelope(SESSION_START_EVENT, &markdown)?)),
        None => Ok(None),
    }
}

/// PreCompact: forward the daemon-assembled pre-compact injection. Does NOT
/// open or close capsules.
async fn pre_compact(client: &Client) -> Result<Option<String>, HookError> {
    match client.pre_compact_context().await? {
        Some(markdown) => Ok(Some(injection_envelope(PRE_COMPACT_EVENT, &markdown)?)),
        None => Ok(None),
    }
}

/// PostToolUse / PostToolUseFailure: append a tool-use observation.
///
/// The failure hook reuses the same mapping; Node's `post-tool-use-failure.js`
/// routes through the identical `onPostToolUse` path with the error populated
/// (`tool_error || error || 'Unknown error'`). We reproduce that by filling a
/// default error when the failure hook carries none.
async fn post_tool_use(
    hook_type: HookType,
    input: &HookInput,
    client: &Client,
) -> Result<Option<String>, HookError> {
    if let Some(observation) = map_capture(hook_type, input) {
        // capsuleId None: observations are not attached to the session capsule
        // in the daemon model (known fidelity gap vs Node's attach). validate
        // true.
        client
            .append_observation(observation, None, Some(true))
            .await?;
    }
    Ok(None)
}

/// UserPromptSubmit: append a `message` observation. Mirrors the Node early
/// return when the content is empty/whitespace.
async fn user_prompt_submit(
    input: &HookInput,
    client: &Client,
) -> Result<Option<String>, HookError> {
    // Empty/whitespace prompts append nothing (Node's early return).
    if let Some(observation) = map_capture(HookType::UserPromptSubmit, input) {
        client
            .append_observation(observation, None, Some(true))
            .await?;
    }
    Ok(None)
}

/// SubagentStop: append a `node_end` observation.
///
/// The mapping reads `agent_type` and `output` directly (Node maps
/// `context.agent_type`/`context.output` into the event's
/// `agentType`/`agentOutput`).
async fn subagent_stop(input: &HookInput, client: &Client) -> Result<Option<String>, HookError> {
    if let Some(observation) = map_capture(HookType::SubagentStop, input) {
        client
            .append_observation(observation, None, Some(true))
            .await?;
    }
    Ok(None)
}

/// Stop: close the session's open capsule. No-op (success) when none is open.
async fn stop(input: &HookInput, client: &Client) -> Result<Option<String>, HookError> {
    let session_id = input.session_id_or_unknown();
    let capsule = client.get_open_capsule(&session_id).await?;
    let Some(capsule) = capsule else {
        // Node warns "session not found" and continues. Nothing to print.
        return Ok(None);
    };

    // Node `onStop` passes `{ reason, summaryContent: summary }`. The daemon's
    // close body takes `generate_summary` + `summary_content`; generate a
    // summary only when one was supplied.
    let summary = input.summary.clone();
    let body = CloseCapsuleBody {
        generate_summary: Some(summary.is_some()),
        summary_content: summary,
        confidence: None,
    };
    client.close_capsule(&capsule.id, body).await?;
    Ok(None)
}

/// Build the Claude Code injection envelope:
/// `{ "continue": true, "hookSpecificOutput": { "hookEventName": <event>,
/// "additionalContext": <markdown> } }`.
fn injection_envelope(event: &str, markdown: &str) -> Result<String, HookError> {
    let value = serde_json::json!({
        "continue": true,
        "hookSpecificOutput": {
            "hookEventName": event,
            "additionalContext": markdown,
        }
    });
    serde_json::to_string(&value).map_err(|e| HookError::Serialize(e.to_string()))
}

/// SessionStart session id: `context.session_id` or, when absent, the Node
/// `session-${Date.now()}` fallback.
fn session_start_session_id(input: &HookInput) -> String {
    match input.session_id.as_deref() {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => format!("session-{}", now_ms()),
    }
}

/// `KINDLING_MAX_CONTEXT` parsed as a `u32`, defaulting to 10 (matching the Node
/// `parseInt(... || '10', 10)`; a non-numeric value falls back to the default).
fn max_context_from_env() -> u32 {
    std::env::var("KINDLING_MAX_CONTEXT")
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .unwrap_or(DEFAULT_MAX_CONTEXT)
}

/// Current epoch milliseconds.
fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_type_parses_all_seven() {
        for (s, expected) in [
            ("session-start", HookType::SessionStart),
            ("post-tool-use", HookType::PostToolUse),
            ("post-tool-use-failure", HookType::PostToolUseFailure),
            ("user-prompt-submit", HookType::UserPromptSubmit),
            ("subagent-stop", HookType::SubagentStop),
            ("stop", HookType::Stop),
            ("pre-compact", HookType::PreCompact),
        ] {
            assert_eq!(HookType::parse(s).unwrap(), expected);
            assert_eq!(expected.as_str(), s);
        }
        assert!(matches!(
            HookType::parse("nope"),
            Err(HookError::UnknownHookType(_))
        ));
    }

    #[test]
    fn injection_envelope_shape() {
        let s = injection_envelope("SessionStart", "# md").unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["continue"], serde_json::json!(true));
        assert_eq!(v["hookSpecificOutput"]["hookEventName"], "SessionStart");
        assert_eq!(v["hookSpecificOutput"]["additionalContext"], "# md");
    }
}
