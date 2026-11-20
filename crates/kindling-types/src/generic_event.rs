//! Wire shapes, limits, and validation for `generic-event@1` (KINTEG-015).
//!
//! All structs and enums serialize as camelCase JSON and emit `ts-rs` bindings
//! when the `ts-rs` feature is enabled. Binary payloads serialize as padded
//! strict base64 (RFC 4648 §4) and project as `string` in TypeScript.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[cfg(feature = "ts-rs")]
use ts_rs::TS;

use crate::common::{ScopeIds, ValidationError};

/// Supported `generic-event@1` envelope version.
pub const GENERIC_EVENT_ENVELOPE_VERSION: u32 = 1;

/// Maximum decoded payload length in bytes (256 KiB).
pub const GENERIC_EVENT_MAX_PAYLOAD_BYTES: u64 = 262_144;

/// Maximum `key` length in bytes.
pub const GENERIC_EVENT_MAX_KEY_BYTES: u32 = 256;

/// Maximum `schemaName` length in bytes.
pub const GENERIC_EVENT_MAX_SCHEMA_NAME_BYTES: u32 = 128;

/// Maximum byte length of any single `scopeIds` field value.
pub const GENERIC_EVENT_MAX_SCOPE_VALUE_BYTES: usize = 256;

/// Capability feature name advertised in `/v1/health`.
pub const GENERIC_EVENT_FEATURE_NAME: &str = "generic-event";

/// Capability feature version advertised in `/v1/health`.
pub const GENERIC_EVENT_FEATURE_VERSION: u32 = 1;

/// Serde adapter for padded strict RFC 4648 §4 base64 byte payloads.
///
/// Rejects unpadded input, URL-safe alphabet characters (`-`, `_`),
/// whitespace, and non-canonical trailing bits.
pub mod base64_bytes {
    use super::*;

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        STANDARD.decode(encoded.as_bytes()).map_err(|err| {
            serde::de::Error::custom(format!("invalid strict base64 payload: {err}"))
        })
    }
}

/// Request envelope for `POST /v1/generic-events`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "camelCase")]
pub struct GenericEvent {
    pub envelope_version: u32,
    pub schema_name: String,
    pub schema_version: u32,
    pub key: String,
    #[serde(default)]
    pub scope_ids: ScopeIds,
    #[serde(with = "crate::generic_event::base64_bytes")]
    #[cfg_attr(feature = "ts-rs", ts(type = "string"))]
    pub payload: Vec<u8>,
}

/// Persisted scope of a generic event, including the daemon-derived `projectId`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "camelCase")]
pub struct GenericEventScope {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub repo_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub agent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub task_id: Option<String>,
}

impl GenericEventScope {
    /// Build a [`GenericEventScope`] from a daemon-derived `project_id` and
    /// caller-supplied [`ScopeIds`].
    pub fn from_project_and_scope(project_id: impl Into<String>, scope_ids: &ScopeIds) -> Self {
        Self {
            project_id: project_id.into(),
            session_id: scope_ids.session_id.clone(),
            repo_id: scope_ids.repo_id.clone(),
            agent_id: scope_ids.agent_id.clone(),
            user_id: scope_ids.user_id.clone(),
            task_id: scope_ids.task_id.clone(),
        }
    }
}

/// Receipt returned once a keyed generic event is durably committed.
///
/// Byte-identical on every replay, lookup, and forget response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "camelCase")]
pub struct GenericEventReceipt {
    pub event_id: String,
    pub envelope_version: u32,
    pub schema_name: String,
    pub schema_version: u32,
    pub key: String,
    pub scope: GenericEventScope,
    pub digest: String,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub committed_at: i64,
}

/// Response body of `POST /v1/generic-events/lookup`, tagged on `"state"`.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum GenericEventLookup {
    #[serde(rename_all = "camelCase")]
    Found {
        receipt: GenericEventReceipt,
        #[serde(with = "crate::generic_event::base64_bytes")]
        #[cfg_attr(feature = "ts-rs", ts(type = "string"))]
        payload: Vec<u8>,
    },
    #[serde(rename_all = "camelCase")]
    Redacted {
        receipt: GenericEventReceipt,
        #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
        redacted_at: i64,
    },
    NotFound,
}

/// Response body of `POST /v1/generic-events/forget`, tagged on `"state"`.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum GenericEventForget {
    #[serde(rename_all = "camelCase")]
    Redacted {
        receipt: GenericEventReceipt,
        #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
        redacted_at: i64,
    },
    NotFound,
}

/// Advertised limits for the `generic-event@1` capability entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "camelCase")]
pub struct GenericEventLimits {
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub max_payload_bytes: u64,
    pub max_key_bytes: u32,
    pub max_schema_name_bytes: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number"))]
    pub project_quota_bytes: Option<u64>,
    pub secret_detector_version: u32,
}

/// Detail enum for client/server payload rejection.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "camelCase")]
pub enum RejectReason {
    #[serde(rename_all = "camelCase")]
    InvalidEnvelope { field: String, message: String },
    #[serde(rename_all = "camelCase")]
    TooLarge {
        #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
        limit: u64,
        #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
        actual: u64,
    },
    #[serde(rename_all = "camelCase")]
    SecretDetected {
        field: String,
        classes: Vec<String>,
        count: u32,
        detector_version: u32,
    },
    #[serde(rename_all = "camelCase")]
    Unrecognized { code: String, message: String },
}

/// Detail enum for key conflicts on append.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "camelCase")]
pub enum ConflictKind {
    #[serde(rename_all = "camelCase")]
    Mismatch { existing: GenericEventReceipt },
    #[serde(rename_all = "camelCase")]
    Redacted {
        existing: GenericEventReceipt,
        #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
        redacted_at: i64,
    },
}

/// Detail enum for definite commit failures.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "camelCase")]
pub enum CommitFailure {
    Busy,
    Storage,
}

/// Validate `schemaName` against `^[a-z0-9][a-z0-9._-]{0,127}$`.
pub fn validate_schema_name(schema_name: &str) -> Result<(), ValidationError> {
    let bytes = schema_name.as_bytes();
    if bytes.is_empty() || bytes.len() > GENERIC_EVENT_MAX_SCHEMA_NAME_BYTES as usize {
        return Err(ValidationError {
            field: "schemaName".to_string(),
            message: format!(
                "schemaName must be 1..={} bytes",
                GENERIC_EVENT_MAX_SCHEMA_NAME_BYTES
            ),
            value: None,
        });
    }
    let first = bytes[0];
    if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return Err(ValidationError {
            field: "schemaName".to_string(),
            message: "schemaName must start with [a-z0-9]".to_string(),
            value: None,
        });
    }
    if !bytes[1..]
        .iter()
        .all(|&b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-'))
    {
        return Err(ValidationError {
            field: "schemaName".to_string(),
            message: "schemaName must match ^[a-z0-9][a-z0-9._-]{0,127}$".to_string(),
            value: None,
        });
    }
    Ok(())
}

/// Validate `key` against `^[\x21-\x7E]{1,256}$` (visible ASCII, byte-for-byte).
pub fn validate_event_key(key: &str) -> Result<(), ValidationError> {
    let bytes = key.as_bytes();
    if bytes.is_empty() || bytes.len() > GENERIC_EVENT_MAX_KEY_BYTES as usize {
        return Err(ValidationError {
            field: "key".to_string(),
            message: format!("key must be 1..={} bytes", GENERIC_EVENT_MAX_KEY_BYTES),
            value: None,
        });
    }
    if !bytes.iter().all(|&b| (0x21..=0x7E).contains(&b)) {
        return Err(ValidationError {
            field: "key".to_string(),
            message: "key must contain only visible ASCII bytes (0x21..=0x7E)".to_string(),
            value: None,
        });
    }
    Ok(())
}

/// Validate one optional `scopeIds` field value: 1..=256 bytes, no control chars, `""` rejected.
pub fn validate_scope_value(field: &str, value: &str) -> Result<(), ValidationError> {
    if value.is_empty() || value.len() > GENERIC_EVENT_MAX_SCOPE_VALUE_BYTES {
        return Err(ValidationError {
            field: field.to_string(),
            message: format!(
                "{field} must be 1..={} bytes",
                GENERIC_EVENT_MAX_SCOPE_VALUE_BYTES
            ),
            value: None,
        });
    }
    if value.chars().any(char::is_control) {
        return Err(ValidationError {
            field: field.to_string(),
            message: format!("{field} must not contain control characters"),
            value: None,
        });
    }
    Ok(())
}

/// Validate every present field in [`ScopeIds`] for a generic event.
pub fn validate_generic_event_scope_ids(scope_ids: &ScopeIds) -> Result<(), ValidationError> {
    for (field, value) in [
        ("scopeIds.sessionId", scope_ids.session_id.as_deref()),
        ("scopeIds.repoId", scope_ids.repo_id.as_deref()),
        ("scopeIds.agentId", scope_ids.agent_id.as_deref()),
        ("scopeIds.userId", scope_ids.user_id.as_deref()),
        ("scopeIds.taskId", scope_ids.task_id.as_deref()),
    ] {
        if let Some(v) = value {
            validate_scope_value(field, v)?;
        }
    }
    Ok(())
}

/// Validate a [`GenericEvent`] envelope's syntactic fields and payload size bounds.
///
/// Does not run secret detection (owned by `kindling-service`).
pub fn validate_generic_event_envelope(
    event: &GenericEvent,
    max_payload_bytes: u64,
) -> Result<(), RejectReason> {
    if event.envelope_version != GENERIC_EVENT_ENVELOPE_VERSION {
        return Err(RejectReason::InvalidEnvelope {
            field: "envelopeVersion".to_string(),
            message: format!(
                "unsupported envelopeVersion {}; supported: {}",
                event.envelope_version, GENERIC_EVENT_ENVELOPE_VERSION
            ),
        });
    }
    if let Err(err) = validate_schema_name(&event.schema_name) {
        return Err(RejectReason::InvalidEnvelope {
            field: err.field,
            message: err.message,
        });
    }
    if event.schema_version < 1 {
        return Err(RejectReason::InvalidEnvelope {
            field: "schemaVersion".to_string(),
            message: "schemaVersion must be >= 1".to_string(),
        });
    }
    if let Err(err) = validate_event_key(&event.key) {
        return Err(RejectReason::InvalidEnvelope {
            field: err.field,
            message: err.message,
        });
    }
    if let Err(err) = validate_generic_event_scope_ids(&event.scope_ids) {
        return Err(RejectReason::InvalidEnvelope {
            field: err.field,
            message: err.message,
        });
    }
    let payload_len = event.payload.len() as u64;
    if payload_len == 0 {
        return Err(RejectReason::InvalidEnvelope {
            field: "payload".to_string(),
            message: "payload must be at least 1 byte".to_string(),
        });
    }
    if payload_len > max_payload_bytes {
        return Err(RejectReason::TooLarge {
            limit: max_payload_bytes,
            actual: payload_len,
        });
    }
    Ok(())
}
