use serde::{Deserialize, Serialize};

#[cfg(feature = "ts-rs")]
use ts_rs::TS;

use crate::common::{Id, ScopeIds, Timestamp};

/// Type of entity that can be pinned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "snake_case")]
pub enum PinTargetType {
    Observation,
    Summary,
}

impl PinTargetType {
    pub const ALL: &'static [PinTargetType] = &[PinTargetType::Observation, PinTargetType::Summary];
}

/// Pinned reference to an observation or summary.
///
/// Mirrors `Pin` in `packages/kindling-core/src/types/pin.ts`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "camelCase")]
pub struct Pin {
    pub id: Id,
    pub target_type: PinTargetType,
    pub target_id: Id,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub reason: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub created_at: Timestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number"))]
    pub expires_at: Option<Timestamp>,
    pub scope_ids: ScopeIds,
}

/// Input for creating a new pin. Optional fields are auto-generated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "camelCase")]
pub struct PinInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub id: Option<Id>,
    pub target_type: PinTargetType,
    pub target_id: Id,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number"))]
    pub created_at: Option<Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number"))]
    pub expires_at: Option<Timestamp>,
    pub scope_ids: ScopeIds,
}

/// True iff the pin has not expired at `now` (epoch ms).
pub fn is_pin_active(pin: &Pin, now: Timestamp) -> bool {
    match pin.expires_at {
        None => true,
        Some(exp) => exp > now,
    }
}
