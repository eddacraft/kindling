//! Request DTOs for the v1 HTTP API.
//!
//! The service option structs (`OpenCapsuleOptions`, `CloseCapsuleOptions`,
//! …) are deliberately NOT `Deserialize`, so this crate owns the wire-facing
//! request shapes and converts them into the service options. All bodies are
//! camelCase JSON. Response bodies serialize the domain types the service
//! returns (already camelCase) — no response DTOs are needed.

use kindling_service::{CloseCapsuleOptions, CreatePinOptions, OpenCapsuleOptions};
use kindling_types::{CapsuleType, Id, ObservationInput, PinTargetType, ScopeIds};
use serde::Deserialize;

/// `POST /v1/capsules` body — open a capsule.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCapsuleRequest {
    pub kind: CapsuleType,
    pub intent: String,
    #[serde(default)]
    pub scope_ids: ScopeIds,
    #[serde(default)]
    pub id: Option<Id>,
}

impl From<OpenCapsuleRequest> for OpenCapsuleOptions {
    fn from(r: OpenCapsuleRequest) -> Self {
        OpenCapsuleOptions {
            kind: r.kind,
            intent: r.intent,
            scope_ids: r.scope_ids,
            id: r.id,
        }
    }
}

/// `PATCH /v1/capsules/:id/close` body — close a capsule. All fields optional.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloseCapsuleRequest {
    #[serde(default)]
    pub generate_summary: Option<bool>,
    #[serde(default)]
    pub summary_content: Option<String>,
    #[serde(default)]
    pub confidence: Option<f64>,
}

impl From<CloseCapsuleRequest> for CloseCapsuleOptions {
    fn from(r: CloseCapsuleRequest) -> Self {
        CloseCapsuleOptions {
            generate_summary: r.generate_summary.unwrap_or(false),
            summary_content: r.summary_content,
            confidence: r.confidence,
        }
    }
}

/// `GET /v1/capsules/open` query — the session id whose open capsule to
/// resolve. Optional on the wire so the session id may instead arrive in the
/// `X-Kindling-Session` header; the handler rejects the request with `400`
/// when neither carries a non-empty value.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCapsuleQuery {
    #[serde(default)]
    pub session_id: Option<String>,
}

/// `POST /v1/observations` body — append an observation.
///
/// The wire shape is the `ObservationInput` fields (flattened) plus two
/// top-level routing/append options:
///   - `capsuleId` (optional): attach the new observation to this capsule.
///   - `validate` (optional, default true): run service validation.
///
/// Example:
/// ```json
/// {
///   "kind": "message",
///   "content": "hello",
///   "scopeIds": { "sessionId": "s1" },
///   "capsuleId": "cap-1",
///   "validate": true
/// }
/// ```
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppendObservationRequest {
    #[serde(flatten)]
    pub input: ObservationInput,
    #[serde(default)]
    pub capsule_id: Option<Id>,
    #[serde(default)]
    pub validate: Option<bool>,
}

/// `POST /v1/pins` body — create a pin.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePinRequest {
    pub target_type: PinTargetType,
    pub target_id: Id,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub ttl_ms: Option<i64>,
    #[serde(default)]
    pub scope_ids: Option<ScopeIds>,
}

impl From<CreatePinRequest> for CreatePinOptions {
    fn from(r: CreatePinRequest) -> Self {
        CreatePinOptions {
            target_type: r.target_type,
            target_id: r.target_id,
            note: r.note,
            ttl_ms: r.ttl_ms,
            scope_ids: r.scope_ids,
        }
    }
}

/// `POST /v1/context/session-start` body. Both fields optional.
///
/// - `maxResults` (default 10): cap on recent observations. Mirrors the Node
///   hook's `KINDLING_MAX_CONTEXT`.
/// - `scopeIds` (default empty): scope to assemble context for. The hook passes
///   `{ repoId: <project root> }`; the daemon already routes the *database* by
///   the `X-Kindling-Project` header, so this narrows *within* that DB.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStartContextRequest {
    #[serde(default)]
    pub max_results: Option<u32>,
    #[serde(default)]
    pub scope_ids: ScopeIds,
}

/// Default `maxResults` for SessionStart context — matches the Node hook's
/// `parseInt(KINDLING_MAX_CONTEXT || '10')`.
pub const DEFAULT_MAX_RESULTS: u32 = 10;

/// `POST /v1/context/pre-compact` body. The only field is the scope; an empty
/// body is valid.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreCompactContextRequest {
    #[serde(default)]
    pub scope_ids: ScopeIds,
}
