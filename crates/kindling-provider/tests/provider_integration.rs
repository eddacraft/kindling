//! Integration tests for `LocalFtsProvider`.
//!
//! Mirrors `packages/kindling-provider-local/test/local-fts.spec.ts` so the
//! Rust provider demonstrably reproduces the TS provider's behaviour.

use kindling_provider::{LocalFtsProvider, RetrievalProvider};
use kindling_store::SqliteKindlingStore;
use kindling_types::{
    Observation, ObservationKind, ProviderSearchOptions, ProviderSearchResult, RetrievedEntity,
    ScopeIds, Summary, Timestamp,
};

const NOW: Timestamp = 1_700_000_000_000;
const DAY_MS: Timestamp = 24 * 60 * 60 * 1000;

fn scope(session_id: Option<&str>, repo_id: Option<&str>) -> ScopeIds {
    ScopeIds {
        session_id: session_id.map(String::from),
        repo_id: repo_id.map(String::from),
        ..ScopeIds::default()
    }
}

fn observation(id: &str, content: &str, scope_ids: ScopeIds, ts: Timestamp) -> Observation {
    Observation {
        id: id.to_string(),
        kind: ObservationKind::Message,
        content: content.to_string(),
        provenance: serde_json::Map::new(),
        ts,
        scope_ids,
        redacted: false,
    }
}

fn summary(id: &str, capsule_id: &str, content: &str, created_at: Timestamp) -> Summary {
    Summary {
        id: id.to_string(),
        capsule_id: capsule_id.to_string(),
        content: content.to_string(),
        confidence: 0.9,
        created_at,
        evidence_refs: Vec::new(),
    }
}

fn capsule(id: &str, scope_ids: ScopeIds) -> kindling_types::Capsule {
    kindling_types::Capsule {
        id: id.to_string(),
        kind: kindling_types::CapsuleType::Session,
        intent: "Test".to_string(),
        status: kindling_types::CapsuleStatus::Open,
        opened_at: NOW,
        closed_at: None,
        scope_ids,
        observation_ids: Vec::new(),
        summary_id: None,
    }
}

fn options(query: &str, scope_ids: ScopeIds) -> ProviderSearchOptions {
    ProviderSearchOptions {
        query: query.to_string(),
        scope_ids,
        max_results: None,
        exclude_ids: None,
        include_redacted: None,
    }
}

fn search(store: &SqliteKindlingStore, opts: &ProviderSearchOptions) -> Vec<ProviderSearchResult> {
    LocalFtsProvider::from_store(store)
        .search(opts, NOW)
        .expect("search should succeed")
}

fn entity_id(result: &ProviderSearchResult) -> &str {
    match &result.entity {
        RetrievedEntity::Observation(obs) => &obs.id,
        RetrievedEntity::Summary(sum) => &sum.id,
    }
}

// ===== FTS search =====

#[test]
fn finds_observations_matching_query() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    store
        .insert_observation(&observation(
            "obs-1",
            "Fixed authentication bug in login flow",
            scope(Some("s1"), Some("/repo")),
            NOW,
        ))
        .unwrap();
    store
        .insert_observation(&observation(
            "obs-2",
            "Updated documentation for API",
            scope(Some("s1"), Some("/repo")),
            NOW,
        ))
        .unwrap();

    let results = search(
        &store,
        &options("authentication", scope(None, Some("/repo"))),
    );

    assert_eq!(results.len(), 1);
    assert_eq!(entity_id(&results[0]), "obs-1");
    assert!(results[0].score > 0.0);
    assert!(results[0].score <= 1.0);
}

#[test]
fn finds_summaries_matching_query() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    store
        .create_capsule(&capsule("cap-1", scope(Some("s1"), None)))
        .unwrap();
    store
        .insert_summary(&summary(
            "sum-1",
            "cap-1",
            "Refactored authentication module for security",
            NOW,
        ))
        .unwrap();

    let results = search(&store, &options("authentication", ScopeIds::default()));

    assert_eq!(results.len(), 1);
    assert_eq!(entity_id(&results[0]), "sum-1");
}

#[test]
fn finds_both_observations_and_summaries() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    store
        .create_capsule(&capsule("cap-1", scope(Some("s1"), None)))
        .unwrap();
    store
        .insert_observation(&observation(
            "obs-1",
            "authentication bug fixed",
            scope(Some("s1"), None),
            NOW,
        ))
        .unwrap();
    store
        .insert_summary(&summary(
            "sum-1",
            "cap-1",
            "Updated authentication flow",
            NOW,
        ))
        .unwrap();

    let results = search(&store, &options("authentication", ScopeIds::default()));

    assert_eq!(results.len(), 2);
    let ids: Vec<&str> = results.iter().map(entity_id).collect();
    assert!(ids.contains(&"obs-1"));
    assert!(ids.contains(&"sum-1"));
}

#[test]
fn returns_empty_for_no_matches() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    store
        .insert_observation(&observation(
            "obs-1",
            "test content",
            scope(Some("s1"), None),
            NOW,
        ))
        .unwrap();

    let results = search(&store, &options("nonexistent", ScopeIds::default()));

    assert!(results.is_empty());
}

// ===== Scope filtering =====

fn scope_fixture() -> SqliteKindlingStore {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    for (id, session, repo) in [
        ("obs-1", "s1", "/repo1"),
        ("obs-2", "s2", "/repo1"),
        ("obs-3", "s1", "/repo2"),
    ] {
        store
            .insert_observation(&observation(
                id,
                "authentication test",
                scope(Some(session), Some(repo)),
                NOW,
            ))
            .unwrap();
    }
    store
}

#[test]
fn filters_by_session_id() {
    let store = scope_fixture();
    let results = search(&store, &options("authentication", scope(Some("s1"), None)));
    assert_eq!(results.len(), 2);
}

#[test]
fn filters_by_repo_id() {
    let store = scope_fixture();
    let results = search(
        &store,
        &options("authentication", scope(None, Some("/repo1"))),
    );
    assert_eq!(results.len(), 2);
}

#[test]
fn filters_by_multiple_scope_dimensions_with_and_semantics() {
    let store = scope_fixture();
    let results = search(
        &store,
        &options("authentication", scope(Some("s1"), Some("/repo1"))),
    );
    assert_eq!(results.len(), 1);
}

#[test]
fn returns_all_results_when_no_scope_specified() {
    let store = scope_fixture();
    let results = search(&store, &options("authentication", ScopeIds::default()));
    assert_eq!(results.len(), 3);
}

#[test]
fn handles_adversarial_sql_metacharacters_in_scope_values() {
    let store = scope_fixture();

    let legit = search(&store, &options("authentication", scope(Some("s1"), None)));
    assert!(!legit.is_empty());

    let adversarial = search(
        &store,
        &options("authentication", scope(Some("' OR '1'='1"), None)),
    );
    assert!(adversarial.is_empty());
}

// ===== Redaction filtering =====

#[test]
fn excludes_redacted_observations_by_default() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    store
        .insert_observation(&observation(
            "obs-1",
            "authentication test",
            scope(Some("s1"), None),
            NOW,
        ))
        .unwrap();
    let mut redacted = observation(
        "obs-2",
        "authentication secret",
        scope(Some("s1"), None),
        NOW,
    );
    redacted.redacted = true;
    store.insert_observation(&redacted).unwrap();

    let results = search(&store, &options("authentication", ScopeIds::default()));

    assert_eq!(results.len(), 1);
    assert_eq!(entity_id(&results[0]), "obs-1");
}

#[test]
fn redacted_observations_are_not_in_fts_index() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    store
        .insert_observation(&observation(
            "obs-1",
            "authentication test",
            scope(Some("s1"), None),
            NOW,
        ))
        .unwrap();
    let mut redacted = observation(
        "obs-2",
        "authentication secret",
        scope(Some("s1"), None),
        NOW,
    );
    redacted.redacted = true;
    store.insert_observation(&redacted).unwrap();

    let mut opts = options("authentication", ScopeIds::default());
    opts.include_redacted = Some(true);
    let results = search(&store, &opts);

    // Redacted observations are never FTS-indexed, so only obs-1 is found.
    assert_eq!(results.len(), 1);
    assert_eq!(entity_id(&results[0]), "obs-1");
}

// ===== Exclusion (deduplication) =====

#[test]
fn excludes_specified_ids() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    store
        .insert_observation(&observation(
            "obs-1",
            "authentication test",
            scope(Some("s1"), None),
            NOW,
        ))
        .unwrap();
    store
        .insert_observation(&observation(
            "obs-2",
            "authentication flow",
            scope(Some("s1"), None),
            NOW,
        ))
        .unwrap();

    let mut opts = options("authentication", ScopeIds::default());
    opts.exclude_ids = Some(vec!["obs-1".to_string()]);
    let results = search(&store, &opts);

    assert_eq!(results.len(), 1);
    assert_eq!(entity_id(&results[0]), "obs-2");
}

// ===== Result limiting =====

#[test]
fn respects_max_results() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    for i in 0..10 {
        store
            .insert_observation(&observation(
                &format!("obs-{i}"),
                &format!("authentication test {i}"),
                scope(Some("s1"), None),
                NOW,
            ))
            .unwrap();
    }

    let mut opts = options("authentication", ScopeIds::default());
    opts.max_results = Some(5);
    let results = search(&store, &opts);

    assert_eq!(results.len(), 5);
}

#[test]
fn defaults_to_50_max_results() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    for i in 0..60 {
        store
            .insert_observation(&observation(
                &format!("obs-{i}"),
                &format!("authentication test {i}"),
                scope(Some("s1"), None),
                NOW,
            ))
            .unwrap();
    }

    let results = search(&store, &options("authentication", ScopeIds::default()));

    assert_eq!(results.len(), 50);
}

// ===== Scoring =====

#[test]
fn scores_are_between_0_and_1() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    store
        .insert_observation(&observation(
            "obs-1",
            "authentication bug fixed",
            scope(Some("s1"), None),
            NOW,
        ))
        .unwrap();

    let results = search(&store, &options("authentication", ScopeIds::default()));

    assert_eq!(results.len(), 1);
    assert!(results[0].score >= 0.0);
    assert!(results[0].score <= 1.0);
}

#[test]
fn ranks_better_matches_higher() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    store
        .insert_observation(&observation(
            "obs-1",
            "authentication authentication authentication",
            scope(Some("s1"), None),
            NOW,
        ))
        .unwrap();
    store
        .insert_observation(&observation(
            "obs-2",
            "authentication bug",
            scope(Some("s1"), None),
            NOW,
        ))
        .unwrap();

    let results = search(&store, &options("authentication", ScopeIds::default()));

    assert_eq!(results.len(), 2);
    assert_eq!(entity_id(&results[0]), "obs-1");
    assert!(results[0].score > results[1].score);
}

#[test]
fn weights_recent_observations_higher() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    store
        .insert_observation(&observation(
            "obs-recent",
            "authentication bug",
            scope(Some("s1"), None),
            NOW,
        ))
        .unwrap();
    store
        .insert_observation(&observation(
            "obs-old",
            "authentication bug",
            scope(Some("s1"), None),
            NOW - 30 * DAY_MS,
        ))
        .unwrap();

    let results = search(&store, &options("authentication", ScopeIds::default()));

    assert_eq!(results.len(), 2);
    assert_eq!(entity_id(&results[0]), "obs-recent");
    assert!(results[0].score > results[1].score);
}

#[test]
fn sorts_results_by_score_descending() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    store
        .insert_observation(&observation(
            "obs-1",
            "authentication",
            scope(Some("s1"), None),
            NOW,
        ))
        .unwrap();
    store
        .insert_observation(&observation(
            "obs-2",
            "authentication authentication",
            scope(Some("s1"), None),
            NOW - 10 * DAY_MS,
        ))
        .unwrap();
    store
        .insert_observation(&observation("obs-3", "auth", scope(Some("s1"), None), NOW))
        .unwrap();

    let results = search(&store, &options("authentication", ScopeIds::default()));

    for pair in results.windows(2) {
        assert!(pair[0].score >= pair[1].score);
    }
}

// ===== Match context =====

#[test]
fn provides_match_context_for_short_content() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    store
        .insert_observation(&observation(
            "obs-1",
            "authentication bug",
            scope(Some("s1"), None),
            NOW,
        ))
        .unwrap();

    let results = search(&store, &options("authentication", ScopeIds::default()));

    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].match_context.as_deref(),
        Some("authentication bug")
    );
}

#[test]
fn truncates_match_context_for_long_content() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    let long_content = format!("authentication {}", "x".repeat(200));
    store
        .insert_observation(&observation(
            "obs-1",
            &long_content,
            scope(Some("s1"), None),
            NOW,
        ))
        .unwrap();

    let results = search(&store, &options("authentication", ScopeIds::default()));

    assert_eq!(results.len(), 1);
    let context = results[0].match_context.as_deref().unwrap();
    assert_eq!(context.len(), 103); // 100 chars + '...'
    assert!(context.ends_with("..."));
}

#[test]
fn match_context_rounds_down_at_surrogate_pair_boundary() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    // 99 BMP units, then an astral char (2 UTF-16 units) straddling the
    // 100-unit boundary. JS substring(0, 100) would emit a lone surrogate,
    // unrepresentable in a Rust String; the documented policy (shared with
    // kindling-filter) rounds down and drops the whole pair.
    let content = format!(
        "authentication {}\u{1F525}{}",
        "x".repeat(84),
        "y".repeat(50)
    );
    store
        .insert_observation(&observation(
            "obs-1",
            &content,
            scope(Some("s1"), None),
            NOW,
        ))
        .unwrap();

    let results = search(&store, &options("authentication", ScopeIds::default()));

    assert_eq!(results.len(), 1);
    let context = results[0].match_context.as_deref().unwrap();
    assert_eq!(context, format!("authentication {}...", "x".repeat(84)));
    assert_eq!(context.encode_utf16().count(), 102); // 99 units + '...'
}

// ===== Determinism =====

#[test]
fn returns_same_results_for_same_query() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    store
        .insert_observation(&observation(
            "obs-1",
            "authentication bug fixed",
            scope(Some("s1"), None),
            NOW,
        ))
        .unwrap();

    let opts = options("authentication", scope(Some("s1"), None));
    let results1 = search(&store, &opts);
    let results2 = search(&store, &opts);

    assert_eq!(results1.len(), results2.len());
    assert_eq!(entity_id(&results1[0]), entity_id(&results2[0]));
    assert_eq!(results1[0].match_context, results2[0].match_context);
    assert_eq!(results1[0].score, results2[0].score);
}

// ===== Provider metadata =====

#[test]
fn has_correct_provider_name() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    let provider = LocalFtsProvider::from_store(&store);
    assert_eq!(provider.name(), "local-fts");
}

// ===== Malformed query handling =====

#[test]
fn malformed_queries_return_empty_results() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    store
        .insert_observation(&observation(
            "obs-1",
            "authentication test",
            scope(Some("s1"), None),
            NOW,
        ))
        .unwrap();

    for query in [
        "AND OR",
        "*",
        "",
        "foo(bar",
        "\"unclosed quote",
        "content:",
        "NOT NOT NOT",
    ] {
        let results = search(&store, &options(query, ScopeIds::default()));
        assert!(
            results.is_empty(),
            "query {query:?} should yield no results"
        );
    }
}
