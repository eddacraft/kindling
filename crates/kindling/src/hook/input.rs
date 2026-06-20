//! Hook type + stdin input shapes.
//!
//! `HookInput` is the permissive, serde-deserialized stdin JSON Claude Code
//! passes to each hook process. Every field is optional; the dispatcher applies
//! the Node defaults (`session_id || "unknown"`, `cwd || cwd()`, etc.). Field
//! names match the snake_case keys the Node scripts read from `context.*`.

use serde::Deserialize;
use serde_json::Value;

use crate::hook::error::HookError;

/// The seven Claude Code hook types this binary handles. The string forms are
/// the `argv[1]` values the umbrella `kindling hook <type>` passes (PORT-013).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookType {
    /// `session-start`: open the session capsule, then inject prior context.
    SessionStart,
    /// `post-tool-use`: capture a successful tool call as an observation.
    PostToolUse,
    /// `post-tool-use-failure`: capture a failed tool call as an observation.
    PostToolUseFailure,
    /// `user-prompt-submit`: capture the user's message as an observation.
    UserPromptSubmit,
    /// `subagent-stop`: capture a subagent completion as an observation.
    SubagentStop,
    /// `stop`: close the session capsule.
    Stop,
    /// `pre-compact`: inject pinned items + latest summary before compaction.
    PreCompact,
}

impl HookType {
    /// Parse a hook type from its CLI string form.
    pub fn parse(s: &str) -> Result<Self, HookError> {
        Ok(match s {
            "session-start" => HookType::SessionStart,
            "post-tool-use" => HookType::PostToolUse,
            "post-tool-use-failure" => HookType::PostToolUseFailure,
            "user-prompt-submit" => HookType::UserPromptSubmit,
            "subagent-stop" => HookType::SubagentStop,
            "stop" => HookType::Stop,
            "pre-compact" => HookType::PreCompact,
            other => return Err(HookError::UnknownHookType(other.to_string())),
        })
    }

    /// The canonical CLI string for this hook type (used in log messages).
    pub fn as_str(self) -> &'static str {
        match self {
            HookType::SessionStart => "session-start",
            HookType::PostToolUse => "post-tool-use",
            HookType::PostToolUseFailure => "post-tool-use-failure",
            HookType::UserPromptSubmit => "user-prompt-submit",
            HookType::SubagentStop => "subagent-stop",
            HookType::Stop => "stop",
            HookType::PreCompact => "pre-compact",
        }
    }

    /// Human-readable label used in the Node-parity error prefix
    /// (`[kindling] <Label> error: …`).
    pub fn log_label(self) -> &'static str {
        match self {
            HookType::SessionStart => "SessionStart",
            HookType::PostToolUse => "PostToolUse",
            HookType::PostToolUseFailure => "PostToolUseFailure",
            HookType::UserPromptSubmit => "UserPromptSubmit",
            HookType::SubagentStop => "SubagentStop",
            HookType::Stop => "Stop",
            HookType::PreCompact => "PreCompact",
        }
    }
}

/// Deserialized hook stdin. All fields optional; the dispatcher fills Node
/// defaults. Unknown keys are ignored.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct HookInput {
    /// `context.session_id`. Defaulted to `"unknown"` by the dispatcher when
    /// absent (SessionStart uses a timestamped fallback — see the dispatcher).
    pub session_id: Option<String>,
    /// `context.cwd`. Defaulted to the process cwd when absent.
    pub cwd: String,
    /// `context.tool_name`.
    pub tool_name: Option<String>,
    /// `context.tool_input` — an object of tool arguments.
    pub tool_input: Option<Value>,
    /// `context.tool_result` — arbitrary JSON (success case).
    pub tool_result: Option<Value>,
    /// `context.tool_error` — error string (failure case).
    pub tool_error: Option<String>,
    /// `context.error` — alternate error field read by the failure hook.
    pub error: Option<String>,
    /// `context.content` — user message (UserPromptSubmit).
    pub content: Option<String>,
    /// `context.prompt` — alternate user-message field.
    pub prompt: Option<String>,
    /// `context.agent_type` — subagent type (SubagentStop).
    pub agent_type: Option<String>,
    /// `context.output` — subagent output (SubagentStop).
    pub output: Option<String>,
    /// `context.task` — subagent task description (carried for parity; unused in
    /// the captured observation, matching Node's `mapSubagentStopEvent`).
    pub task: Option<String>,
    /// `context.stop_reason` — Stop reason (primary).
    pub stop_reason: Option<String>,
    /// `context.reason` — Stop reason (alternate).
    pub reason: Option<String>,
    /// `context.summary` — final summary (Stop).
    pub summary: Option<String>,
}

/// A normalized view of `HookInput` after applying the Node defaults shared by
/// every hook: `session_id || "unknown"` and `cwd || process.cwd()`.
///
/// SessionStart's session-id fallback (`session-<ts>`) differs and is applied in
/// the dispatcher, not here.
impl HookInput {
    /// `session_id`, defaulting to `"unknown"`.
    pub fn session_id_or_unknown(&self) -> String {
        match self.session_id.as_deref() {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => "unknown".to_string(),
        }
    }

    /// `cwd`, defaulting to the process cwd when empty.
    pub fn cwd_or_process(&self) -> String {
        if self.cwd.is_empty() {
            std::env::current_dir()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default()
        } else {
            self.cwd.clone()
        }
    }
}
