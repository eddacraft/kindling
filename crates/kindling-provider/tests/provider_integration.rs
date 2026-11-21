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

/// The returned entity ids, sorted, for membership assertions (not just counts).
fn sorted_ids(results: &[ProviderSearchResult]) -> Vec<&str> {
    let mut ids: Vec<&str> = results.iter().map(entity_id).collect();
    ids.sort_unstable();
    ids
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
    // s1 matches obs-1 (/repo1) and obs-3 (/repo2), never obs-2 (s2).
    assert_eq!(sorted_ids(&results), ["obs-1", "obs-3"]);
}

#[test]
fn filters_by_repo_id() {
    let store = scope_fixture();
    let results = search(
        &store,
        &options("authentication", scope(None, Some("/repo1"))),
    );
    // /repo1 matches obs-1 (s1) and obs-2 (s2), never obs-3 (/repo2).
    assert_eq!(sorted_ids(&results), ["obs-1", "obs-2"]);
}

#[test]
fn filters_by_multiple_scope_dimensions_with_and_semantics() {
    let store = scope_fixture();
    let results = search(
        &store,
        &options("authentication", scope(Some("s1"), Some("/repo1"))),
    );
    // Only obs-1 carries BOTH s1 and /repo1.
    assert_eq!(sorted_ids(&results), ["obs-1"]);
}

#[test]
fn returns_all_results_when_no_scope_specified() {
    let store = scope_fixture();
    let results = search(&store, &options("authentication", ScopeIds::default()));
    assert_eq!(sorted_ids(&results), ["obs-1", "obs-2", "obs-3"]);
}

#[test]
fn handles_adversarial_sql_metacharacters_in_scope_values() {
    let store = scope_fixture();

    let legit = search(&store, &options("authentication", scope(Some("s1"), None)));
    assert_eq!(sorted_ids(&legit), ["obs-1", "obs-3"]);

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
        // FTS5 column-filter syntax against a column that is not in the index.
        "missing:term",
        "col1:foo",
        "hello missing:term",
    ] {
        let results = search(&store, &options(query, ScopeIds::default()));
        assert!(
            results.is_empty(),
            "query {query:?} should yield no results"
        );
    }
}

#[test]
fn genuine_schema_errors_are_not_swallowed() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    store
        .insert_observation(&observation(
            "obs-1",
            "authentication test",
            scope(Some("s1"), None),
            NOW,
        ))
        .unwrap();

    // Drop the denormalised column the observations query selects so SQLite
    // reports `no such column: o.redacted` — the same error class as an
    // unknown FTS column filter, but a real schema failure.
    store
        .connection()
        .execute_batch(
            "DROP TRIGGER IF EXISTS observations_fts_insert;
             DROP TRIGGER IF EXISTS observations_fts_update;
             DROP TRIGGER IF EXISTS observations_fts_delete;
             ALTER TABLE observations DROP COLUMN redacted;",
        )
        .unwrap();

    let err = LocalFtsProvider::from_store(&store)
        .search(&options("authentication", ScopeIds::default()), NOW)
        .expect_err("a missing observations column must surface as a provider error");
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("no such column"),
        "expected a schema no-such-column error, got {err}"
    );
}

#[test]
fn genuine_backend_errors_are_not_swallowed() {
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
        .connection()
        .execute_batch("DROP TABLE observations_fts;")
        .unwrap();

    let err = LocalFtsProvider::from_store(&store)
        .search(&options("authentication", ScopeIds::default()), NOW)
        .expect_err("a missing FTS table must surface as a provider error");
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("no such table"),
        "expected a backend no-such-table error, got {err}"
    );
}

// ===== Exact composite ranking =====
//
// `max_results` must bound the output of the documented composite score, not
// a BM25 shortlist taken before scoring. These tests pin three properties:
// a better composite candidate is never dropped, scores do not depend on the
// limit, and every limited result is a prefix of the unlimited ranking.

/// Non-matching rows so the query term is rare enough for BM25 to have a
/// positive IDF (FTS5 floors IDF when most documents contain the term).
fn insert_background(store: &SqliteKindlingStore, count: usize) {
    for i in 0..count {
        store
            .insert_observation(&observation(
                &format!("bg-{i}"),
                "unrelated background content about deployment",
                scope(Some("s1"), None),
                NOW - 40 * DAY_MS,
            ))
            .unwrap();
    }
}

/// Strong-but-old, medium-but-old, and weak-but-recent matches for "needle".
fn shortlist_fixture() -> SqliteKindlingStore {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    insert_background(&store, 6);
    store
        .insert_observation(&observation(
            "obs-strong-old",
            "needle needle needle needle",
            scope(Some("s1"), None),
            NOW - 29 * DAY_MS,
        ))
        .unwrap();
    store
        .insert_observation(&observation(
            "obs-medium-old",
            "a needle in a long haystack of words",
            scope(Some("s1"), None),
            NOW - 29 * DAY_MS,
        ))
        .unwrap();
    store
        .insert_observation(&observation(
            "obs-weak-recent",
            "one needle buried among many many other unrelated haystack words here",
            scope(Some("s1"), None),
            NOW,
        ))
        .unwrap();
    store
}

fn with_limit(query: &str, limit: Option<u32>) -> ProviderSearchOptions {
    ProviderSearchOptions {
        max_results: limit,
        ..options(query, ScopeIds::default())
    }
}

fn ids(results: &[ProviderSearchResult]) -> Vec<&str> {
    results.iter().map(entity_id).collect()
}

#[test]
fn recent_candidate_outside_bm25_shortlist_is_not_dropped() {
    let store = shortlist_fixture();

    // Preconditions: BM25 alone ranks strong > medium > weak, but the composite
    // score puts the recent weak match above the old medium one.
    let full = search(&store, &with_limit("needle", None));
    assert_eq!(
        ids(&full),
        vec!["obs-strong-old", "obs-weak-recent", "obs-medium-old"],
        "fixture must rank the recent weak match above the old medium match"
    );

    let limited = search(&store, &with_limit("needle", Some(2)));
    assert_eq!(ids(&limited), vec!["obs-strong-old", "obs-weak-recent"]);
}

#[test]
fn scores_do_not_depend_on_max_results() {
    let store = shortlist_fixture();
    let full = search(&store, &with_limit("needle", None));

    for limit in 1..=full.len() as u32 {
        let limited = search(&store, &with_limit("needle", Some(limit)));
        for result in &limited {
            let expected = full
                .iter()
                .find(|f| entity_id(f) == entity_id(result))
                .expect("limited result must appear in the full ranking");
            assert_eq!(
                result.score,
                expected.score,
                "score of {} changed at max_results={limit}",
                entity_id(result)
            );
        }
    }
}

/// Observations and summaries with relevance and recency pulling in opposite
/// directions, so the full ranking interleaves the two types.
fn mixed_fixture() -> SqliteKindlingStore {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    insert_background(&store, 6);
    let observations = [
        ("obs-a", "needle needle needle", 25),
        ("obs-b", "needle needle and some more text", 1),
        (
            "obs-c",
            "a needle with plenty of surrounding filler text words",
            0,
        ),
        ("obs-d", "needle needle needle needle needle", 29),
        ("obs-e", "the needle", 15),
    ];
    for (id, content, age_days) in observations {
        store
            .insert_observation(&observation(
                id,
                content,
                scope(Some("s1"), None),
                NOW - age_days * DAY_MS,
            ))
            .unwrap();
    }
    let summaries = [
        ("sum-a", "cap-a", "needle needle summary", 20),
        (
            "sum-b",
            "cap-b",
            "summary mentioning a needle once among other words",
            0,
        ),
        ("sum-c", "cap-c", "needle", 28),
        ("sum-d", "cap-d", "unrelated summary text", 0),
        ("sum-e", "cap-e", "deployment notes for the release", 10),
        ("sum-f", "cap-f", "refactored the storage layer", 12),
        ("sum-g", "cap-g", "investigated flaky daemon startup", 14),
    ];
    for (id, capsule_id, content, age_days) in summaries {
        store
            .create_capsule(&capsule(capsule_id, scope(Some("s1"), None)))
            .unwrap();
        store
            .insert_summary(&summary(id, capsule_id, content, NOW - age_days * DAY_MS))
            .unwrap();
    }
    store
}

#[test]
fn limited_results_are_a_prefix_of_the_full_ranking_across_types() {
    let store = mixed_fixture();
    let full = search(&store, &with_limit("needle", None));
    let full_ids = ids(&full);

    // Precondition: the ranking really interleaves the two entity types.
    let kinds: Vec<bool> = full
        .iter()
        .map(|r| matches!(r.entity, RetrievedEntity::Summary(_)))
        .collect();
    assert!(
        kinds.windows(2).any(|w| w[0] != w[1])
            && kinds.windows(3).any(|w| w[0] == w[2] && w[0] != w[1]),
        "fixture must interleave observations and summaries: {full_ids:?}"
    );

    for limit in 1..=full.len() as u32 {
        let limited = search(&store, &with_limit("needle", Some(limit)));
        assert_eq!(
            ids(&limited),
            full_ids[..limit as usize].to_vec(),
            "max_results={limit} is not a prefix of the full ranking"
        );
    }
}

#[test]
fn equal_scores_are_ordered_by_id() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    insert_background(&store, 6);
    // Inserted in reverse id order: rowid order must not decide ties.
    for id in ["obs-c", "obs-b", "obs-a"] {
        store
            .insert_observation(&observation(
                id,
                "identical needle content",
                scope(Some("s1"), None),
                NOW,
            ))
            .unwrap();
    }

    let results = search(&store, &with_limit("needle", None));
    assert_eq!(ids(&results), vec!["obs-a", "obs-b", "obs-c"]);
    assert!(results.windows(2).all(|w| w[0].score == w[1].score));
}

#[test]
fn retrieve_returns_top_composite_candidates_within_max_candidates() {
    let store = shortlist_fixture();
    let provider = LocalFtsProvider::from_store(&store);
    let options = kindling_types::RetrieveOptions {
        query: "needle".to_string(),
        scope_ids: ScopeIds::default(),
        token_budget: None,
        max_candidates: Some(2),
        include_redacted: None,
    };

    let result = kindling_provider::retrieve_at(&store, &provider, &options, NOW).unwrap();
    let candidate_ids: Vec<&str> = result
        .candidates
        .iter()
        .map(|c| match &c.entity {
            RetrievedEntity::Observation(obs) => obs.id.as_str(),
            RetrievedEntity::Summary(sum) => sum.id.as_str(),
        })
        .collect();
    assert_eq!(candidate_ids, vec!["obs-strong-old", "obs-weak-recent"]);
}

#[test]
fn ranking_does_not_depend_on_the_query_plan() {
    // ANALYZE gives the planner statistics, which can change the join order
    // (for example, driving summaries from a selective capsule scope index).
    // The ranking is computed in Rust, so it must not change.
    let scoped = ScopeIds {
        session_id: Some("s1".to_string()),
        ..ScopeIds::default()
    };
    let store = mixed_fixture();
    let before = search(&store, &options("needle", scoped.clone()));
    store.connection().execute_batch("ANALYZE").unwrap();
    let after = search(&store, &options("needle", scoped));

    assert!(!before.is_empty());
    assert_eq!(ids(&after), ids(&before));
    let scores = |r: &[ProviderSearchResult]| r.iter().map(|x| x.score).collect::<Vec<_>>();
    assert_eq!(scores(&after), scores(&before));
}

#[test]
fn zero_max_results_returns_nothing() {
    let store = shortlist_fixture();
    assert!(search(&store, &with_limit("needle", Some(0))).is_empty());
}

#[test]
fn search_inside_a_caller_transaction_sees_its_uncommitted_rows() {
    let store = shortlist_fixture();
    let tx = store.connection().unchecked_transaction().unwrap();
    store
        .insert_observation(&observation(
            "obs-in-tx",
            "needle needle needle needle needle",
            scope(Some("s1"), None),
            NOW,
        ))
        .unwrap();

    let results = search(&store, &with_limit("needle", Some(1)));
    assert_eq!(ids(&results), vec!["obs-in-tx"]);
    drop(tx); // rolls back
    assert_ne!(
        ids(&search(&store, &with_limit("needle", Some(1)))),
        vec!["obs-in-tx"]
    );
}

/// GH-216: a TS-era row may hold an array-shaped `scope_ids` blob (with NULL
/// denormalized columns). Search must still return it, as unscoped.
#[test]
fn ts_era_array_scope_ids_row_is_searchable_as_unscoped() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    store
        .insert_observation(&observation(
            "obs-1",
            "legacy migration note",
            scope(Some("s1"), None),
            NOW,
        ))
        .unwrap();
    store
        .connection()
        .execute(
            r#"UPDATE observations SET scope_ids = '["s1"]', session_id = NULL"#,
            [],
        )
        .unwrap();

    let results = search(&store, &options("migration", ScopeIds::default()));
    assert_eq!(sorted_ids(&results), ["obs-1"]);
    match &results[0].entity {
        RetrievedEntity::Observation(obs) => assert_eq!(obs.scope_ids, ScopeIds::default()),
        other => panic!("expected an observation, got {other:?}"),
    }
}
