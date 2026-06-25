//! Canonical kindling domain types.
//!
//! This crate is the source of truth for the wire-format shapes shared between
//! the Rust implementation and TypeScript consumers. Each public type
//! round-trips through the JSON encoding produced by the existing TypeScript
//! definitions in `packages/kindling-core/src/types/`.
//!
//! Enable the `ts-rs` feature to derive the TypeScript projection — running
//! `cargo test -p kindling-types --features ts-rs` writes the corresponding
//! `.ts` files into `crates/kindling-types/bindings/`.

pub mod capability;
pub mod capsule;
pub mod common;
pub mod list;
pub mod observation;
pub mod pin;
pub mod retrieval;
pub mod summary;

pub use capability::{
    build_capability, kind_registry, supported_kind_names, Capability, KindRegistryEntry,
    OBSERVATION_REQUIRED_FIELDS,
};
pub use capsule::{Capsule, CapsuleInput, CapsuleStatus, CapsuleType};
pub use common::{Id, ScopeIds, Timestamp, ValidationError};
pub use list::{ListObservationsRequest, ListObservationsResult};
pub use observation::{Observation, ObservationInput, ObservationKind};
pub use pin::{is_pin_active, Pin, PinInput, PinTargetType};
pub use retrieval::{
    CandidateResult, PinResult, ProviderSearchOptions, ProviderSearchResult, RetrieveOptions,
    RetrieveProvenance, RetrieveResult, RetrievedEntity,
};
pub use summary::{is_valid_confidence, Summary, SummaryInput};
