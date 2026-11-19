//! Drives `ts_rs` export so `cargo test -p kindling-types --features ts-rs`
//! writes the TypeScript projection of every public type into
//! `crates/kindling-types/bindings/`.
//!
//! Without the `ts-rs` feature the test compiles to nothing.

#![cfg(feature = "ts-rs")]

use kindling_types::*;
use std::path::PathBuf;
use ts_rs::TS;

fn bindings_dir() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("bindings");
    p
}

#[test]
fn export_all_types_to_bindings_dir() {
    // `export_all_to` writes (or overwrites) one file per type. ts-rs handles
    // creating the directory if needed.
    let dir = bindings_dir();
    let exports: Vec<Result<(), ts_rs::ExportError>> = vec![
        Capability::export_all_to(&dir),
        CapabilityFeature::export_all_to(&dir),
        KindRegistryEntry::export_all_to(&dir),
        ScopeIds::export_all_to(&dir),
        ValidationError::export_all_to(&dir),
        Observation::export_all_to(&dir),
        ObservationInput::export_all_to(&dir),
        Capsule::export_all_to(&dir),
        CapsuleInput::export_all_to(&dir),
        Summary::export_all_to(&dir),
        SummaryInput::export_all_to(&dir),
        Pin::export_all_to(&dir),
        PinInput::export_all_to(&dir),
        RetrieveOptions::export_all_to(&dir),
        RetrieveResult::export_all_to(&dir),
        RetrieveProvenance::export_all_to(&dir),
        PinResult::export_all_to(&dir),
        CandidateResult::export_all_to(&dir),
        ProviderSearchOptions::export_all_to(&dir),
        ProviderSearchResult::export_all_to(&dir),
        GenericEvent::export_all_to(&dir),
        GenericEventScope::export_all_to(&dir),
        GenericEventReceipt::export_all_to(&dir),
        GenericEventLookup::export_all_to(&dir),
        GenericEventForget::export_all_to(&dir),
        GenericEventLimits::export_all_to(&dir),
        RejectReason::export_all_to(&dir),
        ConflictKind::export_all_to(&dir),
        CommitFailure::export_all_to(&dir),
    ];

    for (i, r) in exports.into_iter().enumerate() {
        if let Err(e) = r {
            panic!("ts-rs export #{i} failed: {e}");
        }
    }

    // Spot-check that the directory now contains the expected files.
    for f in [
        "Capability.ts",
        "CapabilityFeature.ts",
        "KindRegistryEntry.ts",
        "Observation.ts",
        "Capsule.ts",
        "Summary.ts",
        "Pin.ts",
        "RetrieveResult.ts",
        "ScopeIds.ts",
        "GenericEvent.ts",
        "GenericEventScope.ts",
        "GenericEventReceipt.ts",
        "GenericEventLookup.ts",
        "GenericEventForget.ts",
        "GenericEventLimits.ts",
        "RejectReason.ts",
        "ConflictKind.ts",
        "CommitFailure.ts",
    ] {
        let path = bindings_dir().join(f);
        assert!(path.exists(), "expected {} after export", path.display());
    }

    // Spec (KINTEG-015a): ts-rs types `payload` as string and every i64/u64 as number (never bigint).
    for (file, must_contain) in [
        ("GenericEvent.ts", "payload: string"),
        ("GenericEventReceipt.ts", "committedAt: number"),
        ("GenericEventLookup.ts", "payload: string"),
        ("GenericEventLookup.ts", "redactedAt: number"),
        ("GenericEventForget.ts", "redactedAt: number"),
        ("GenericEventLimits.ts", "maxPayloadBytes: number"),
        ("GenericEventLimits.ts", "projectQuotaBytes?: number"),
        ("RejectReason.ts", "limit: number"),
        ("RejectReason.ts", "actual: number"),
        ("ConflictKind.ts", "redactedAt: number"),
    ] {
        let contents = std::fs::read_to_string(bindings_dir().join(file))
            .unwrap_or_else(|e| panic!("failed to read {file}: {e}"));
        assert!(
            contents.contains(must_contain),
            "{file} must contain `{must_contain}`, got:\n{contents}"
        );
        assert!(
            !contents.contains("bigint"),
            "{file} must not emit bigint:\n{contents}"
        );
    }
}
