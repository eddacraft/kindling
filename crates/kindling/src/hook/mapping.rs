//! Event → observation mapping.
//!
//! Byte-for-byte port of the Node adapter's capture mapping:
//! `packages/kindling-adapter-claude-code/src/claude-code/{mapping.ts,
//! provenance.ts}` (content formatting + provenance), using the adapter
//! filter port in [`crate::hook::filter`].
//!
//! Each builder returns an [`ObservationInput`] with `scopeIds`
//! `{ sessionId, repoId: <raw cwd> }`. **NOTE:** Node's `mapping.ts` uses the
//! *raw* `cwd` for `repoId` (not the git-toplevel project root). We preserve
//! that quirk exactly — only the daemon's *database routing* uses the project
//! root (via the `X-Kindling-Project` header); the stored `scopeIds.repoId`
//! field is the raw cwd, matching Node.

use kindling_types::{ObservationInput, ObservationKind, ScopeIds};
use serde_json::{Map, Value};

use crate::hook::filter::{filter_content, filter_tool_result, MAX_RESULT_LENGTH};
use crate::hook::input::HookInput;

/// Map a tool name to its observation kind. Mirrors `TOOL_TO_KIND_MAP` with the
/// `?? 'tool_call'` default for unknown tools.
fn tool_to_kind(tool_name: &str) -> ObservationKind {
    match tool_name {
        "Write" | "Edit" => ObservationKind::FileDiff,
        "Bash" => ObservationKind::Command,
        // Read | Glob | Grep | Task | WebFetch | WebSearch | AskUserQuestion |
        // Skill | <unknown> → tool_call
        _ => ObservationKind::ToolCall,
    }
}

/// `event.toolInput?.<key>` as a `&str`, if present and a string.
fn input_str<'a>(input: &'a Value, key: &str) -> Option<&'a str> {
    input.get(key).and_then(Value::as_str)
}

/// Build the `scopeIds` for a captured observation: `{ sessionId, repoId: cwd }`
/// where `cwd` is the **raw** cwd (the Node quirk) and `sessionId` is the Node
/// `session_id || 'unknown'` default.
fn capture_scope(hook: &HookInput) -> ScopeIds {
    ScopeIds {
        session_id: Some(hook.session_id_or_unknown()),
        repo_id: Some(hook.cwd_or_process()),
        ..Default::default()
    }
}

/// Map a PostToolUse / PostToolUseFailure event to an observation. Ports
/// `mapToolUseEvent` + `formatToolContent` + `extractToolUseProvenance`.
pub fn map_tool_use(hook: &HookInput) -> ObservationInput {
    let tool_name = hook.tool_name.as_deref().unwrap_or("unknown");
    let kind = tool_to_kind(tool_name);
    let content = format_tool_content(hook, tool_name);
    let provenance = extract_tool_use_provenance(hook, tool_name);

    ObservationInput {
        id: None,
        kind,
        content,
        provenance: Some(provenance),
        ts: None,
        scope_ids: capture_scope(hook),
        redacted: None,
    }
}

/// Map a UserPromptSubmit event to a `message` observation. Ports
/// `mapUserPromptEvent` + `extractUserPromptProvenance`.
pub fn map_user_prompt(hook: &HookInput, user_content: &str) -> ObservationInput {
    let content = filter_content(user_content, 10_000);
    let mut provenance = Map::new();
    provenance.insert("role".to_string(), Value::String("user".to_string()));
    provenance.insert(
        "length".to_string(),
        Value::from(utf16_len(user_content) as u64),
    );

    ObservationInput {
        id: None,
        kind: ObservationKind::Message,
        content,
        provenance: Some(provenance),
        ts: None,
        scope_ids: capture_scope(hook),
        redacted: None,
    }
}

/// Map a SubagentStop event to a `node_end` observation. Ports
/// `mapSubagentStopEvent` + `formatSubagentContent` +
/// `extractSubagentProvenance`.
pub fn map_subagent_stop(hook: &HookInput) -> ObservationInput {
    let agent_type = hook.agent_type.as_deref();
    let agent_output = hook.output.as_deref();

    // formatSubagentContent: `Subagent: <type||'unknown'>` then, if output,
    // `Output:\n<filtered(5000)>`, joined by "\n\n".
    let mut parts: Vec<String> = vec![format!("Subagent: {}", agent_type.unwrap_or("unknown"))];
    if let Some(output) = agent_output {
        let filtered = filter_content(output, 5_000);
        parts.push(format!("Output:\n{filtered}"));
    }
    let content = parts.join("\n\n");

    // extractSubagentProvenance.
    let mut provenance = Map::new();
    provenance.insert(
        "agentType".to_string(),
        match agent_type {
            Some(t) => Value::String(t.to_string()),
            // JS `event.agentType` would be `undefined` → JSON.stringify drops
            // the key. We omit it too (never serialize null).
            None => return finish_subagent(content, provenance, hook),
        },
    );
    provenance.insert("hasOutput".to_string(), Value::Bool(agent_output.is_some()));
    provenance.insert(
        "outputLength".to_string(),
        Value::from(agent_output.map(utf16_len).unwrap_or(0) as u64),
    );

    ObservationInput {
        id: None,
        kind: ObservationKind::NodeEnd,
        content,
        provenance: Some(provenance),
        ts: None,
        scope_ids: capture_scope(hook),
        redacted: None,
    }
}

/// Finish a subagent observation when `agentType` is absent (key omitted) but
/// the remaining provenance keys still apply.
fn finish_subagent(
    content: String,
    mut provenance: Map<String, Value>,
    hook: &HookInput,
) -> ObservationInput {
    let agent_output = hook.output.as_deref();
    provenance.insert("hasOutput".to_string(), Value::Bool(agent_output.is_some()));
    provenance.insert(
        "outputLength".to_string(),
        Value::from(agent_output.map(utf16_len).unwrap_or(0) as u64),
    );
    ObservationInput {
        id: None,
        kind: ObservationKind::NodeEnd,
        content,
        provenance: Some(provenance),
        ts: None,
        scope_ids: capture_scope(hook),
        redacted: None,
    }
}

/// Port of `formatToolContent`. Parts joined by `"\n\n"`, first `Tool: <name>`.
fn format_tool_content(hook: &HookInput, tool_name: &str) -> String {
    let empty = Value::Object(Map::new());
    let input = hook.tool_input.as_ref().unwrap_or(&empty);
    let mut parts: Vec<String> = vec![format!("Tool: {tool_name}")];

    match tool_name {
        "Read" => {
            if let Some(fp) = input_str(input, "file_path") {
                parts.push(format!("File: {fp}"));
            }
        }
        "Write" => {
            if let Some(fp) = input_str(input, "file_path") {
                parts.push(format!("File: {fp}"));
            }
            parts.push("Action: Created/overwrote file".to_string());
        }
        "Edit" => {
            if let Some(fp) = input_str(input, "file_path") {
                parts.push(format!("File: {fp}"));
            }
            parts.push("Action: Edited file".to_string());
        }
        "Bash" => {
            if let Some(cmd) = input_str(input, "command") {
                parts.push(format!("$ {cmd}"));
            }
            if let Some(result_str) =
                filter_tool_result(tool_name, hook.tool_result.as_ref(), MAX_RESULT_LENGTH)
            {
                // JS pushes only truthy strings; an empty string is falsy and
                // is skipped.
                if !result_str.is_empty() {
                    parts.push(result_str);
                }
            }
        }
        "Glob" | "Grep" => {
            if let Some(p) = input_str(input, "pattern") {
                parts.push(format!("Pattern: {p}"));
            }
            if let Some(p) = input_str(input, "path") {
                parts.push(format!("Path: {p}"));
            }
        }
        "Task" => {
            if let Some(a) = input_str(input, "subagent_type") {
                parts.push(format!("Agent: {a}"));
            }
            if let Some(d) = input_str(input, "description") {
                parts.push(format!("Task: {d}"));
            }
        }
        "WebFetch" => {
            if let Some(u) = input_str(input, "url") {
                parts.push(format!("URL: {u}"));
            }
        }
        "WebSearch" => {
            if let Some(q) = input_str(input, "query") {
                parts.push(format!("Query: {q}"));
            }
        }
        _ => {
            // Unknown tool: show input keys (insertion order) when toolInput is
            // an object. `Object.keys({}).join(', ')` === "" → still pushed,
            // matching `if (event.toolInput)` (truthy even when empty).
            if let Some(obj) = input.as_object() {
                let keys: Vec<&str> = obj.keys().map(String::as_str).collect();
                parts.push(format!("Input keys: {}", keys.join(", ")));
            }
        }
    }

    if let Some(err) = hook.tool_error.as_deref() {
        parts.push(format!("Error: {err}"));
    }

    parts.join("\n\n")
}

/// Port of `extractToolUseProvenance`. Returns an object that OMITS keys whose
/// JS value would be `undefined` (never serializes them as null).
fn extract_tool_use_provenance(hook: &HookInput, tool_name: &str) -> Map<String, Value> {
    let empty = Value::Object(Map::new());
    let input = hook.tool_input.as_ref().unwrap_or(&empty);
    let mut p = Map::new();

    p.insert("toolName".to_string(), Value::String(tool_name.to_string()));
    p.insert(
        "hasError".to_string(),
        Value::Bool(hook.tool_error.is_some()),
    );

    // Helper: set a string key only when present (else omit, like a JS
    // `undefined` value dropped by JSON.stringify).
    let set_opt_str = |p: &mut Map<String, Value>, key: &str, jskey: &str| {
        if let Some(v) = input_str(input, key) {
            p.insert(jskey.to_string(), Value::String(v.to_string()));
        }
    };

    match tool_name {
        "Read" | "Write" => {
            set_opt_str(&mut p, "file_path", "filePath");
        }
        "Edit" => {
            set_opt_str(&mut p, "file_path", "filePath");
            p.insert(
                "hasOldString".to_string(),
                Value::Bool(input.get("old_string").map(is_truthy).unwrap_or(false)),
            );
        }
        "Bash" => {
            // command = first whitespace token of the command, if any (else key
            // omitted — JS `extractCommandName(undefined)` is `undefined`).
            if let Some(name) = command_name(input_str(input, "command")) {
                p.insert("command".to_string(), Value::String(name));
            }
            // exitCode = number from result .exitCode / .exit_code if present.
            if let Some(code) = extract_exit_code(hook.tool_result.as_ref()) {
                p.insert("exitCode".to_string(), Value::from(code));
            }
        }
        "Glob" | "Grep" => {
            set_opt_str(&mut p, "pattern", "pattern");
            set_opt_str(&mut p, "path", "path");
        }
        "Task" => {
            set_opt_str(&mut p, "subagent_type", "subagentType");
            set_opt_str(&mut p, "description", "description");
        }
        "WebFetch" => {
            set_opt_str(&mut p, "url", "url");
        }
        "WebSearch" => {
            set_opt_str(&mut p, "query", "query");
        }
        _ => {
            // Unknown tool: inputKeys = array of input keys (insertion order),
            // only when toolInput is present (truthy). `Object.keys`.
            if let Some(obj) = input.as_object() {
                let keys: Vec<Value> = obj.keys().map(|k| Value::String(k.clone())).collect();
                p.insert("inputKeys".to_string(), Value::Array(keys));
            }
        }
    }

    p
}

/// JS truthiness for `!!event.toolInput?.old_string` and similar: any non-empty
/// string, non-zero number, `true`, non-null object/array is truthy. We only
/// need the cases that appear for `old_string` (a string), but keep this
/// general for safety.
fn is_truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// `extractCommandName`: trim, split on whitespace, take the first token; `None`
/// when the command is absent or yields an empty first token.
fn command_name(command: Option<&str>) -> Option<String> {
    let command = command?;
    let trimmed = command.trim();
    let first = trimmed.split_whitespace().next()?;
    if first.is_empty() {
        None
    } else {
        Some(first.to_string())
    }
}

/// `extractExitCode`: an integer `exitCode` or `exit_code` from an object
/// result. JS checks `typeof === 'number'`; we accept any JSON number and
/// return it as `i64` (exit codes are integers).
fn extract_exit_code(result: Option<&Value>) -> Option<i64> {
    let obj = result?.as_object()?;
    for key in ["exitCode", "exit_code"] {
        if let Some(n) = obj.get(key).and_then(Value::as_i64) {
            return Some(n);
        }
    }
    None
}

/// UTF-16 code-unit length of a string, matching JS `String.prototype.length`.
fn utf16_len(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn hook_with(
        tool: &str,
        input: Value,
        result: Option<Value>,
        error: Option<&str>,
    ) -> HookInput {
        HookInput {
            session_id: Some("s1".to_string()),
            cwd: "/repo".to_string(),
            tool_name: Some(tool.to_string()),
            tool_input: Some(input),
            tool_result: result,
            tool_error: error.map(String::from),
            ..HookInput::default()
        }
    }

    #[test]
    fn read_content_and_provenance() {
        let hook = hook_with("Read", json!({ "file_path": "/a.rs" }), None, None);
        let obs = map_tool_use(&hook);
        assert_eq!(obs.kind, ObservationKind::ToolCall);
        assert_eq!(obs.content, "Tool: Read\n\nFile: /a.rs");
        let p = obs.provenance.unwrap();
        assert_eq!(p["toolName"], json!("Read"));
        assert_eq!(p["hasError"], json!(false));
        assert_eq!(p["filePath"], json!("/a.rs"));
    }

    #[test]
    fn bash_with_exit_code() {
        let hook = hook_with(
            "Bash",
            json!({ "command": "cargo build --workspace" }),
            Some(json!({ "exitCode": 0 })),
            None,
        );
        let obs = map_tool_use(&hook);
        assert_eq!(obs.kind, ObservationKind::Command);
        let p = obs.provenance.unwrap();
        assert_eq!(p["command"], json!("cargo"));
        assert_eq!(p["exitCode"], json!(0));
    }

    #[test]
    fn unknown_tool_input_keys() {
        let hook = hook_with("Frobnicate", json!({ "alpha": 1, "beta": 2 }), None, None);
        let obs = map_tool_use(&hook);
        assert_eq!(obs.content, "Tool: Frobnicate\n\nInput keys: alpha, beta");
        let p = obs.provenance.unwrap();
        assert_eq!(p["inputKeys"], json!(["alpha", "beta"]));
    }

    #[test]
    fn error_appended_to_content_and_provenance() {
        let hook = hook_with("Read", json!({ "file_path": "/a" }), None, Some("boom"));
        let obs = map_tool_use(&hook);
        assert!(obs.content.ends_with("Error: boom"));
        assert_eq!(obs.provenance.unwrap()["hasError"], json!(true));
    }
}
