//! Content filtering — a byte-for-byte port of the **adapter** filter at
//! `packages/kindling-adapter-claude-code/src/claude-code/filter.ts`.
//!
//! # Why not `kindling-filter`?
//!
//! The `kindling-filter` crate ports the **plugin hook** filter
//! (`plugins/kindling-claude-code/hooks/lib/filter.js`), which is a *different*
//! implementation from the adapter filter: different length limits (10k vs
//! 50k/10k), a different truncation notice (`\n[Truncated N chars]` vs
//! `\n\n[Truncated N characters]`), compact vs pretty `JSON.stringify`, a
//! noisy-tool special case, and different secret-pattern thresholds.
//!
//! What the production Node hooks actually *stored* went through the adapter
//! mapping (`mapping.ts`), which calls the adapter `filter.ts`. To reproduce the
//! stored observations byte-for-byte we must mirror the adapter filter, so this
//! module ports `filter.ts` rather than reusing `kindling-filter`. See the
//! crate-level docs and the final report for the full rationale.
//!
//! All lengths are measured in **UTF-16 code units**, matching JavaScript's
//! `String.prototype.length` / `substring`. When a truncation boundary falls
//! inside a surrogate pair we round the prefix down to the previous char
//! boundary (a lone surrogate could not survive a JSON round-trip anyway) while
//! still reporting the UTF-16 count JS would.

use std::sync::OnceLock;

use regex::Regex;

/// Maximum content length before truncation (UTF-16 code units).
///
/// Mirrors `MAX_CONTENT_LENGTH` in `filter.ts`. It is the default cap for
/// `filterContent` when no explicit length is given. The hook call sites always
/// pass an explicit limit (10k for user prompts, 5k for subagent output, [`MAX_RESULT_LENGTH`]
/// for Bash results), so this is exported for contract documentation and parity
/// tests rather than used by the dispatcher.
#[allow(dead_code)]
pub const MAX_CONTENT_LENGTH: usize = 50_000;

/// Maximum result length for tool results (UTF-16 code units).
/// Mirrors `MAX_RESULT_LENGTH` in `filter.ts`.
pub const MAX_RESULT_LENGTH: usize = 10_000;

/// Tools whose full results are never captured. Mirrors `SKIP_RESULT_TOOLS`.
const SKIP_RESULT_TOOLS: &[&str] = &["WebSearch"];

/// The adapter's `SECRET_PATTERNS`, in application order.
///
/// These differ from the plugin filter's patterns (which `kindling-filter`
/// uses); they are transcribed verbatim from `filter.ts`. The Rust `regex`
/// crate has no lookahead, so the "generic API token" pattern
/// (`\b(?=…)(?=…)[A-Za-z0-9]{32,}\b`) is rewritten without lookahead into an
/// equivalent that matches a 32+ alphanumeric run containing at least one digit
/// and at least one letter (see `GENERIC_TOKEN`).
fn secret_patterns() -> &'static [Regex] {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        [
            // API keys and tokens: key/value assignments.
            r#"(?i)['"]?(?:api[-_]?key|apikey|token|secret|password|passwd|pwd)['"]?\s*[:=]\s*['"]?[^\s'"]+['"]?"#,
            // AWS keys.
            r#"(?:AWS|aws)[-_]?(?:SECRET|secret)[-_]?(?:ACCESS|access)[-_]?(?:KEY|key)\s*[:=]\s*['"]?[A-Za-z0-9/+=]{40}['"]?"#,
            // Generic API tokens — see GenericToken below (lookahead-free).
            // Placeholder index 2; replaced by the custom matcher in mask/contains.
            r"\b[A-Za-z0-9]{32,}\b",
            // Bearer tokens.
            r"(?i)Bearer\s+[A-Za-z0-9\-._~+/]+=*",
            // Basic auth.
            r"(?i)Basic\s+[A-Za-z0-9+/]+=*",
            // Anthropic API keys.
            r"sk-ant-[A-Za-z0-9\-_]{90,}",
            // OpenAI API keys.
            r"sk-[A-Za-z0-9]{48,}",
        ]
        .iter()
        .map(|pattern| Regex::new(pattern).expect("secret pattern compiles"))
        .collect()
    })
}

/// Index of the lookahead-rewritten "generic API token" pattern in
/// [`secret_patterns`]. It matches any 32+ alphanumeric run; we additionally
/// require (in code) at least one digit AND at least one letter to match the
/// original `(?=.*[0-9])(?=.*[A-Za-z])` lookaheads.
const GENERIC_TOKEN_IDX: usize = 2;

/// Whether a candidate generic-token match has both a digit and a letter, per
/// the original JS lookaheads.
fn generic_token_qualifies(s: &str) -> bool {
    s.bytes().any(|b| b.is_ascii_digit()) && s.bytes().any(|b| b.is_ascii_alphabetic())
}

/// True if the content matches any known secret pattern. Mirrors
/// `containsSecrets`.
pub fn contains_secrets(content: &str) -> bool {
    for (idx, pattern) in secret_patterns().iter().enumerate() {
        if idx == GENERIC_TOKEN_IDX {
            if pattern
                .find_iter(content)
                .any(|m| generic_token_qualifies(m.as_str()))
            {
                return true;
            }
        } else if pattern.is_match(content) {
            return true;
        }
    }
    false
}

/// Mask secrets in content, mirroring the adapter `maskSecrets` replacement
/// callback: a match containing `:` or `=` becomes everything before the first
/// separator plus `=[REDACTED]`; any other match becomes `[REDACTED]`.
pub fn mask_secrets(content: &str) -> String {
    let mut masked = content.to_string();
    for (idx, pattern) in secret_patterns().iter().enumerate() {
        masked = pattern
            .replace_all(&masked, |caps: &regex::Captures<'_>| {
                let matched = &caps[0];
                if idx == GENERIC_TOKEN_IDX && !generic_token_qualifies(matched) {
                    // Not a real match for the lookahead-gated pattern: leave it.
                    return matched.to_string();
                }
                match matched.find([':', '=']) {
                    Some(sep) => format!("{}=[REDACTED]", &matched[..sep]),
                    None => "[REDACTED]".to_string(),
                }
            })
            .into_owned();
    }
    masked
}

/// Truncate content to `max_length` UTF-16 code units, appending the adapter
/// notice (`\n\n[Truncated N characters]`) when content was cut. Mirrors
/// `truncateContent` with `showTruncationNotice = true`.
pub fn truncate_content(content: &str, max_length: usize) -> String {
    let (prefix, total_units) = utf16_prefix(content, max_length);
    if total_units <= max_length {
        return content.to_string();
    }
    let remaining = total_units - max_length;
    format!("{prefix}\n\n[Truncated {remaining} characters]")
}

/// Filter content with all safety rules: mask secrets when detected, then
/// truncate to `max_length`. Mirrors `filterContent` (with `maskSecrets`
/// defaulting on).
pub fn filter_content(content: &str, max_length: usize) -> String {
    let filtered = if contains_secrets(content) {
        mask_secrets(content)
    } else {
        content.to_string()
    };
    truncate_content(&filtered, max_length)
}

/// Whether a tool's result should be captured. Mirrors `shouldCaptureToolResult`.
pub fn should_capture_tool_result(tool_name: &str) -> bool {
    !SKIP_RESULT_TOOLS.contains(&tool_name)
}

/// Filter a tool result for storage. Mirrors `filterToolResult`:
/// - skip-result tools → `Some("[Result not captured]")`;
/// - null/absent result → `None`;
/// - string result → used as-is;
/// - any other JSON value → `JSON.stringify(result, null, 2)` (two-space pretty
///   print).
///
/// The chosen string is then run through [`filter_content`] with `max_length`.
pub fn filter_tool_result(
    tool_name: &str,
    result: Option<&serde_json::Value>,
    max_length: usize,
) -> Option<String> {
    if !should_capture_tool_result(tool_name) {
        return Some("[Result not captured]".to_string());
    }
    let result = match result {
        Some(serde_json::Value::Null) | None => return None,
        Some(v) => v,
    };
    let result_str = match result {
        serde_json::Value::String(s) => s.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string()),
    };
    Some(filter_content(&result_str, max_length))
}

/// Longest prefix of `content` that fits in `max_units` UTF-16 code units
/// without splitting a surrogate pair, plus the total UTF-16 length.
fn utf16_prefix(content: &str, max_units: usize) -> (&str, usize) {
    let mut units = 0;
    for (byte_idx, ch) in content.char_indices() {
        let ch_units = ch.len_utf16();
        if units + ch_units > max_units {
            let total = units
                + content[byte_idx..]
                    .chars()
                    .map(char::len_utf16)
                    .sum::<usize>();
            return (&content[..byte_idx], total);
        }
        units += ch_units;
    }
    (content, units)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn short_content_unchanged() {
        assert_eq!(filter_content("hello", 50_000), "hello");
    }

    #[test]
    fn truncation_uses_adapter_notice() {
        let content = "x".repeat(15);
        assert_eq!(
            truncate_content(&content, 10),
            format!("{}\n\n[Truncated 5 characters]", "x".repeat(10))
        );
    }

    #[test]
    fn utf16_length_like_javascript() {
        // "🦀🦀🦀".length === 6 in JS.
        let content = "🦀🦀🦀";
        assert_eq!(truncate_content(content, 6), content);
        assert_eq!(
            truncate_content(content, 4),
            "🦀🦀\n\n[Truncated 2 characters]"
        );
    }

    #[test]
    fn bash_string_result_passes_through() {
        assert_eq!(
            filter_tool_result("Bash", Some(&json!("output line")), MAX_RESULT_LENGTH),
            Some("output line".to_string())
        );
    }

    #[test]
    fn bash_object_result_is_pretty_json() {
        let r = json!({ "exitCode": 0 });
        assert_eq!(
            filter_tool_result("Bash", Some(&r), MAX_RESULT_LENGTH),
            Some("{\n  \"exitCode\": 0\n}".to_string())
        );
    }

    #[test]
    fn websearch_result_not_captured() {
        assert_eq!(
            filter_tool_result("WebSearch", Some(&json!("anything")), MAX_RESULT_LENGTH),
            Some("[Result not captured]".to_string())
        );
    }

    #[test]
    fn null_result_is_none() {
        assert_eq!(
            filter_tool_result("Bash", Some(&serde_json::Value::Null), MAX_RESULT_LENGTH),
            None
        );
        assert_eq!(filter_tool_result("Bash", None, MAX_RESULT_LENGTH), None);
    }

    #[test]
    fn secret_assignment_is_masked() {
        let masked = filter_content("api_key=supersecretvalue123", 50_000);
        assert!(masked.contains("[REDACTED]"), "got {masked}");
        assert!(!masked.contains("supersecretvalue123"), "got {masked}");
    }
}
