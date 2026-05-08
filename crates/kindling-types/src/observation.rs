use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[cfg(feature = "ts-rs")]
use ts_rs::TS;

use crate::common::{Id, ScopeIds, Timestamp};

/// Types of observations that can be captured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "snake_case")]
pub enum ObservationKind {
    ToolCall,
    Command,
    FileDiff,
    Error,
    Message,
    NodeStart,
    NodeEnd,
    NodeOutput,
    NodeError,
}

impl ObservationKind {
    /// All variants in declaration order. Mirrors `OBSERVATION_KINDS` in TS.
    pub const ALL: &'static [ObservationKind] = &[
        ObservationKind::ToolCall,
        ObservationKind::Command,
        ObservationKind::FileDiff,
        ObservationKind::Error,
        ObservationKind::Message,
        ObservationKind::NodeStart,
        ObservationKind::NodeEnd,
        ObservationKind::NodeOutput,
        ObservationKind::NodeError,
    ];
}

/// Atomic, immutable record of an event captured during development.
///
/// Mirrors `Observation` in `packages/kindling-core/src/types/observation.ts`.
/// The `redacted` flag is the only mutable field (via explicit redaction APIs).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    pub id: Id,
    pub kind: ObservationKind,
    pub content: String,
    /// Source-specific metadata stored as a JSON object.
    pub provenance: Map<String, Value>,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub ts: Timestamp,
    pub scope_ids: ScopeIds,
    pub redacted: bool,
}

/// Input for creating a new observation. Optional fields are auto-generated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "camelCase")]
pub struct ObservationInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub id: Option<Id>,
    pub kind: ObservationKind,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub provenance: Option<Map<String, Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number"))]
    pub ts: Option<Timestamp>,
    pub scope_ids: ScopeIds,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub redacted: Option<bool>,
}
