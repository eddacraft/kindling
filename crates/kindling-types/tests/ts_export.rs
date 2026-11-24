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
    // `export_all` writes (or overwrites) one file per type into `cfg.out_dir()`.
    // ts-rs handles creating the directory if needed.
    let cfg = ts_rs::Config::new().with_out_dir(bindings_dir());
    let exports: Vec<Result<(), ts_rs::ExportError>> = vec![
        Capability::export_all(&cfg),
        CapabilityFeature::export_all(&cfg),
        KindRegistryEntry::export_all(&cfg),
        ScopeIds::export_all(&cfg),
        ValidationError::export_all(&cfg),
        Observation::export_all(&cfg),
        ObservationInput::export_all(&cfg),
        Capsule::export_all(&cfg),
        CapsuleInput::export_all(&cfg),
        Summary::export_all(&cfg),
        SummaryInput::export_all(&cfg),
        Pin::export_all(&cfg),
        PinInput::export_all(&cfg),
        RetrieveOptions::export_all(&cfg),
        QueryMode::export_all(&cfg),
        RetrieveResult::export_all(&cfg),
        RetrieveProvenance::export_all(&cfg),
        PinResult::export_all(&cfg),
        CandidateResult::export_all(&cfg),
        ProviderSearchOptions::export_all(&cfg),
        ProviderSearchResult::export_all(&cfg),
        GenericEvent::export_all(&cfg),
        GenericEventScope::export_all(&cfg),
        GenericEventReceipt::export_all(&cfg),
        GenericEventLookup::export_all(&cfg),
        GenericEventForget::export_all(&cfg),
        GenericEventLimits::export_all(&cfg),
        RejectReason::export_all(&cfg),
        ConflictKind::export_all(&cfg),
        CommitFailure::export_all(&cfg),
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
