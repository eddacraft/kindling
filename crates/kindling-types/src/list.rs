//! Types for the exhaustive, deterministically-paginated observation list
//! (`POST /v1/observations/list`).
//!
//! Distinct from [`crate::RetrieveOptions`] / [`crate::RetrieveResult`], which
//! are *ranked* top-K retrieval. The list API enumerates the **full** set of
//! observations matching a `(kind, scope, time-range)` filter, in the stable
//! `(ts ASC, id ASC)` order the store guarantees, via a keyset cursor — so a
//! consumer can compute exact counts / set-differences over every matching row.

use serde::{Deserialize, Serialize};

#[cfg(feature = "ts-rs")]
use ts_rs::TS;

use crate::common::{ScopeIds, Timestamp};
use crate::observation::{Observation, ObservationKind};

/// Filter + pagination for an observation list request.
///
/// Time bounds are **half-open**: `since` is inclusive, `until` is exclusive, so
/// two adjacent range polls `[t0, t1)` + `[t1, t2)` never double-count the
/// boundary. `task_id` is intentionally **not** a filter dimension (it is carried
/// only for provenance) — `scope_ids.task_id` is ignored if set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "camelCase")]
pub struct ListObservationsRequest {
    /// Scope filter (session / repo / agent / user).
    #[serde(default)]
    pub scope_ids: ScopeIds,
    /// Restrict to these observation kinds. Empty (or omitted) = all kinds.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kinds: Vec<ObservationKind>,
    /// Inclusive lower bound on `ts` (epoch ms).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number"))]
    pub since: Option<Timestamp>,
    /// Exclusive upper bound on `ts` (epoch ms).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number"))]
    pub until: Option<Timestamp>,
    /// Max rows per page. Server-clamped to `[1, 1000]`; default 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub limit: Option<u32>,
    /// Opaque cursor from a prior response's `nextCursor`. Treat as a token; do
    /// not parse or construct it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub cursor: Option<String>,
    /// Include redacted rows. Default `false`. A redacted row keeps its `kind`,
    /// scope and provenance but its `content` is `[redacted]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub include_redacted: Option<bool>,
}

/// One page of observations in stable `(ts ASC, id ASC)` order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "camelCase")]
pub struct ListObservationsResult {
    /// The matching observations for this page.
    pub observations: Vec<Observation>,
    /// Present iff more rows remain — pass it back as `cursor` to fetch the next
    /// page. **Absent means enumeration is complete** (the completeness signal a
    /// consumer needs for an exact count / set-difference).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub next_cursor: Option<String>,
}
