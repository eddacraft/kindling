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
            // API keys and tokens: key/value assignments (including prefixed/suffixed key names).
            r#"(?i)['"]?(?:[A-Za-z0-9_-]*(?:api[-_]?key|apikey|token|secret|password|passwd|pwd)[A-Za-z0-9_-]*|(?:private|access|auth|client|signing|encryption|ssh|master)[-_]?key)['"]?\s*[:=]\s*['"]?[^\s'"]+['"]?"#,
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
            // GitHub tokens (PAT, OAuth, app, refresh).
            r"gh[pousr]_[A-Za-z0-9]{36,}",
            // AWS access key IDs.
            r"AKIA[0-9A-Z]{16}",
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
/// Bearer tokens. Base64 padding is `=`, so this match must not be split on `=`.
const BEARER_TOKEN_IDX: usize = 3;
/// Basic credentials. Same padding rule as [`BEARER_TOKEN_IDX`].
const BASIC_AUTH_IDX: usize = 4;

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
/// callback for key/value assignments: a match containing `:` or `=` becomes
/// everything before the first separator plus `=[REDACTED]`. Bearer and Basic
/// matches are replaced wholesale. Their base64 padding contains `=`, and
/// splitting on that `=` would keep the credential. Every other match becomes
/// `[REDACTED]`.
///
/// This is a cleartext-logging sanitizer. CodeQL is taught that via
/// `.github/codeql/extensions/kindling-rust`.
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
                redact_secret_match(idx, matched)
            })
            .into_owned();
    }
    masked
}

fn redact_secret_match(pattern_idx: usize, matched: &str) -> String {
    if pattern_idx == BEARER_TOKEN_IDX || pattern_idx == BASIC_AUTH_IDX {
        return "[REDACTED]".to_string();
    }
    match matched.find([':', '=']) {
        Some(sep) => format!("{}=[REDACTED]", &matched[..sep]),
        None => "[REDACTED]".to_string(),
    }
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

/// Filter content with all safety rules: mask secrets when detected, sanitize
/// URLs carrying credentials/queries/fragments, then truncate to `max_length`.
/// Mirrors `filterContent` (with `maskSecrets` defaulting on).
pub fn filter_content(content: &str, max_length: usize) -> String {
    let url_clean = sanitize_urls_in_text(content);
    let filtered = if contains_secrets(&url_clean) {
        mask_secrets(&url_clean)
    } else {
        url_clean
    };
    truncate_content(&filtered, max_length)
}

/// Strip userinfo (`user:pass@`), query strings (`?...`), and fragments
/// (`#...`) from a URL string, then mask any secrets remaining in the host or
/// path. Clean URLs (no userinfo, query, or fragment) pass through unchanged.
pub fn sanitize_url(url: &str) -> String {
    let stripped = strip_url_sensitive_parts(url);
    if contains_secrets(&stripped) {
        mask_secrets(&stripped)
    } else {
        stripped
    }
}

fn strip_url_sensitive_parts(url: &str) -> String {
    let without_fragment = url.split('#').next().unwrap_or(url);
    let without_query = without_fragment
        .split('?')
        .next()
        .unwrap_or(without_fragment);

    if let Some((scheme, rest)) = without_query.split_once("://") {
        let (authority, path) = match rest.find('/') {
            Some(idx) => (&rest[..idx], &rest[idx..]),
            None => (rest, ""),
        };
        let host = authority.rsplit('@').next().unwrap_or(authority);
        if host.is_empty() {
            return String::new();
        }
        format!("{scheme}://{host}{path}")
    } else if let Some(rest) = without_query.strip_prefix("//") {
        let (authority, path) = match rest.find('/') {
            Some(idx) => (&rest[..idx], &rest[idx..]),
            None => (rest, ""),
        };
        let host = authority.rsplit('@').next().unwrap_or(authority);
        if host.is_empty() {
            return String::new();
        }
        format!("//{host}{path}")
    } else {
        let (authority, path) = match without_query.find('/') {
            Some(idx) => (&without_query[..idx], &without_query[idx..]),
            None => (without_query, ""),
        };
        let host = authority.rsplit('@').next().unwrap_or(authority);
        format!("{host}{path}")
    }
}

fn url_in_text_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"[a-zA-Z][a-zA-Z0-9+.-]*://[^\s'"<>)\]]+"#).expect("url regex compiles")
    })
}

/// Sanitize any `scheme://...` URLs embedded in `text` that carry userinfo,
/// query parameters, or fragments, while leaving clean URLs untouched.
pub fn sanitize_urls_in_text(text: &str) -> String {
    if !text.contains("://") {
        return text.to_string();
    }
    url_in_text_regex()
        .replace_all(text, |caps: &regex::Captures<'_>| {
            let raw = &caps[0];
            let trimmed = raw.trim_end_matches(['.', ',', ';', ':']);
            let trailing = &raw[trimmed.len()..];
            let has_sensitive_url_parts = trimmed.contains('?')
                || trimmed.contains('#')
                || trimmed
                    .split_once("://")
                    .map(|(_, rest)| {
                        let authority = rest.split('/').next().unwrap_or(rest);
                        authority.contains('@')
                    })
                    .unwrap_or(false);
            if !has_sensitive_url_parts {
                return raw.to_string();
            }
            let cleaned = strip_url_sensitive_parts(trimmed);
            format!("{cleaned}{trailing}")
        })
        .into_owned()
}

fn command_assignment_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r#"(?:^|(?P<pre>[\s;|&(]))(?P<name>[A-Za-z_][A-Za-z0-9_]*)=(?:"(?:\\.|[^"\\])*"|'[^']*'|[^\s'";|&)]+)+"#,
        )
        .expect("command assignment regex compiles")
    })
}

/// Redact shell variable assignments (`VAR=value` -> `VAR=[REDACTED]`) in a
/// command or argument string while preserving variable names and surrounding
/// tokens. CLI flags (`--flag=value`) start with `-` and are left untouched
/// unless matched by [`mask_secrets`].
pub fn sanitize_command_assignments(text: &str) -> String {
    if !text.contains('=') {
        return text.to_string();
    }
    command_assignment_regex()
        .replace_all(text, "${pre}${name}=[REDACTED]")
        .into_owned()
}

/// Extract the executable/command name from a shell command string, skipping
/// leading environment variable assignments (`VAR=value`), command separators
/// (`;`, `&&`, `||`, `|`, `&`), and `env` wrappers. Returns `None` when the
/// command consists solely of variable assignments or when the remaining token
/// itself carries a secret or assignment.
pub fn extract_command_name(command: Option<&str>) -> Option<String> {
    let command = command?;
    let tokens = shell_tokens(command.trim());
    let mut idx = 0;

    while idx < tokens.len() {
        let tok = tokens[idx].as_str();
        if is_shell_operator(tok) || is_shell_assignment(tok) {
            idx += 1;
            continue;
        }
        if tok == "env" {
            let mut look = idx + 1;
            let mut saw_env_arg = false;
            while look < tokens.len() {
                let next = tokens[look].as_str();
                if next.starts_with('-') || is_shell_assignment(next) {
                    saw_env_arg = true;
                    look += 1;
                } else {
                    break;
                }
            }
            if (look < tokens.len() && !is_shell_operator(&tokens[look])) || saw_env_arg {
                idx = look;
                continue;
            }
        }
        break;
    }

    let candidate = tokens.get(idx)?.as_str();
    if candidate.is_empty()
        || is_shell_operator(candidate)
        || is_shell_assignment(candidate)
        || candidate.contains('=')
        || contains_secrets(candidate)
    {
        return None;
    }

    let cleaned = sanitize_urls_in_text(candidate);
    if cleaned.is_empty() || cleaned == "[REDACTED]" || contains_secrets(&cleaned) {
        return None;
    }
    Some(cleaned)
}

fn is_shell_operator(tok: &str) -> bool {
    matches!(tok, ";" | "|" | "||" | "&" | "&&")
}

fn is_shell_assignment(tok: &str) -> bool {
    let Some((name, _)) = tok.split_once('=') else {
        return false;
    };
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn shell_tokens(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();

    while let Some(&ch) = chars.peek() {
        if ch.is_whitespace() {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            chars.next();
            continue;
        }
        if matches!(ch, ';' | '|' | '&') {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            let op = chars.next().unwrap();
            let mut op_str = String::from(op);
            if matches!(op, '|' | '&') && chars.peek() == Some(&op) {
                op_str.push(chars.next().unwrap());
            }
            tokens.push(op_str);
            continue;
        }
        match ch {
            '\'' => {
                current.push(chars.next().unwrap());
                for c in chars.by_ref() {
                    current.push(c);
                    if c == '\'' {
                        break;
                    }
                }
            }
            '"' => {
                current.push(chars.next().unwrap());
                while let Some(c) = chars.next() {
                    current.push(c);
                    if c == '\\' {
                        if let Some(escaped) = chars.next() {
                            current.push(escaped);
                        }
                    } else if c == '"' {
                        break;
                    }
                }
            }
            '\\' => {
                current.push(chars.next().unwrap());
                if let Some(escaped) = chars.next() {
                    current.push(escaped);
                }
            }
            _ => {
                current.push(chars.next().unwrap());
            }
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
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
        // Do not interpolate `masked` into the assert message: CodeQL treats
        // that as rust/cleartext-logging (#11, #12) even after mask_secrets.
        assert!(
            masked.contains("[REDACTED]"),
            "secret assignment should be redacted"
        );
        assert!(
            !masked.contains("supersecretvalue123"),
            "plaintext secret must not remain"
        );
    }

    #[test]
    fn padded_basic_auth_is_fully_masked() {
        let masked = mask_secrets("Authorization: Basic abcdefghij==");
        assert_eq!(masked, "Authorization: [REDACTED]");
    }

    #[test]
    fn padded_bearer_is_fully_masked() {
        let masked = mask_secrets("Authorization: Bearer abcdefghij==");
        assert_eq!(masked, "Authorization: [REDACTED]");
    }

    #[test]
    fn unpadded_bearer_is_fully_masked() {
        let masked = mask_secrets("Authorization: Bearer abcdefghijklmnopqrstuvwxyz");
        assert_eq!(masked, "Authorization: [REDACTED]");
    }
}
