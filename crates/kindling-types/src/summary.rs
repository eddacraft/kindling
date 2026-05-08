use serde::{Deserialize, Serialize};

#[cfg(feature = "ts-rs")]
use ts_rs::TS;

use crate::common::{Id, Timestamp};

/// High-level description of a capsule's content (typically LLM-generated).
///
/// Mirrors `Summary` in `packages/kindling-core/src/types/summary.ts`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub id: Id,
    pub capsule_id: Id,
    pub content: String,
    /// Quality/confidence score in `[0.0, 1.0]`.
    pub confidence: f64,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub created_at: Timestamp,
    pub evidence_refs: Vec<Id>,
}

/// Input for creating a new summary. Optional fields are auto-generated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "camelCase")]
pub struct SummaryInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub id: Option<Id>,
    pub capsule_id: Id,
    pub content: String,
    pub confidence: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number"))]
    pub created_at: Option<Timestamp>,
    pub evidence_refs: Vec<Id>,
}

/// Validate that confidence score is in `[0.0, 1.0]` and not NaN.
pub fn is_valid_confidence(value: f64) -> bool {
    !value.is_nan() && (0.0..=1.0).contains(&value)
}
