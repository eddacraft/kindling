//! Secret detection and masking.
//!
//! Ports `SECRET_PATTERNS`, `containsSecrets`, and `maskSecrets` from
//! `plugins/kindling-claude-code/hooks/lib/filter.js`. Patterns are applied
//! in the same order with the same replacement semantics: a match containing
//! `:` or `=` is replaced by everything before the first separator plus
//! `=[REDACTED]`; any other match is replaced wholesale.

use std::sync::OnceLock;

use kindling_types::RedactionEvidence;
use regex::Regex;

/// A secret pattern plus the stable, machine-readable class it detects. The
/// class names are part of the redaction-evidence contract (KINTEG-006) and
/// must stay stable; they never carry matched substrings.
struct SecretPattern {
    /// Compiled detection regex.
    regex: Regex,
    /// Stable class identifier surfaced in [`RedactionEvidence::classes`].
    class: &'static str,
}

/// The secret patterns from the Node.js filter, in application order. The
/// regexes and their order are unchanged from the Node filter (so masked output
/// stays byte-for-byte identical); each is tagged with its evidence class.
fn secret_patterns() -> &'static [SecretPattern] {
    static PATTERNS: OnceLock<Vec<SecretPattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        [
            // key/value assignments: api_key, token, secret, password, ...
            (
                r#"(?i)['"]?(?:api[-_]?key|apikey|token|secret|password|passwd|pwd)['"]?\s*[:=]\s*['"]?[^\s'"]{8,}['"]?"#,
                "credentialAssignment",
            ),
            // Anthropic keys
            (r"sk-ant-[A-Za-z0-9\-_]{20,}", "anthropicKey"),
            // OpenAI keys
            (r"sk-[A-Za-z0-9]{40,}", "openaiKey"),
            // Bearer tokens
            (r"(?i)Bearer\s+[A-Za-z0-9\-._~+/]{20,}", "bearerToken"),
            // Basic auth
            (r"(?i)Basic\s+[A-Za-z0-9+/]{20,}", "basicAuth"),
        ]
        .iter()
        .map(|(pattern, class)| SecretPattern {
            regex: Regex::new(pattern).expect("secret pattern compiles"),
            class,
        })
        .collect()
    })
}

/// True if the content matches any known secret pattern.
pub fn contains_secrets(content: &str) -> bool {
    secret_patterns()
        .iter()
        .any(|pattern| pattern.regex.is_match(content))
}

/// Mask secrets in content, mirroring the Node.js replacement callback.
///
/// Output is byte-for-byte identical to the Node filter (pinned by
/// `tests/node_fixtures.rs`); evidence is discarded. Prefer
/// [`mask_secrets_with_evidence`] when the caller needs redaction evidence.
pub fn mask_secrets(content: &str) -> String {
    mask_secrets_with_evidence(content).0
}

/// Mask secrets and report redaction evidence derived from the *same* pass.
///
/// Returns the masked content plus a [`RedactionEvidence`] carrying the number
/// of masked matches and the distinct classes that matched (in detection
/// order). Because the evidence is produced by the masking pass itself, it
/// cannot be bypassed or drift from what was masked. The masked string is
/// identical to [`mask_secrets`].
pub fn mask_secrets_with_evidence(content: &str) -> (String, RedactionEvidence) {
    let mut masked = content.to_string();
    let mut count: u32 = 0;
    let mut classes: Vec<String> = Vec::new();
    for pattern in secret_patterns() {
        let mut matched_this_pattern = false;
        masked = pattern
            .regex
            .replace_all(&masked, |caps: &regex::Captures<'_>| {
                count += 1;
                matched_this_pattern = true;
                let matched = &caps[0];
                match matched.find([':', '=']) {
                    Some(sep) => format!("{}=[REDACTED]", &matched[..sep]),
                    None => "[REDACTED]".to_string(),
                }
            })
            .into_owned();
        if matched_this_pattern {
            classes.push(pattern.class.to_string());
        }
    }
    (masked, RedactionEvidence { count, classes })
}
