//! Byte-for-byte capture-mapping parity against the Node adapter.
//!
//! `tests/fixtures/capture-cases.json` is generated from the REAL Node adapter
//! (`packages/kindling-adapter-claude-code/src/claude-code/*.ts`) by
//! `tests/fixtures/generate-fixtures.mjs` (run via tsx). Each case carries the
//! stdin-shaped `hookInput`, the `hookType`, and the `expected`
//! `{ kind, content, provenance, scopeIds }` Node produced.
//!
//! This test runs the Rust [`map_capture`] over each `hookInput` and asserts the
//! resulting observation's kind/content/provenance/scopeIds equal Node's,
//! compared as `serde_json::Value` (object key order is therefore NOT asserted —
//! see the known-gaps note in the crate docs about provenance key ordering).

use kindling_hook::{map_capture, HookInput, HookType};
use serde_json::Value;

const FIXTURES: &str = include_str!("fixtures/capture-cases.json");

#[derive(serde::Deserialize)]
struct Case {
    name: String,
    #[serde(rename = "hookType")]
    hook_type: String,
    #[serde(rename = "hookInput")]
    hook_input: Value,
    expected: Value,
}

#[test]
fn capture_mapping_matches_node_byte_for_byte() {
    let cases: Vec<Case> = serde_json::from_str(FIXTURES).expect("parse capture-cases.json");
    assert!(!cases.is_empty(), "fixtures should not be empty");

    for case in &cases {
        let hook_type = HookType::parse(&case.hook_type)
            .unwrap_or_else(|_| panic!("[{}] bad hookType {}", case.name, case.hook_type));
        let input: HookInput = serde_json::from_value(case.hook_input.clone())
            .unwrap_or_else(|e| panic!("[{}] deserialize hookInput: {e}", case.name));

        let observation = map_capture(hook_type, &input)
            .unwrap_or_else(|| panic!("[{}] expected an observation, got None", case.name));

        // Project the Rust observation into the same shape the fixtures hold.
        let actual = serde_json::json!({
            "kind": observation.kind,
            "content": observation.content,
            "provenance": observation.provenance.unwrap_or_default(),
            "scopeIds": observation.scope_ids,
        });

        assert_eq!(
            actual, case.expected,
            "[{}] mapping mismatch\n  expected: {}\n  actual:   {}",
            case.name, case.expected, actual
        );
    }
}

#[test]
fn empty_user_prompt_maps_to_nothing() {
    let input = HookInput {
        session_id: Some("s1".to_string()),
        cwd: "/repo".to_string(),
        content: Some("   ".to_string()),
        ..HookInput::default()
    };
    assert!(map_capture(HookType::UserPromptSubmit, &input).is_none());
}

#[test]
fn failure_hook_defaults_missing_error() {
    // post-tool-use-failure with no tool_error/error → "Unknown error".
    let input = HookInput {
        session_id: Some("s1".to_string()),
        cwd: "/repo".to_string(),
        tool_name: Some("Bash".to_string()),
        tool_input: Some(serde_json::json!({ "command": "boom" })),
        ..HookInput::default()
    };
    let obs = map_capture(HookType::PostToolUseFailure, &input).expect("observation");
    assert!(
        obs.content.ends_with("Error: Unknown error"),
        "{}",
        obs.content
    );
    assert_eq!(obs.provenance.unwrap()["hasError"], serde_json::json!(true));
}

#[test]
fn lifecycle_hooks_map_to_none() {
    let input = HookInput {
        session_id: Some("s1".to_string()),
        cwd: "/repo".to_string(),
        ..HookInput::default()
    };
    for ht in [HookType::SessionStart, HookType::PreCompact, HookType::Stop] {
        assert!(
            map_capture(ht, &input).is_none(),
            "{ht:?} should map to None"
        );
    }
}
