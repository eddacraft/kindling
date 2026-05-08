use serde::{Deserialize, Serialize};

#[cfg(feature = "ts-rs")]
use ts_rs::TS;

use crate::common::{Id, ScopeIds, Timestamp};

/// Types of capsules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "snake_case")]
pub enum CapsuleType {
    Session,
    PocketflowNode,
}

impl CapsuleType {
    pub const ALL: &'static [CapsuleType] = &[CapsuleType::Session, CapsuleType::PocketflowNode];
}

/// Capsule lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "snake_case")]
pub enum CapsuleStatus {
    Open,
    Closed,
}

impl CapsuleStatus {
    pub const ALL: &'static [CapsuleStatus] = &[CapsuleStatus::Open, CapsuleStatus::Closed];
}

/// Bounded unit of meaning grouping related observations.
///
/// Mirrors `Capsule` in `packages/kindling-core/src/types/capsule.ts`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "camelCase")]
pub struct Capsule {
    pub id: Id,
    #[serde(rename = "type")]
    #[cfg_attr(feature = "ts-rs", ts(rename = "type"))]
    pub kind: CapsuleType,
    pub intent: String,
    pub status: CapsuleStatus,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub opened_at: Timestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number"))]
    pub closed_at: Option<Timestamp>,
    pub scope_ids: ScopeIds,
    pub observation_ids: Vec<Id>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub summary_id: Option<Id>,
}

/// Input for creating a new capsule. Optional fields are auto-generated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "camelCase")]
pub struct CapsuleInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub id: Option<Id>,
    #[serde(rename = "type")]
    #[cfg_attr(feature = "ts-rs", ts(rename = "type"))]
    pub kind: CapsuleType,
    pub intent: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub status: Option<CapsuleStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number"))]
    pub opened_at: Option<Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number"))]
    pub closed_at: Option<Timestamp>,
    pub scope_ids: ScopeIds,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub observation_ids: Option<Vec<Id>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub summary_id: Option<Id>,
}
