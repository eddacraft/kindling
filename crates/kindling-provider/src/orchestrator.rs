//! Retrieval orchestrator.
//!
//! Combines pins, the current session summary, and provider candidates into a
//! unified [`RetrieveResult`]. Mirrors `retrieve` in
//! `packages/kindling-core/src/retrieval/orchestrator.ts`:
//!
//! 1. Active pins (non-evictable, always included)
//! 2. Current session summary (non-evictable if it exists)
//! 3. Provider candidates (ranked, capped at `max_candidates`)
//!
//! Token-budget assembly is deliberately absent: `token_budget` is deprecated
//! and budgeting is a downstream-system responsibility.

use std::time::{SystemTime, UNIX_EPOCH};

use kindling_store::SqliteKindlingStore;
use kindling_types::{
    CandidateResult, Id, PinResult, PinTargetType, RetrieveOptions, RetrieveProvenance,
    RetrieveResult, RetrievedEntity, Timestamp,
};

use crate::error::ProviderResult;
use crate::provider::RetrievalProvider;

/// Default candidate cap when `max_candidates` is not given.
const DEFAULT_MAX_CANDIDATES: u32 = 10;

/// Retrieve relevant context for a query, scored as of the current time.
pub fn retrieve(
    store: &SqliteKindlingStore,
    provider: &impl RetrievalProvider,
    options: &RetrieveOptions,
) -> ProviderResult<RetrieveResult> {
    retrieve_at(store, provider, options, now_ms())
}

/// Retrieve relevant context for a query, scored as of `now` (epoch ms).
///
/// The explicit clock is the only deviation from the TS orchestrator (which
/// reads `Date.now()`); it keeps pin expiry and recency scoring deterministic.
pub fn retrieve_at(
    store: &SqliteKindlingStore,
    provider: &impl RetrievalProvider,
    options: &RetrieveOptions,
    now: Timestamp,
) -> ProviderResult<RetrieveResult> {
    let max_candidates = options.max_candidates.unwrap_or(DEFAULT_MAX_CANDIDATES);
    let include_redacted = options.include_redacted.unwrap_or(false);

    // Step 1: active pins for the scope.
    let pins = store.list_active_pins(Some(&options.scope_ids), Some(now))?;

    // Step 2: resolve pins to their targets.
    let mut pin_results: Vec<PinResult> = Vec::new();
    let mut pinned_ids: Vec<Id> = Vec::new();

    for pin in pins {
        let target = match pin.target_type {
            PinTargetType::Observation => store
                .get_observation_by_id(&pin.target_id)?
                .map(RetrievedEntity::Observation),
            PinTargetType::Summary => store
                .get_summary_by_id(&pin.target_id)?
                .map(RetrievedEntity::Summary),
        };

        if let Some(target) = target {
            // Skip redacted observations unless explicitly requested.
            if let RetrievedEntity::Observation(obs) = &target {
                if obs.redacted && !include_redacted {
                    continue;
                }
            }
            let target_id = entity_id(&target).to_string();
            if !pinned_ids.contains(&target_id) {
                pinned_ids.push(target_id);
            }
            pin_results.push(PinResult { pin, target });
        }
    }

    // Step 3: current session summary (non-evictable).
    // The TS orchestrator gates on JS truthiness, so an empty-string session
    // ID is treated as absent here too.
    let mut current_summary = None;
    if let Some(session_id) = options
        .scope_ids
        .session_id
        .as_deref()
        .filter(|s| !s.is_empty())
    {
        if let Some(capsule) = store.get_open_capsule_for_session(session_id)? {
            if let Some(summary) = store.get_latest_summary_for_capsule(&capsule.id)? {
                if !pinned_ids.contains(&summary.id) {
                    pinned_ids.push(summary.id.clone());
                }
                current_summary = Some(summary);
            }
        }
    }

    // Step 4: provider candidates, excluding pinned IDs and the current summary.
    let provider_results = provider.search(
        &kindling_types::ProviderSearchOptions {
            query: options.query.clone(),
            scope_ids: options.scope_ids.clone(),
            max_results: Some(max_candidates),
            exclude_ids: Some(pinned_ids),
            include_redacted: Some(include_redacted),
        },
        now,
    )?;

    // Step 5: convert provider results to candidates.
    let total_candidates = provider_results.len() as u32;
    let candidates: Vec<CandidateResult> = provider_results
        .into_iter()
        .map(|result| CandidateResult {
            entity: result.entity,
            score: result.score,
            match_context: result.match_context,
        })
        .collect();

    // Step 6: provenance.
    let provenance = RetrieveProvenance {
        query: options.query.clone(),
        scope_ids: options.scope_ids.clone(),
        total_candidates,
        returned_candidates: candidates.len() as u32,
        truncated_due_to_token_budget: false,
        provider_used: provider.name().to_string(),
    };

    Ok(RetrieveResult {
        pins: pin_results,
        current_summary,
        candidates,
        provenance,
    })
}

fn entity_id(entity: &RetrievedEntity) -> &str {
    match entity {
        RetrievedEntity::Observation(obs) => &obs.id,
        RetrievedEntity::Summary(sum) => &sum.id,
    }
}

fn now_ms() -> Timestamp {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_millis() as Timestamp
}
