//! Capability handshake and machine-readable kind registry.
//!
//! Pure, side-effect-free assembly used by `/v1/health`, `kindling status --json`,
//! and the daemon client. The kind registry is derived from [`ObservationKind::ALL`]
//! so it cannot drift from the canonical enum definition.

use serde::{Deserialize, Serialize};

#[cfg(feature = "ts-rs")]
use ts_rs::TS;

use crate::observation::ObservationKind;

/// Observation fields every kind must carry on the wire (camelCase keys).
pub const OBSERVATION_REQUIRED_FIELDS: &[&str] = &["content", "provenance", "scopeIds", "ts"];

/// One entry in the machine-readable kind registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "camelCase")]
pub struct KindRegistryEntry {
    /// Snake-case kind name (matches the `ObservationKind` wire encoding).
    pub kind: String,
    /// Required observation + documented provenance fields for this kind.
    pub required_fields: Vec<String>,
}

/// Capability block surfaced by health and `kindling status --json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "camelCase")]
pub struct Capability {
    pub version: String,
    pub schema_version: u32,
    pub supported_kinds: Vec<String>,
    pub storage_path: String,
    pub kind_registry: Vec<KindRegistryEntry>,
}

impl ObservationKind {
    /// Wire name (snake_case) for this kind.
    pub fn wire_name(self) -> &'static str {
        match self {
            ObservationKind::ToolCall => "tool_call",
            ObservationKind::Command => "command",
            ObservationKind::FileDiff => "file_diff",
            ObservationKind::Error => "error",
            ObservationKind::Message => "message",
            ObservationKind::NodeStart => "node_start",
            ObservationKind::NodeEnd => "node_end",
            ObservationKind::NodeOutput => "node_output",
            ObservationKind::NodeError => "node_error",
        }
    }

    /// Documented provenance keys adapters emit for this kind (camelCase).
    pub fn documented_provenance_fields(self) -> &'static [&'static str] {
        match self {
            ObservationKind::ToolCall => &["toolName", "hasError"],
            ObservationKind::Command => &["toolName", "hasError", "command"],
            ObservationKind::FileDiff => &["toolName", "hasError", "filePath"],
            ObservationKind::Error => &["source"],
            ObservationKind::Message => &["role", "length"],
            ObservationKind::NodeStart => &["nodeName"],
            ObservationKind::NodeEnd => &["nodeName", "duration", "status"],
            ObservationKind::NodeOutput => &["nodeName", "outputType", "duration"],
            ObservationKind::NodeError => &["nodeName", "errorType", "errorMessage"],
        }
    }

    /// Full required-field list: base observation fields plus provenance keys.
    pub fn required_fields(self) -> Vec<String> {
        let mut fields: Vec<String> = OBSERVATION_REQUIRED_FIELDS
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        fields.extend(
            self.documented_provenance_fields()
                .iter()
                .map(|s| (*s).to_string()),
        );
        fields
    }
}

/// Snake-case kind names for every [`ObservationKind`] variant, in declaration order.
pub fn supported_kind_names() -> Vec<String> {
    ObservationKind::ALL
        .iter()
        .map(|k| k.wire_name().to_string())
        .collect()
}

/// Machine-readable registry listing every kind with its required fields.
pub fn kind_registry() -> Vec<KindRegistryEntry> {
    ObservationKind::ALL
        .iter()
        .map(|&kind| KindRegistryEntry {
            kind: kind.wire_name().to_string(),
            required_fields: kind.required_fields(),
        })
        .collect()
}

/// Assemble the capability block from runtime inputs (version, schema, storage path).
pub fn build_capability(
    version: impl Into<String>,
    schema_version: u32,
    storage_path: impl Into<String>,
) -> Capability {
    Capability {
        version: version.into(),
        schema_version,
        supported_kinds: supported_kind_names(),
        storage_path: storage_path.into(),
        kind_registry: kind_registry(),
    }
}
