//! Wire-facing request bodies for the v1 API.
//!
//! These mirror the camelCase shapes in `kindling-server`'s `dto.rs` exactly.
//! Response bodies are the domain types from `kindling-types` (already
//! camelCase) and are deserialized directly — no response DTOs needed.

use kindling_types::{
    CapsuleType, Id, Observation, ObservationInput, PinTargetType, RedactionEvidence, ScopeIds,
};
use serde::{Deserialize, Serialize};

/// `POST /v1/capsules` body — open a capsule.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OpenCapsuleBody {
    pub kind: CapsuleType,
    pub intent: String,
    pub scope_ids: ScopeIds,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Id>,
}

/// `PATCH /v1/capsules/:id/close` body — all fields optional.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloseCapsuleBody {
    /// Ask the daemon to generate a summary on close.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generate_summary: Option<bool>,
    /// Provide summary content directly.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary_content: Option<String>,
    /// Confidence for a provided summary, in `[0.0, 1.0]`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
}

/// `POST /v1/observations` body — `ObservationInput` flattened plus the
/// top-level routing/append options (`capsuleId`, `validate`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppendObservationBody {
    #[serde(flatten)]
    pub input: ObservationInput,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capsule_id: Option<Id>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validate: Option<bool>,
}

/// `POST /v1/observations` response — the stored observation (flattened) plus
/// the daemon's `deduplicated` marker and redaction evidence. Mirrors
/// `AppendObservationResponse` in `kindling-server`'s `dto.rs`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppendObservationResponseBody {
    #[serde(flatten)]
    pub observation: Observation,
    /// Defaults to `false` for rolling-upgrade safety: an older
    /// (pre-KINTEG-002) daemon does not emit this field, and an absent marker
    /// semantically means "a fresh write" (not a duplicate).
    #[serde(default)]
    pub deduplicated: bool,
    /// Redaction evidence for the request's incoming content (KINTEG-006).
    /// Defaults to empty for rolling-upgrade safety: an older daemon does not
    /// emit this field, and an absent block means "no evidence reported".
    #[serde(default)]
    pub redaction: RedactionEvidence,
}

/// `POST /v1/pins` body — create a pin.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePinBody {
    /// What kind of entity is being pinned.
    pub target_type: PinTargetType,
    /// The id of the observation or summary to pin.
    pub target_id: Id,
    /// Optional free-text note.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Optional time-to-live in milliseconds; the pin expires after this.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_ms: Option<i64>,
    /// Optional scope override for the pin.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope_ids: Option<ScopeIds>,
}

impl CreatePinBody {
    /// Construct a minimal pin body for `target_type`/`target_id`.
    pub fn new(target_type: PinTargetType, target_id: impl Into<Id>) -> Self {
        Self {
            target_type,
            target_id: target_id.into(),
            note: None,
            ttl_ms: None,
            scope_ids: None,
        }
    }
}

/// `POST /v1/context/session-start` body. Both fields optional on the wire;
/// the client always sends `scopeIds` (a repo scope built from the project
/// root) to reproduce the Node hook's `{ repoId: <project root> }` filter.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionStartContextBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_results: Option<u32>,
    pub scope_ids: ScopeIds,
}

/// `POST /v1/context/pre-compact` body.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreCompactContextBody {
    pub scope_ids: ScopeIds,
}
