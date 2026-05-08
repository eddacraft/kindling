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
    ];

    for (i, r) in exports.into_iter().enumerate() {
        if let Err(e) = r {
            panic!("ts-rs export #{i} failed: {e}");
        }
    }

    // Spot-check that the directory now contains the expected files.
    for f in [
        "Observation.ts",
        "Capsule.ts",
        "Summary.ts",
        "Pin.ts",
        "RetrieveResult.ts",
        "ScopeIds.ts",
    ] {
        let path = bindings_dir().join(f);
        assert!(path.exists(), "expected {} after export", path.display());
    }
}
