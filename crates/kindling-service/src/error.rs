//! Service-layer error type.

use kindling_provider::ProviderError;
use kindling_store::StoreError;
use kindling_types::{Id, ValidationError};

/// Errors produced by [`KindlingService`](crate::KindlingService).
///
/// Mirrors the Result-type error surface of the TS service, which throws
/// `Error` for validation failures and lifecycle violations. Here those become
/// structured variants so callers can branch on them.
#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    /// Underlying store failure (sqlite, JSON, IO, or a store lifecycle error
    /// not otherwise specialised by the service).
    #[error(transparent)]
    Store(#[from] StoreError),

    /// Retrieval/provider failure.
    #[error(transparent)]
    Provider(#[from] ProviderError),

    /// JSON (de)serialization failure (export/import bundle handling).
    #[error("json (de)serialization error: {0}")]
    Json(#[from] serde_json::Error),

    /// Input validation failed. Carries every field error, matching the TS
    /// validators which collect all problems before returning.
    #[error("validation failed: {}", format_validation(.0))]
    Validation(Vec<ValidationError>),

    /// A session already has an open capsule (open lifecycle invariant).
    #[error("conflict: {0}")]
    Conflict(String),

    /// The referenced capsule does not exist.
    #[error("capsule {0} not found")]
    NotFound(Id),

    /// The capsule exists but is already closed.
    #[error("capsule {0} is already closed")]
    AlreadyClosed(Id),
}

/// Result alias for service operations.
pub type ServiceResult<T> = Result<T, ServiceError>;

fn format_validation(errors: &[ValidationError]) -> String {
    errors
        .iter()
        .map(|e| e.message.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}
