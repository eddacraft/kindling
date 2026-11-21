//! Canonical secret-detection rule table serving two profiles (KINTEG-015a):
//!
//! 1. **Observation masking** (`OBSERVATION_MASKING_RULES`): preserves the 7
//!    rules used by `mask_secrets` / `mask_secrets_with_evidence` unchanged.
//! 2. **`GenericEventDetector` v1** (`GENERIC_EVENT_V1_RULES`): read-only byte
//!    scanner compiled with Unicode off (`(?-u)`), rejecting matches without
//!    ever mutating payload bytes or echoing matched text.

use std::sync::OnceLock;

use kindling_types::GenericEvent;
use regex::bytes::{Regex as BytesRegex, RegexBuilder as BytesRegexBuilder};

/// Current `GenericEventDetector` profile version advertised in capability limits.
pub const GENERIC_EVENT_DETECTOR_VERSION: u32 = 1;

/// Shared core regex bodies pinned by parity tests across both profiles.
pub const ANTHROPIC_KEY_CORE: &str = r"sk-ant-[A-Za-z0-9\-_]{20,}";
pub const OPENAI_KEY_CORE: &str = r"sk-[A-Za-z0-9]{40,}";
pub const GITHUB_TOKEN_CORE: &str = r"gh[pousr]_[A-Za-z0-9]{36,}";
pub const AWS_ACCESS_KEY_ID_CORE: &str = r"AKIA[0-9A-Z]{16}";
pub const BEARER_TOKEN_CORE: &str = r"Bearer\s+[A-Za-z0-9\-._~+/]{20,}";
pub const BASIC_AUTH_CORE: &str = r"Basic\s+[A-Za-z0-9+/]{20,}";

/// One rule in the observation-masking profile (`(pattern, class)`).
///
/// Order and regexes are unchanged from the Node.js filter so masked output
/// stays byte-for-byte identical.
pub const OBSERVATION_MASKING_RULES: &[(&str, &str)] = &[
    (
        r#"(?i)['"]?(?:[A-Za-z0-9_-]*(?:api[-_]?key|apikey|token|secret|password|passwd|pwd)[A-Za-z0-9_-]*|(?:private|access|auth|client|signing|encryption|ssh|master)[-_]?key)['"]?\s*[:=]\s*['"]?[^\s'"]{8,}['"]?"#,
        "credentialAssignment",
    ),
    (ANTHROPIC_KEY_CORE, "anthropicKey"),
    (OPENAI_KEY_CORE, "openaiKey"),
    (r"(?i)Bearer\s+[A-Za-z0-9\-._~+/]{20,}", "bearerToken"),
    (r"(?i)Basic\s+[A-Za-z0-9+/]{20,}", "basicAuth"),
    (GITHUB_TOKEN_CORE, "githubToken"),
    (AWS_ACCESS_KEY_ID_CORE, "awsAccessKeyId"),
];

/// Declarative specification of one `GenericEventDetector` v1 rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetectorRuleSpec {
    pub name: &'static str,
    pub pattern: &'static str,
    /// Whether this rule also screens generic-event `key` values.
    pub applies_to_key: bool,
}

/// Exact `GenericEventDetector` v1 rule table (spec §GenericEventDetector v1 rules).
pub const GENERIC_EVENT_V1_RULES: &[DetectorRuleSpec] = &[
    DetectorRuleSpec {
        name: "anthropicKey",
        pattern: r"(?:^|[^A-Za-z0-9_-])sk-ant-[A-Za-z0-9\-_]{20,}",
        applies_to_key: true,
    },
    DetectorRuleSpec {
        name: "openaiKey",
        pattern: r"(?:^|[^A-Za-z0-9_-])sk-[A-Za-z0-9]{40,}",
        applies_to_key: true,
    },
    DetectorRuleSpec {
        name: "githubToken",
        pattern: r"(?:^|[^A-Za-z0-9_-])gh[pousr]_[A-Za-z0-9]{36,}",
        applies_to_key: true,
    },
    DetectorRuleSpec {
        name: "awsAccessKeyId",
        pattern: r"(?:^|[^A-Za-z0-9_-])AKIA[0-9A-Z]{16}",
        applies_to_key: true,
    },
    DetectorRuleSpec {
        name: "bearerToken",
        pattern: r"Bearer\s+[A-Za-z0-9\-._~+/]{20,}",
        applies_to_key: false,
    },
    DetectorRuleSpec {
        name: "basicAuth",
        pattern: r"Basic\s+[A-Za-z0-9+/]{20,}={0,2}(?:[^A-Za-z0-9+/=]|$)",
        applies_to_key: false,
    },
    DetectorRuleSpec {
        name: "privateKeyBlock",
        pattern: r"-----BEGIN [A-Z ]*PRIVATE KEY-----",
        applies_to_key: false,
    },
    DetectorRuleSpec {
        name: "credentialField",
        pattern: r#"(?i)(?:^|[^A-Za-z0-9_-])(?:password|passwd|secret|token|api[-_]?key|access[-_]?token|refresh[-_]?token|client[-_]?secret|secret[-_]?key|private[-_]?key)["']?\s*[:=]\s*["']?[^\s"']{8,}"#,
        applies_to_key: false,
    },
];

/// Structured result of a `GenericEventDetector` match. Never contains matched
/// substrings or byte offsets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretDetectionHit {
    pub field: String,
    pub classes: Vec<String>,
    pub count: u32,
    pub detector_version: u32,
}

struct CompiledDetectorRule {
    spec: DetectorRuleSpec,
    regex: BytesRegex,
}

fn compiled_v1_rules() -> &'static [CompiledDetectorRule] {
    static RULES: OnceLock<Vec<CompiledDetectorRule>> = OnceLock::new();
    RULES.get_or_init(|| {
        GENERIC_EVENT_V1_RULES
            .iter()
            .map(|&spec| {
                let regex = BytesRegexBuilder::new(spec.pattern)
                    .unicode(false)
                    .build()
                    .unwrap_or_else(|e| panic!("rule {} failed to compile: {e}", spec.name));
                CompiledDetectorRule { spec, regex }
            })
            .collect()
    })
}

fn scan_with_filter(field: &str, bytes: &[u8], key_only: bool) -> Option<SecretDetectionHit> {
    let mut classes = Vec::new();
    let mut count: u32 = 0;

    for rule in compiled_v1_rules() {
        if key_only && !rule.spec.applies_to_key {
            continue;
        }
        let mut rule_hits: u32 = 0;
        for m in rule.regex.find_iter(bytes) {
            if rule.spec.name == "basicAuth" && m.as_bytes().windows(2).any(|w| w == b"//") {
                continue;
            }
            rule_hits += 1;
        }
        if rule_hits > 0 {
            classes.push(rule.spec.name.to_string());
            count += rule_hits;
        }
    }

    if count == 0 {
        None
    } else {
        Some(SecretDetectionHit {
            field: field.to_string(),
            classes,
            count,
            detector_version: GENERIC_EVENT_DETECTOR_VERSION,
        })
    }
}

/// Screen raw bytes (payload or scope value) with all `GenericEventDetector` v1 rules.
pub fn scan_generic_event_bytes(field: &str, bytes: &[u8]) -> Option<SecretDetectionHit> {
    scan_with_filter(field, bytes, false)
}

/// Screen a generic-event `key` with the four key-surface rules (`anthropicKey`,
/// `openaiKey`, `githubToken`, `awsAccessKeyId`).
pub fn scan_generic_event_key(key: &str) -> Option<SecretDetectionHit> {
    scan_with_filter("key", key.as_bytes(), true)
}

/// Screen every secret-checked surface of a [`GenericEvent`]:
/// `key`, `scopeIds.*`, and raw `payload` bytes (`schemaName` is grammar-only).
///
/// Read-only: never mutates the event or echoes matched text.
pub fn detect_generic_event_secrets(event: &GenericEvent) -> Option<SecretDetectionHit> {
    if let Some(hit) = scan_generic_event_key(&event.key) {
        return Some(hit);
    }
    for (field, value) in [
        ("scopeIds.sessionId", event.scope_ids.session_id.as_deref()),
        ("scopeIds.repoId", event.scope_ids.repo_id.as_deref()),
        ("scopeIds.agentId", event.scope_ids.agent_id.as_deref()),
        ("scopeIds.userId", event.scope_ids.user_id.as_deref()),
        ("scopeIds.taskId", event.scope_ids.task_id.as_deref()),
    ] {
        if let Some(v) = value {
            if let Some(hit) = scan_generic_event_bytes(field, v.as_bytes()) {
                return Some(hit);
            }
        }
    }
    scan_generic_event_bytes("payload", &event.payload)
}
