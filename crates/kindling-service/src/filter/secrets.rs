//! Secret detection and masking.
//!
//! Ports `SECRET_PATTERNS`, `containsSecrets`, and `maskSecrets` from
//! `plugins/kindling-claude-code/hooks/lib/filter.js`. Patterns are applied
//! in the same order with the same replacement semantics: a match containing
//! `:` or `=` is replaced by everything before the first separator plus
//! `=[REDACTED]`; any other match is replaced wholesale.

use std::sync::OnceLock;

use kindling_types::{Observation, ObservationKind, RedactionEvidence, ScopeIds};
use regex::Regex;
use serde_json::{Map, Value};

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
                r#"(?i)['"]?(?:[A-Za-z0-9_-]*(?:api[-_]?key|apikey|token|secret|password|passwd|pwd)[A-Za-z0-9_-]*|(?:private|access|auth|client|signing|encryption|ssh|master)[-_]?key)['"]?\s*[:=]\s*['"]?[^\s'"]{8,}['"]?"#,
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
            // GitHub tokens (PAT, OAuth, app, refresh)
            (r"gh[pousr]_[A-Za-z0-9]{36,}", "githubToken"),
            // AWS access key IDs
            (r"AKIA[0-9A-Z]{16}", "awsAccessKeyId"),
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
///
/// This is a cleartext-logging sanitizer: matching secrets become
/// `[REDACTED]`. CodeQL is taught that via
/// `.github/codeql/extensions/kindling-rust`.
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
///
/// The returned content string is a cleartext-logging sanitizer (same
/// replacement as [`mask_secrets`]). CodeQL is taught that via
/// `.github/codeql/extensions/kindling-rust`.
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
                    Some(sep) if pattern.class != "bearerToken" && pattern.class != "basicAuth" => {
                        format!("{}=[REDACTED]", &matched[..sep])
                    }
                    _ => "[REDACTED]".to_string(),
                }
            })
            .into_owned();
        if matched_this_pattern {
            classes.push(pattern.class.to_string());
        }
    }
    (masked, RedactionEvidence { count, classes })
}

/// Sanitize all persisted fields of an [`Observation`] (`content`,
/// `provenance`, and `scope_ids`) at the service ingestion boundary, returning
/// the combined [`RedactionEvidence`] produced by secret masking across those
/// fields.
pub fn sanitize_observation_with_evidence(observation: &mut Observation) -> RedactionEvidence {
    let sanitize_cmd = observation.kind == ObservationKind::Command
        || observation.content.starts_with("$ ")
        || observation.content.contains("\n\n$ ");

    let (_, mut redaction) = mask_secrets_with_evidence(&observation.content);
    let url_clean = sanitize_urls_in_text(&observation.content);
    let cmd_clean = if sanitize_cmd {
        sanitize_command_assignments(&url_clean)
    } else {
        url_clean
    };
    observation.content = mask_secrets(&cmd_clean);

    observation.provenance = sanitize_provenance_map(&observation.provenance, &mut redaction);
    observation.scope_ids = sanitize_scope_ids(&observation.scope_ids, &mut redaction);

    redaction
}

fn merge_evidence(target: &mut RedactionEvidence, other: RedactionEvidence) {
    target.count += other.count;
    for class in other.classes {
        if !target.classes.contains(&class) {
            target.classes.push(class);
        }
    }
}

fn sanitize_provenance_map(
    map: &Map<String, Value>,
    redaction: &mut RedactionEvidence,
) -> Map<String, Value> {
    let mut out = Map::with_capacity(map.len());
    for (key, value) in map {
        let (_, key_ev) = mask_secrets_with_evidence(key);
        merge_evidence(redaction, key_ev);
        let clean_key = mask_secrets(&sanitize_command_assignments(&sanitize_urls_in_text(key)));

        if key == "command" || key == "cmd" {
            match value {
                Value::String(s) => {
                    let (_, ev) = mask_secrets_with_evidence(s);
                    merge_evidence(redaction, ev);
                    if let Some(cmd_name) = extract_command_name(Some(s)) {
                        out.insert(clean_key, Value::String(mask_secrets(&cmd_name)));
                    }
                }
                other => {
                    out.insert(clean_key, sanitize_provenance_value(other, redaction));
                }
            }
            continue;
        }

        if key == "url" {
            match value {
                Value::String(s) => {
                    let (_, ev) = mask_secrets_with_evidence(s);
                    merge_evidence(redaction, ev);
                    let cleaned = sanitize_url(s);
                    if !cleaned.is_empty() {
                        out.insert(clean_key, Value::String(cleaned));
                    }
                }
                other => {
                    out.insert(clean_key, sanitize_provenance_value(other, redaction));
                }
            }
            continue;
        }

        if is_sensitive_provenance_key(key) {
            if let Value::String(s) = value {
                let (_, ev) = mask_secrets_with_evidence(&format!("token={s}"));
                merge_evidence(redaction, ev);
            }
            out.insert(clean_key, Value::String("[REDACTED]".to_string()));
            continue;
        }

        out.insert(clean_key, sanitize_provenance_value(value, redaction));
    }
    out
}

fn sanitize_provenance_value(value: &Value, redaction: &mut RedactionEvidence) -> Value {
    match value {
        Value::String(s) => Value::String(sanitize_string_with_evidence(s, redaction)),
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| sanitize_provenance_value(item, redaction))
                .collect(),
        ),
        Value::Object(nested) => Value::Object(sanitize_provenance_map(nested, redaction)),
        Value::Null | Value::Bool(_) | Value::Number(_) => value.clone(),
    }
}

fn sanitize_scope_ids(scope: &ScopeIds, redaction: &mut RedactionEvidence) -> ScopeIds {
    ScopeIds {
        session_id: scope
            .session_id
            .as_deref()
            .map(|s| sanitize_string_with_evidence(s, redaction)),
        repo_id: scope
            .repo_id
            .as_deref()
            .map(|s| sanitize_string_with_evidence(s, redaction)),
        agent_id: scope
            .agent_id
            .as_deref()
            .map(|s| sanitize_string_with_evidence(s, redaction)),
        user_id: scope
            .user_id
            .as_deref()
            .map(|s| sanitize_string_with_evidence(s, redaction)),
        task_id: scope
            .task_id
            .as_deref()
            .map(|s| sanitize_string_with_evidence(s, redaction)),
    }
}

fn sanitize_string_with_evidence(s: &str, redaction: &mut RedactionEvidence) -> String {
    let (_, ev) = mask_secrets_with_evidence(s);
    merge_evidence(redaction, ev);
    let url_clean = sanitize_urls_in_text(s);
    let assigned = sanitize_command_assignments(&url_clean);
    mask_secrets(&assigned)
}

fn is_sensitive_provenance_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    if lower == "inputkeys" {
        return false;
    }
    matches!(
        lower.as_str(),
        "api_key"
            | "apikey"
            | "api-key"
            | "token"
            | "access_token"
            | "refresh_token"
            | "auth_token"
            | "secret"
            | "client_secret"
            | "api_secret"
            | "secret_key"
            | "private_key"
            | "access_key"
            | "password"
            | "passwd"
            | "pwd"
            | "authorization"
            | "auth"
            | "credential"
            | "credentials"
    ) || lower.contains("password")
        || lower.contains("passwd")
        || lower.contains("secret")
        || lower.contains("token")
        || lower.contains("apikey")
        || lower.contains("api_key")
        || lower.contains("api-key")
        || lower.contains("credential")
}

/// Strip userinfo (`user:pass@`), query strings (`?...`), and fragments
/// (`#...`) from a URL string, then mask any secrets remaining in the host or
/// path.
pub fn sanitize_url(url: &str) -> String {
    let stripped = strip_url_sensitive_parts(url);
    mask_secrets(&stripped)
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

fn sanitize_urls_in_text(text: &str) -> String {
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

fn sanitize_command_assignments(text: &str) -> String {
    if !text.contains('=') {
        return text.to_string();
    }
    command_assignment_regex()
        .replace_all(text, "${pre}${name}=[REDACTED]")
        .into_owned()
}

fn extract_command_name(command: Option<&str>) -> Option<String> {
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
