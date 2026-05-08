use serde::{Deserialize, Serialize};

#[cfg(feature = "ts-rs")]
use ts_rs::TS;

use crate::common::{Id, ScopeIds};
use crate::observation::Observation;
use crate::pin::Pin;
use crate::summary::Summary;

/// An entity returned from retrieval — either an observation or a summary.
/// Untagged so the wire format matches the TS structural union `Observation | Summary`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(untagged)]
pub enum RetrievedEntity {
    Observation(Observation),
    Summary(Summary),
}

/// Options for a retrieval request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "camelCase")]
pub struct RetrieveOptions {
    pub query: String,
    pub scope_ids: ScopeIds,
    /// Deprecated: token-budget assembly is a downstream-system responsibility.
    /// Prefer `max_candidates` for bounded result sets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub token_budget: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub max_candidates: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub include_redacted: Option<bool>,
}

/// Pin together with the observation or summary it points at.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "camelCase")]
pub struct PinResult {
    pub pin: Pin,
    pub target: RetrievedEntity,
}

/// Candidate (observation or summary) ranked by score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "camelCase")]
pub struct CandidateResult {
    pub entity: RetrievedEntity,
    pub score: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub match_context: Option<String>,
}

/// Provenance for a retrieval result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "camelCase")]
pub struct RetrieveProvenance {
    pub query: String,
    pub scope_ids: ScopeIds,
    pub total_candidates: u32,
    pub returned_candidates: u32,
    pub truncated_due_to_token_budget: bool,
    pub provider_used: String,
}

/// Complete retrieval result: pins, optional current summary, ranked candidates, provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "camelCase")]
pub struct RetrieveResult {
    pub pins: Vec<PinResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub current_summary: Option<Summary>,
    pub candidates: Vec<CandidateResult>,
    pub provenance: RetrieveProvenance,
}

/// Search options for a retrieval provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "camelCase")]
pub struct ProviderSearchOptions {
    pub query: String,
    pub scope_ids: ScopeIds,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub max_results: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub exclude_ids: Option<Vec<Id>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub include_redacted: Option<bool>,
}

/// A single result from a retrieval provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "camelCase")]
pub struct ProviderSearchResult {
    pub entity: RetrievedEntity,
    pub score: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub match_context: Option<String>,
}
