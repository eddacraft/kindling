//! Input validation + normalisation.
//!
//! Ports the relevant runtime checks from
//! `packages/kindling-core/src/validation/{capsule,observation,pin,summary}.ts`.
//!
//! Many TS checks (`typeof === 'number'`, `isObservationKind`, "must be an
//! object") are enforced by Rust's type system at the `*Input` boundary and so
//! are omitted here. What remains are the constraints the types cannot express:
//! non-empty-after-trim strings, non-negative timestamps, and confidence in
//! `[0, 1]`. Defaulting (fresh ids, `now` timestamps, empty collections)
//! happens here too, mirroring each validator's normalisation step.

use serde_json::Value;
use uuid::Uuid;

use kindling_types::{
    is_valid_confidence, Capsule, CapsuleInput, CapsuleStatus, Observation, ObservationInput, Pin,
    PinInput, Summary, SummaryInput, Timestamp, ValidationError,
};

/// A bare UUIDv4 with no prefix — matches `randomUUID()` defaulting in the TS
/// capsule/observation validators.
fn fresh_uuid() -> String {
    Uuid::new_v4().to_string()
}

/// True when `s` is empty after trimming surrounding whitespace, mirroring the
/// TS `value.trim().length === 0` checks.
fn is_blank(s: &str) -> bool {
    s.trim().is_empty()
}

fn error(field: &str, message: &str) -> ValidationError {
    ValidationError {
        field: field.to_string(),
        message: message.to_string(),
        value: None,
    }
}

fn error_with(field: &str, message: &str, value: Value) -> ValidationError {
    ValidationError {
        field: field.to_string(),
        message: message.to_string(),
        value: Some(value),
    }
}

/// Validate + normalise a capsule input as of `now`.
///
/// Defaults: `id` → fresh UUID, `status` → Open, `opened_at` → `now`,
/// `observation_ids` → `[]`.
pub fn validate_capsule(
    input: CapsuleInput,
    now: Timestamp,
) -> Result<Capsule, Vec<ValidationError>> {
    let mut errors = Vec::new();

    if is_blank(&input.intent) {
        errors.push(error("intent", "intent cannot be empty"));
    }
    if let Some(opened_at) = input.opened_at {
        if opened_at < 0 {
            errors.push(error_with(
                "openedAt",
                "openedAt must be non-negative",
                Value::from(opened_at),
            ));
        }
    }
    if let Some(closed_at) = input.closed_at {
        if closed_at < 0 {
            errors.push(error_with(
                "closedAt",
                "closedAt must be non-negative",
                Value::from(closed_at),
            ));
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(Capsule {
        id: input.id.unwrap_or_else(fresh_uuid),
        kind: input.kind,
        intent: input.intent,
        status: input.status.unwrap_or(CapsuleStatus::Open),
        opened_at: input.opened_at.unwrap_or(now),
        closed_at: input.closed_at,
        scope_ids: input.scope_ids,
        observation_ids: input.observation_ids.unwrap_or_default(),
        summary_id: input.summary_id,
    })
}

/// Validate + normalise an observation input as of `now`.
///
/// Defaults: `id` → fresh UUID, `ts` → `now`, `redacted` → false,
/// `provenance` → empty object.
pub fn validate_observation(
    input: ObservationInput,
    now: Timestamp,
) -> Result<Observation, Vec<ValidationError>> {
    let mut errors = Vec::new();

    if is_blank(&input.content) {
        errors.push(error("content", "content cannot be empty"));
    }
    if let Some(ts) = input.ts {
        if ts < 0 {
            errors.push(error_with("ts", "ts must be non-negative", Value::from(ts)));
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(Observation {
        id: input.id.unwrap_or_else(fresh_uuid),
        kind: input.kind,
        content: input.content,
        provenance: input.provenance.unwrap_or_default(),
        ts: input.ts.unwrap_or(now),
        scope_ids: input.scope_ids,
        redacted: input.redacted.unwrap_or(false),
    })
}

/// Normalise an observation input WITHOUT running validation (the
/// `validate: false` path). Still applies all defaulting so the stored record
/// is well-formed.
pub fn normalize_observation(input: ObservationInput, now: Timestamp) -> Observation {
    Observation {
        id: input.id.unwrap_or_else(fresh_uuid),
        kind: input.kind,
        content: input.content,
        provenance: input.provenance.unwrap_or_default(),
        ts: input.ts.unwrap_or(now),
        scope_ids: input.scope_ids,
        redacted: input.redacted.unwrap_or(false),
    }
}

/// Validate + normalise a summary input as of `now`.
///
/// Defaults: `id` → `sum_<uuid>`, `created_at` → `now`.
pub fn validate_summary(
    input: SummaryInput,
    now: Timestamp,
) -> Result<Summary, Vec<ValidationError>> {
    let mut errors = Vec::new();

    if is_blank(&input.capsule_id) {
        errors.push(error("capsuleId", "capsuleId cannot be empty"));
    }
    if is_blank(&input.content) {
        errors.push(error("content", "content cannot be empty"));
    }
    if !is_valid_confidence(input.confidence) {
        errors.push(error_with(
            "confidence",
            "confidence must be between 0.0 and 1.0",
            Value::from(input.confidence),
        ));
    }
    if let Some(created_at) = input.created_at {
        if created_at < 0 {
            errors.push(error_with(
                "createdAt",
                "createdAt must be non-negative",
                Value::from(created_at),
            ));
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(Summary {
        id: input.id.unwrap_or_else(|| format!("sum_{}", fresh_uuid())),
        capsule_id: input.capsule_id,
        content: input.content,
        confidence: input.confidence,
        created_at: input.created_at.unwrap_or(now),
        evidence_refs: input.evidence_refs,
    })
}

/// Validate + normalise a pin input as of `now`.
///
/// Defaults: `id` → `pin_<uuid>`, `created_at` → `now`.
pub fn validate_pin(input: PinInput, now: Timestamp) -> Result<Pin, Vec<ValidationError>> {
    let mut errors = Vec::new();

    if is_blank(&input.target_id) {
        errors.push(error("targetId", "targetId cannot be empty"));
    }
    if let Some(created_at) = input.created_at {
        if created_at < 0 {
            errors.push(error_with(
                "createdAt",
                "createdAt must be non-negative",
                Value::from(created_at),
            ));
        }
    }
    if let Some(expires_at) = input.expires_at {
        if expires_at < 0 {
            errors.push(error_with(
                "expiresAt",
                "expiresAt must be non-negative",
                Value::from(expires_at),
            ));
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(Pin {
        id: input.id.unwrap_or_else(|| format!("pin_{}", fresh_uuid())),
        target_type: input.target_type,
        target_id: input.target_id,
        reason: input.reason,
        created_at: input.created_at.unwrap_or(now),
        expires_at: input.expires_at,
        scope_ids: input.scope_ids,
    })
}
