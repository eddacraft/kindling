//! Integration tests for the retrieval orchestrator.
//!
//! Mirrors the `retrieve` block of
//! `packages/kindling-core/test/retrieval.spec.ts`, using a mock provider for
//! orchestration-only behaviour and the real store throughout.

use kindling_provider::{retrieve_at, ProviderResult, RetrievalProvider};
use kindling_store::SqliteKindlingStore;
use kindling_types::{
    Observation, ObservationKind, Pin, PinTargetType, ProviderSearchOptions, ProviderSearchResult,
    RetrieveOptions, RetrievedEntity, ScopeIds, Summary, Timestamp,
};

const NOW: Timestamp = 1_700_000_000_000;

/// Mock provider mirroring `MockProvider` in retrieval.spec.ts: returns
/// pre-seeded results, honouring `exclude_ids` and `max_results`.
struct MockProvider {
    results: Vec<ProviderSearchResult>,
}

impl RetrievalProvider for MockProvider {
    fn name(&self) -> &str {
        "mock-provider"
    }

    fn search(
        &self,
        options: &ProviderSearchOptions,
        _now: Timestamp,
    ) -> ProviderResult<Vec<ProviderSearchResult>> {
        let exclude = options.exclude_ids.clone().unwrap_or_default();
        let mut filtered: Vec<ProviderSearchResult> = self
            .results
            .iter()
            .filter(|r| !exclude.contains(&entity_id(&r.entity).to_string()))
            .cloned()
            .collect();
        if let Some(max) = options.max_results {
            filtered.truncate(max as usize);
        }
        Ok(filtered)
    }
}

fn entity_id(entity: &RetrievedEntity) -> &str {
    match entity {
        RetrievedEntity::Observation(obs) => &obs.id,
        RetrievedEntity::Summary(sum) => &sum.id,
    }
}

fn session_scope(session_id: &str) -> ScopeIds {
    ScopeIds {
        session_id: Some(session_id.to_string()),
        ..ScopeIds::default()
    }
}

fn observation(id: &str, content: &str, redacted: bool) -> Observation {
    Observation {
        id: id.to_string(),
        kind: ObservationKind::Message,
        content: content.to_string(),
        provenance: serde_json::Map::new(),
        ts: NOW,
        scope_ids: session_scope("s1"),
        redacted,
    }
}

fn pin(
    id: &str,
    target_type: PinTargetType,
    target_id: &str,
    expires_at: Option<Timestamp>,
) -> Pin {
    Pin {
        id: id.to_string(),
        target_type,
        target_id: target_id.to_string(),
        reason: None,
        created_at: NOW,
        expires_at,
        scope_ids: session_scope("s1"),
    }
}

fn summary(id: &str, capsule_id: &str, content: &str) -> Summary {
    Summary {
        id: id.to_string(),
        capsule_id: capsule_id.to_string(),
        content: content.to_string(),
        confidence: 0.9,
        created_at: NOW,
        evidence_refs: Vec::new(),
    }
}

fn open_capsule(id: &str, session_id: &str) -> kindling_types::Capsule {
    kindling_types::Capsule {
        id: id.to_string(),
        kind: kindling_types::CapsuleType::Session,
        intent: "Test".to_string(),
        status: kindling_types::CapsuleStatus::Open,
        opened_at: NOW,
        closed_at: None,
        scope_ids: session_scope(session_id),
        observation_ids: Vec::new(),
        summary_id: None,
    }
}

fn retrieve_options(query: &str) -> RetrieveOptions {
    RetrieveOptions {
        query: query.to_string(),
        scope_ids: session_scope("s1"),
        token_budget: None,
        max_candidates: None,
        include_redacted: None,
    }
}

fn provider_result(
    entity: RetrievedEntity,
    score: f64,
    context: Option<&str>,
) -> ProviderSearchResult {
    ProviderSearchResult {
        entity,
        score,
        match_context: context.map(String::from),
    }
}

#[test]
fn returns_empty_result_when_no_data_exists() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    let provider = MockProvider { results: vec![] };

    let result = retrieve_at(&store, &provider, &retrieve_options("test query"), NOW).unwrap();

    assert!(result.pins.is_empty());
    assert!(result.current_summary.is_none());
    assert!(result.candidates.is_empty());
    assert_eq!(result.provenance.query, "test query");
    assert_eq!(result.provenance.provider_used, "mock-provider");
}

#[test]
fn includes_active_pins_with_resolved_targets() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    store
        .insert_observation(&observation("obs-1", "Pinned message", false))
        .unwrap();
    store
        .insert_pin(&pin("pin-1", PinTargetType::Observation, "obs-1", None))
        .unwrap();
    let provider = MockProvider { results: vec![] };

    let result = retrieve_at(&store, &provider, &retrieve_options("test"), NOW).unwrap();

    assert_eq!(result.pins.len(), 1);
    assert_eq!(result.pins[0].pin.id, "pin-1");
    assert_eq!(entity_id(&result.pins[0].target), "obs-1");
}

#[test]
fn excludes_expired_pins() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    store
        .insert_observation(&observation("obs-1", "Expired pin", false))
        .unwrap();
    store
        .insert_pin(&pin(
            "pin-1",
            PinTargetType::Observation,
            "obs-1",
            Some(NOW - 1),
        ))
        .unwrap();
    let provider = MockProvider { results: vec![] };

    let result = retrieve_at(&store, &provider, &retrieve_options("test"), NOW).unwrap();

    assert!(result.pins.is_empty());
}

#[test]
fn excludes_redacted_pins_by_default() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    store
        .insert_observation(&observation("obs-1", "[redacted]", true))
        .unwrap();
    store
        .insert_pin(&pin("pin-1", PinTargetType::Observation, "obs-1", None))
        .unwrap();
    let provider = MockProvider { results: vec![] };

    let result = retrieve_at(&store, &provider, &retrieve_options("test"), NOW).unwrap();

    assert!(result.pins.is_empty());
}

#[test]
fn includes_redacted_pins_when_requested() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    store
        .insert_observation(&observation("obs-1", "[redacted]", true))
        .unwrap();
    store
        .insert_pin(&pin("pin-1", PinTargetType::Observation, "obs-1", None))
        .unwrap();
    let provider = MockProvider { results: vec![] };

    let mut opts = retrieve_options("test");
    opts.include_redacted = Some(true);
    let result = retrieve_at(&store, &provider, &opts, NOW).unwrap();

    assert_eq!(result.pins.len(), 1);
}

#[test]
fn includes_current_session_summary() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    store.create_capsule(&open_capsule("cap-1", "s1")).unwrap();
    store
        .insert_summary(&summary("sum-1", "cap-1", "Current session summary"))
        .unwrap();
    let provider = MockProvider { results: vec![] };

    let result = retrieve_at(&store, &provider, &retrieve_options("test"), NOW).unwrap();

    assert_eq!(
        result.current_summary.as_ref().map(|s| s.id.as_str()),
        Some("sum-1")
    );
}

#[test]
fn includes_provider_candidates() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    let provider = MockProvider {
        results: vec![provider_result(
            RetrievedEntity::Observation(observation("obs-1", "Provider result", false)),
            0.95,
            Some("exact match"),
        )],
    };

    let result = retrieve_at(&store, &provider, &retrieve_options("test"), NOW).unwrap();

    assert_eq!(result.candidates.len(), 1);
    assert_eq!(entity_id(&result.candidates[0].entity), "obs-1");
    assert_eq!(result.candidates[0].score, 0.95);
}

#[test]
fn excludes_pinned_ids_from_candidates() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    store
        .insert_observation(&observation("obs-1", "Pinned", false))
        .unwrap();
    store
        .insert_pin(&pin("pin-1", PinTargetType::Observation, "obs-1", None))
        .unwrap();
    let provider = MockProvider {
        results: vec![
            provider_result(
                RetrievedEntity::Observation(observation("obs-1", "Pinned", false)),
                0.95,
                Some("match 1"),
            ),
            provider_result(
                RetrievedEntity::Observation(observation("obs-2", "Not pinned", false)),
                0.9,
                Some("match 2"),
            ),
        ],
    };

    let result = retrieve_at(&store, &provider, &retrieve_options("test"), NOW).unwrap();

    assert_eq!(result.pins.len(), 1);
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(entity_id(&result.candidates[0].entity), "obs-2");
}

#[test]
fn excludes_current_summary_from_candidates() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    store.create_capsule(&open_capsule("cap-1", "s1")).unwrap();
    store
        .insert_summary(&summary("sum-1", "cap-1", "Current summary"))
        .unwrap();
    let provider = MockProvider {
        results: vec![provider_result(
            RetrievedEntity::Summary(summary("sum-1", "cap-1", "Current summary")),
            0.95,
            Some("match"),
        )],
    };

    let result = retrieve_at(&store, &provider, &retrieve_options("test"), NOW).unwrap();

    assert!(result.current_summary.is_some());
    assert!(result.candidates.is_empty());
}

#[test]
fn respects_max_candidates_limit() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    let provider = MockProvider {
        results: (1..=5)
            .map(|i| {
                provider_result(
                    RetrievedEntity::Observation(observation(
                        &format!("obs-{i}"),
                        &format!("Message {i}"),
                        false,
                    )),
                    1.0 - 0.1 * f64::from(i),
                    None,
                )
            })
            .collect(),
    };

    let mut opts = retrieve_options("test");
    opts.max_candidates = Some(3);
    let result = retrieve_at(&store, &provider, &opts, NOW).unwrap();

    assert_eq!(result.candidates.len(), 3);
    assert_eq!(result.provenance.total_candidates, 3);
}

#[test]
fn end_to_end_with_local_fts_provider() {
    use kindling_provider::LocalFtsProvider;

    let store = SqliteKindlingStore::open_in_memory().unwrap();
    store.create_capsule(&open_capsule("cap-1", "s1")).unwrap();
    store
        .insert_summary(&summary("sum-1", "cap-1", "authentication work summary"))
        .unwrap();
    store
        .insert_observation(&observation(
            "obs-pinned",
            "authentication pinned note",
            false,
        ))
        .unwrap();
    store
        .insert_pin(&pin(
            "pin-1",
            PinTargetType::Observation,
            "obs-pinned",
            None,
        ))
        .unwrap();
    store
        .insert_observation(&observation("obs-hit", "authentication candidate", false))
        .unwrap();

    let provider = LocalFtsProvider::from_store(&store);
    let result = retrieve_at(&store, &provider, &retrieve_options("authentication"), NOW).unwrap();

    // Pinned observation and current summary are tier 0/1, and excluded from
    // candidates; only obs-hit remains a ranked candidate.
    assert_eq!(result.pins.len(), 1);
    assert_eq!(entity_id(&result.pins[0].target), "obs-pinned");
    assert_eq!(
        result.current_summary.as_ref().map(|s| s.id.as_str()),
        Some("sum-1")
    );
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(entity_id(&result.candidates[0].entity), "obs-hit");
    assert_eq!(result.provenance.provider_used, "local-fts");
}
