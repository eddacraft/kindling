use serde::{Deserialize, Serialize};

#[cfg(feature = "ts-rs")]
use ts_rs::TS;

/// Unique identifier for entities. Implementation uses UUIDv4 format.
pub type Id = String;

/// Timestamp in epoch milliseconds.
pub type Timestamp = i64;

/// Scope identifiers for multi-dimensional isolation.
///
/// All fields are optional to support partial scoping. Mirrors `ScopeIds`
/// in `packages/kindling-core/src/types/common.ts`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "camelCase")]
pub struct ScopeIds {
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

/// Validation error details.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "camelCase")]
pub struct ValidationError {
    pub field: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub value: Option<serde_json::Value>,
}
