//! Contract tests for `generic-event@1` digest framing and `GenericEventDetector` v1 (KINTEG-015a):
//! - Golden digest vectors from `schema/fixtures/generic-event-digest-v1.json`
//! - Detector fixture vectors from `schema/fixtures/generic-event-detector-v1.json`
//! - Line-for-line equality between fixture rule bodies and the spec block
//! - Compiled rule table equality with the fixture
//! - Observation masking parity (unchanged output and shared core bodies)

use std::path::PathBuf;

use kindling_service::filter::{
    detect_generic_event_secrets, mask_secrets_with_evidence, rules, scan_generic_event_bytes,
    scan_generic_event_key, GENERIC_EVENT_DETECTOR_VERSION, GENERIC_EVENT_V1_RULES,
    OBSERVATION_MASKING_RULES,
};
use kindling_service::generic_event::{
    compute_generic_event_digest, frame_generic_event_digest_input,
};
use kindling_types::{GenericEvent, GenericEventScope, ScopeIds};
use serde::Deserialize;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DigestFixtureFile {
    version: u32,
    domain_tag: String,
    vectors: Vec<DigestVector>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DigestVector {
    name: String,
    schema_name: String,
    schema_version: u32,
    scope: GenericEventScope,
    payload_encoding: String,
    #[serde(default)]
    payload: Option<String>,
    #[serde(default)]
    payload_length: Option<usize>,
    framed_length: usize,
    #[serde(default)]
    framed_hex: Option<String>,
    digest: String,
}

fn decode_hex(hex: &str) -> Vec<u8> {
    assert_eq!(hex.len() % 2, 0);
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

#[test]
fn digest_matches_golden_vectors() {
    let path = repo_root().join("schema/fixtures/generic-event-digest-v1.json");
    let raw = std::fs::read_to_string(&path).expect("read digest fixture");
    let fixture: DigestFixtureFile = serde_json::from_str(&raw).expect("parse digest fixture");
    assert_eq!(fixture.version, 1);
    assert_eq!(fixture.domain_tag, "kindling.generic-event.v1");
    assert_eq!(fixture.vectors.len(), 3);

    for vec in &fixture.vectors {
        let payload: Vec<u8> = match vec.payload_encoding.as_str() {
            "hex" => decode_hex(vec.payload.as_deref().unwrap()),
            "utf8" => vec.payload.as_deref().unwrap().as_bytes().to_vec(),
            "mod251" => {
                let len = vec.payload_length.unwrap();
                (0..len).map(|i| (i % 251) as u8).collect()
            }
            other => panic!("unknown payloadEncoding {other}"),
        };

        let framed = frame_generic_event_digest_input(
            &vec.schema_name,
            vec.schema_version,
            &vec.scope,
            &payload,
        );
        assert_eq!(
            framed.len(),
            vec.framed_length,
            "vector {} framed length mismatch",
            vec.name
        );
        if let Some(expected_hex) = &vec.framed_hex {
            assert_eq!(
                encode_hex(&framed),
                *expected_hex,
                "vector {} framed bytes mismatch",
                vec.name
            );
        }

        let digest = compute_generic_event_digest(
            &vec.schema_name,
            vec.schema_version,
            &vec.scope,
            &payload,
        );
        assert_eq!(digest, vec.digest, "vector {} digest mismatch", vec.name);
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DetectorFixtureFile {
    detector_version: u32,
    rules: Vec<DetectorFixtureRule>,
    payload_vectors: PayloadVectors,
    key_vectors: KeyVectors,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DetectorFixtureRule {
    name: String,
    pattern: String,
    applies_to_key: bool,
}

#[derive(Deserialize)]
struct PayloadVectors {
    clean: Vec<CleanPayloadVector>,
    rejected: Vec<RejectedPayloadVector>,
}

#[derive(Deserialize)]
struct CleanPayloadVector {
    name: String,
    input: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RejectedPayloadVector {
    name: String,
    input: String,
    expected_classes: Vec<String>,
    expected_count: u32,
}

#[derive(Deserialize)]
struct KeyVectors {
    clean: Vec<String>,
    rejected: Vec<RejectedKeyVector>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RejectedKeyVector {
    key: String,
    expected_classes: Vec<String>,
}

fn load_detector_fixture() -> DetectorFixtureFile {
    let path = repo_root().join("schema/fixtures/generic-event-detector-v1.json");
    let raw = std::fs::read_to_string(&path).expect("read detector fixture");
    serde_json::from_str(&raw).expect("parse detector fixture")
}

/// Extract `(name, pattern)` lines from the ```text block under
/// `### GenericEventDetector v1 rules` in the design spec.
fn parse_spec_detector_block() -> Vec<(String, String)> {
    let spec_path = repo_root().join("plans/specs/2026-11-18-generic-event-keyed-append-design.md");
    let spec = std::fs::read_to_string(&spec_path).expect("read design spec");

    let section_start = spec
        .find("### GenericEventDetector v1 rules")
        .expect("spec must contain GenericEventDetector v1 rules section");
    let after_heading = &spec[section_start..];
    let fence_start = after_heading
        .find("```text\n")
        .expect("spec must contain ```text fence")
        + "```text\n".len();
    let after_fence = &after_heading[fence_start..];
    let fence_end = after_fence
        .find("\n```")
        .expect("spec must close ```text fence");
    let block = &after_fence[..fence_end];

    block
        .lines()
        .map(|line| {
            // Strip trailing `# ...` comment (e.g. basicAuth note).
            let without_comment = match line.find("  # ") {
                Some(idx) => line[..idx].trim_end(),
                None => line.trim_end(),
            };
            let mut parts = without_comment.splitn(2, char::is_whitespace);
            let name = parts.next().expect("rule name").to_string();
            let pattern = parts.next().expect("rule pattern").trim_start().to_string();
            (name, pattern)
        })
        .collect()
}

#[test]
fn detector_fixture_equals_spec_block_and_compiled_table() {
    let fixture = load_detector_fixture();
    assert_eq!(fixture.detector_version, GENERIC_EVENT_DETECTOR_VERSION);

    let spec_rules = parse_spec_detector_block();
    assert_eq!(
        fixture.rules.len(),
        spec_rules.len(),
        "fixture rule count must match spec block"
    );

    for (idx, ((spec_name, spec_pat), fix_rule)) in
        spec_rules.iter().zip(fixture.rules.iter()).enumerate()
    {
        assert_eq!(
            &fix_rule.name, spec_name,
            "rule #{idx} name mismatch between fixture and spec"
        );
        assert_eq!(
            &fix_rule.pattern, spec_pat,
            "rule #{idx} ({spec_name}) pattern mismatch between fixture and spec"
        );
    }

    assert_eq!(
        GENERIC_EVENT_V1_RULES.len(),
        fixture.rules.len(),
        "compiled rule table length must match fixture"
    );
    for (compiled, fix_rule) in GENERIC_EVENT_V1_RULES.iter().zip(fixture.rules.iter()) {
        assert_eq!(compiled.name, fix_rule.name);
        assert_eq!(compiled.pattern, fix_rule.pattern);
        assert_eq!(compiled.applies_to_key, fix_rule.applies_to_key);
    }
}

#[test]
fn detector_matches_all_fixture_vectors_and_non_utf8_bytes() {
    let fixture = load_detector_fixture();

    for clean in &fixture.payload_vectors.clean {
        let hit = scan_generic_event_bytes("payload", clean.input.as_bytes());
        assert!(
            hit.is_none(),
            "clean payload vector {:?} should not match, got {hit:?}",
            clean.name
        );
    }

    for rej in &fixture.payload_vectors.rejected {
        let hit = scan_generic_event_bytes("payload", rej.input.as_bytes())
            .unwrap_or_else(|| panic!("rejected payload vector {:?} must match", rej.name));
        assert_eq!(hit.field, "payload");
        assert_eq!(hit.classes, rej.expected_classes, "vector {:?}", rej.name);
        assert_eq!(hit.count, rej.expected_count, "vector {:?}", rej.name);
        assert_eq!(hit.detector_version, GENERIC_EVENT_DETECTOR_VERSION);
    }

    for clean_key in &fixture.key_vectors.clean {
        let hit = scan_generic_event_key(clean_key);
        assert!(
            hit.is_none(),
            "clean key {clean_key:?} should not match, got {hit:?}"
        );
    }

    for rej_key in &fixture.key_vectors.rejected {
        let hit = scan_generic_event_key(&rej_key.key)
            .unwrap_or_else(|| panic!("rejected key {:?} must match", rej_key.key));
        assert_eq!(hit.field, "key");
        assert_eq!(hit.classes, rej_key.expected_classes);
        assert_eq!(hit.detector_version, GENERIC_EVENT_DETECTOR_VERSION);
    }

    // Non-UTF-8 bytes adjacent to a secret must still be scanned cleanly with (?-u).
    let mut non_utf8 = vec![0xFF, 0xFE, 0x80];
    non_utf8.extend_from_slice(b"AKIA0000000000000000");
    non_utf8.extend_from_slice(&[0xFF, 0x00]);
    let hit = scan_generic_event_bytes("payload", &non_utf8).expect("non-utf8 secret detected");
    assert_eq!(hit.classes, vec!["awsAccessKeyId"]);

    // Scope surfaces are screened by detect_generic_event_secrets, and payload bytes are never mutated.
    let event = GenericEvent {
        envelope_version: 1,
        schema_name: "example.audit".to_string(),
        schema_version: 1,
        key: "kfit/run-42/evt-7".to_string(),
        scope_ids: ScopeIds {
            session_id: Some("ghp_000000000000000000000000000000000000".to_string()),
            ..Default::default()
        },
        payload: b"clean".to_vec(),
    };
    let scope_hit = detect_generic_event_secrets(&event).expect("scope secret detected");
    assert_eq!(scope_hit.field, "scopeIds.sessionId");
    assert_eq!(scope_hit.classes, vec!["githubToken"]);
}

#[test]
fn observation_masking_shares_core_bodies_and_preserves_output() {
    // Shared core bodies appear in both profiles.
    let obs_patterns: Vec<&str> = OBSERVATION_MASKING_RULES.iter().map(|(p, _)| *p).collect();
    let ge_patterns: Vec<&str> = GENERIC_EVENT_V1_RULES.iter().map(|r| r.pattern).collect();

    for core in [
        rules::ANTHROPIC_KEY_CORE,
        rules::OPENAI_KEY_CORE,
        rules::GITHUB_TOKEN_CORE,
        rules::AWS_ACCESS_KEY_ID_CORE,
        rules::BEARER_TOKEN_CORE,
        rules::BASIC_AUTH_CORE,
    ] {
        assert!(
            obs_patterns.iter().any(|p| p.contains(core)),
            "observation masking rules must share core body {core}"
        );
        assert!(
            ge_patterns.iter().any(|p| p.contains(core)),
            "generic-event v1 rules must share core body {core}"
        );
    }

    // Observation masking output is unchanged.
    let (masked, ev) = mask_secrets_with_evidence("token=abcdefgh1234 and AKIA0000000000000000");
    assert_eq!(masked, "token=[REDACTED] and [REDACTED]");
    assert_eq!(ev.count, 2);
    assert_eq!(ev.classes, vec!["credentialAssignment", "awsAccessKeyId"]);
}
