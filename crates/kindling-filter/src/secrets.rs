//! Secret detection and masking.
//!
//! Ports `SECRET_PATTERNS`, `containsSecrets`, and `maskSecrets` from
//! `plugins/kindling-claude-code/hooks/lib/filter.js`. Patterns are applied
//! in the same order with the same replacement semantics: a match containing
//! `:` or `=` is replaced by everything before the first separator plus
//! `=[REDACTED]`; any other match is replaced wholesale.

use std::sync::OnceLock;

use regex::Regex;

/// The secret patterns from the Node.js filter, in application order.
fn secret_patterns() -> &'static [Regex] {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        [
            // key/value assignments: api_key, token, secret, password, ...
            r#"(?i)['"]?(?:api[-_]?key|apikey|token|secret|password|passwd|pwd)['"]?\s*[:=]\s*['"]?[^\s'"]{8,}['"]?"#,
            // Anthropic keys
            r"sk-ant-[A-Za-z0-9\-_]{20,}",
            // OpenAI keys
            r"sk-[A-Za-z0-9]{40,}",
            // Bearer tokens
            r"(?i)Bearer\s+[A-Za-z0-9\-._~+/]{20,}",
            // Basic auth
            r"(?i)Basic\s+[A-Za-z0-9+/]{20,}",
        ]
        .iter()
        .map(|pattern| Regex::new(pattern).expect("secret pattern compiles"))
        .collect()
    })
}

/// True if the content matches any known secret pattern.
pub fn contains_secrets(content: &str) -> bool {
    secret_patterns()
        .iter()
        .any(|pattern| pattern.is_match(content))
}

/// Mask secrets in content, mirroring the Node.js replacement callback.
pub fn mask_secrets(content: &str) -> String {
    let mut masked = content.to_string();
    for pattern in secret_patterns() {
        masked = pattern
            .replace_all(&masked, |caps: &regex::Captures<'_>| {
                let matched = &caps[0];
                match matched.find([':', '=']) {
                    Some(sep) => format!("{}=[REDACTED]", &matched[..sep]),
                    None => "[REDACTED]".to_string(),
                }
            })
            .into_owned();
    }
    masked
}
