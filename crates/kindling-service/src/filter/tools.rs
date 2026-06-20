//! Tool-result capture rules.
//!
//! Ports `SKIP_TOOLS`, `NOISY_TOOLS`, `shouldCaptureTool`, `isNoisyTool`,
//! and `filterToolResult` from
//! `plugins/kindling-claude-code/hooks/lib/filter.js`.

use serde_json::Value;

use super::secrets::mask_secrets;
use super::truncate::{truncate, MAX_CONTENT_LENGTH, NOISY_TOOL_MAX_LENGTH};

/// Tools whose results are skipped entirely (too noisy to store).
const SKIP_TOOLS: &[&str] = &["WebSearch"];

/// Tools whose results are truncated more aggressively.
const NOISY_TOOLS: &[&str] = &["Grep", "Glob"];

/// True unless the tool's results are configured to be skipped.
pub fn should_capture_tool(tool_name: &str) -> bool {
    !SKIP_TOOLS.contains(&tool_name)
}

/// True if the tool's results get the tighter truncation limit.
pub fn is_noisy_tool(tool_name: &str) -> bool {
    NOISY_TOOLS.contains(&tool_name)
}

/// Filter a tool result for storage. `None` in, `None` out (mirrors the
/// Node.js `null`/`undefined` handling — `Value::Null` counts as absent).
///
/// Non-string values are serialized with `serde_json::to_string`, matching
/// the Node.js `JSON.stringify(result)` call — with the caveat that object
/// key order follows the `Value`'s map order rather than JS insertion order.
pub fn filter_tool_result(tool_name: &str, result: Option<&Value>) -> Option<String> {
    if !should_capture_tool(tool_name) {
        return Some("[Result not captured]".to_string());
    }

    let result = match result {
        None | Some(Value::Null) => return None,
        Some(value) => value,
    };

    let result_str = match result {
        Value::String(s) => s.clone(),
        other => serde_json::to_string(other).unwrap_or_else(|_| other.to_string()),
    };

    let max_len = if is_noisy_tool(tool_name) {
        NOISY_TOOL_MAX_LENGTH
    } else {
        MAX_CONTENT_LENGTH
    };
    Some(truncate(&mask_secrets(&result_str), max_len))
}
