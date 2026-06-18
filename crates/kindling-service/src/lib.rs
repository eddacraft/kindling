//! In-process Kindling orchestration.
//!
//! [`KindlingService`] exposes capsule lifecycle, observation append,
//! retrieval, and pin management over the already-ported store, provider, and
//! filter crates. Consumed in-process by the daemon (`kindling-server`), by the
//! CLI, and directly by Rust integrators (e.g. Anvil headless flows).
//!
//! Ports `KindlingService` from
//! `packages/kindling-core/src/service/kindling-service.ts`. Two deliberate
//! deviations from the TS service:
//!
//! 1. **Result-typed errors.** Every method returns [`ServiceResult`] instead
//!    of throwing; validation failures and lifecycle violations are structured
//!    [`ServiceError`] variants.
//! 2. **Service-boundary secret masking.** [`KindlingService::append_observation`]
//!    runs observation content through `kindling_filter::mask_secrets` before
//!    storing, so non-Rust consumers cannot bypass secret filtering. (Masking
//!    only — content truncation stays a hook-layer concern.)
//!
//! Export/import/bundle methods from the TS service are intentionally absent;
//! they belong to the CLI task (PORT-012).

mod context;
mod error;
mod service;
mod validation;

pub use context::{PreCompactContext, ResolvedPin, SessionStartContext};
pub use error::{ServiceError, ServiceResult};
pub use service::{
    AppendObservationOptions, CloseCapsuleOptions, CreatePinOptions, KindlingService,
    OpenCapsuleOptions,
};
