//! Kind registry tests — every ObservationKind variant must appear with required fields.

use kindling_types::{kind_registry, supported_kind_names, ObservationKind};

const EXPECTED_KINDS: &[&str] = &[
    "tool_call",
    "command",
    "file_diff",
    "error",
    "message",
    "node_start",
    "node_end",
    "node_output",
    "node_error",
];

#[test]
fn supported_kind_names_lists_all_nine_variants_in_order() {
    let names = supported_kind_names();
    assert_eq!(names.len(), 9);
    assert_eq!(names, EXPECTED_KINDS);
    for (kind, name) in ObservationKind::ALL.iter().zip(names.iter()) {
        assert_eq!(kind.wire_name(), name.as_str());
    }
}

#[test]
fn kind_registry_covers_every_variant_with_required_fields() {
    let registry = kind_registry();
    assert_eq!(registry.len(), 9);

    for (entry, expected_name) in registry.iter().zip(EXPECTED_KINDS.iter()) {
        assert_eq!(&entry.kind, expected_name);
        assert!(
            !entry.required_fields.is_empty(),
            "kind {expected_name} must list required fields"
        );
        for base in kindling_types::OBSERVATION_REQUIRED_FIELDS {
            assert!(
                entry.required_fields.iter().any(|f| f == base),
                "kind {expected_name} missing base field {base}"
            );
        }
    }

    let kinds: Vec<&str> = registry.iter().map(|e| e.kind.as_str()).collect();
    assert_eq!(kinds, EXPECTED_KINDS);
}

#[test]
fn build_capability_round_trips_through_json() {
    let cap = kindling_types::build_capability("0.2.0", 1, "/tmp/kindling.db");
    let json = serde_json::to_value(&cap).expect("serialize capability");
    assert_eq!(json["version"], "0.2.0");
    assert_eq!(json["schemaVersion"], 1);
    assert_eq!(json["storagePath"], "/tmp/kindling.db");
    let kinds = json["supportedKinds"]
        .as_array()
        .expect("supportedKinds array");
    assert_eq!(kinds.len(), 9);
    let registry = json["kindRegistry"].as_array().expect("kindRegistry array");
    assert_eq!(registry.len(), 9);

    let parsed: kindling_types::Capability =
        serde_json::from_value(json).expect("deserialize capability");
    assert_eq!(parsed, cap);
}
