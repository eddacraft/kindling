//! Integration tests for `kindling-client` against a real in-process daemon.

mod support;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use kindling_client::{
    Client, ClientConfig, ClientError, CloseCapsuleBody, CreatePinBody, Spawner,
};
use kindling_server::serve;
use kindling_types::{CapsuleType, ObservationInput, ObservationKind, PinTargetType, ScopeIds};
use serde_json::Map;
use support::{schema_version_u32, temp_server_config, TestDaemon};

const PROJECT_A: &str = "/tmp/kindling-client-test/project-a";
const PROJECT_B: &str = "/tmp/kindling-client-test/project-b";

fn message_input(content: &str) -> ObservationInput {
    ObservationInput {
        id: None,
        kind: ObservationKind::Message,
        content: content.to_string(),
        provenance: None,
        ts: None,
        scope_ids: ScopeIds {
            session_id: Some("s1".to_string()),
            ..Default::default()
        },
        redacted: None,
    }
}

/// 1. Warm-call round-trip exercising every method.
#[tokio::test]
async fn warm_call_round_trip_all_methods() {
    let daemon = TestDaemon::start().await;
    let client = daemon.client(PROJECT_A);

    // open_capsule
    let capsule = client
        .open_capsule(
            CapsuleType::Session,
            "round trip",
            ScopeIds {
                session_id: Some("s1".to_string()),
                ..Default::default()
            },
            None,
        )
        .await
        .expect("open capsule");
    assert_eq!(capsule.intent, "round trip");
    assert_eq!(capsule.kind, CapsuleType::Session);

    // append_observation (with project header + capsule attachment)
    let obs = client
        .append_observation(
            message_input("the quick brown fox jumps"),
            Some(capsule.id.clone()),
            None,
        )
        .await
        .expect("append observation");
    assert_eq!(obs.kind, ObservationKind::Message);
    assert_eq!(obs.content, "the quick brown fox jumps");

    // a second, unpinned observation to surface as a candidate
    let obs2 = client
        .append_observation(
            message_input("another brown fox sighting"),
            Some(capsule.id.clone()),
            None,
        )
        .await
        .expect("append observation 2");

    // pin
    let pin = client
        .pin(CreatePinBody {
            target_type: PinTargetType::Observation,
            target_id: obs.id.clone(),
            note: Some("important".to_string()),
            ttl_ms: None,
            scope_ids: Some(ScopeIds {
                session_id: Some("s1".to_string()),
                ..Default::default()
            }),
        })
        .await
        .expect("create pin");
    assert_eq!(pin.target_id, obs.id);

    // retrieve — the unpinned observation surfaces as a candidate, the pinned
    // one under pins
    let result = client
        .retrieve(kindling_types::RetrieveOptions {
            query: "brown fox".to_string(),
            scope_ids: ScopeIds {
                session_id: Some("s1".to_string()),
                ..Default::default()
            },
            token_budget: None,
            max_candidates: None,
            include_redacted: None,
        })
        .await
        .expect("retrieve");
    assert!(
        result
            .pins
            .iter()
            .any(|p| p.pin.id == pin.id && p.target_id() == obs.id),
        "pinned observation should surface in retrieval pins: {result:#?}"
    );
    assert!(
        result.candidates.iter().any(|c| c.entity_id() == obs2.id),
        "unpinned observation should surface as a candidate: {result:#?}"
    );

    // close_capsule
    let closed = client
        .close_capsule(&capsule.id, CloseCapsuleBody::default())
        .await
        .expect("close capsule");
    assert_eq!(closed.status, kindling_types::CapsuleStatus::Closed);

    // unpin
    client.unpin(&pin.id).await.expect("unpin");
}

/// `forget` redacts an observation so it no longer surfaces in retrieval, and a
/// missing id maps to an `Api { status: 404 }`.
#[tokio::test]
async fn forget_redacts_observation() {
    let daemon = TestDaemon::start().await;
    let client = daemon.client(PROJECT_A);

    let obs = client
        .append_observation(message_input("forgettable client needle"), None, None)
        .await
        .expect("append observation");

    // Surfaces before forgetting.
    let before = client
        .retrieve(kindling_types::RetrieveOptions {
            query: "needle".to_string(),
            scope_ids: ScopeIds {
                session_id: Some("s1".to_string()),
                ..Default::default()
            },
            token_budget: None,
            max_candidates: None,
            include_redacted: None,
        })
        .await
        .expect("retrieve before forget");
    assert!(
        before.candidates.iter().any(|c| c.entity_id() == obs.id),
        "observation should surface before forget: {before:#?}"
    );

    // Forget it.
    client.forget(&obs.id).await.expect("forget");

    // No longer surfaces.
    let after = client
        .retrieve(kindling_types::RetrieveOptions {
            query: "needle".to_string(),
            scope_ids: ScopeIds {
                session_id: Some("s1".to_string()),
                ..Default::default()
            },
            token_budget: None,
            max_candidates: None,
            include_redacted: None,
        })
        .await
        .expect("retrieve after forget");
    assert!(
        !after.candidates.iter().any(|c| c.entity_id() == obs.id),
        "redacted observation must not surface after forget: {after:#?}"
    );

    // A missing observation id → Api 404.
    let err = client
        .forget("does-not-exist")
        .await
        .expect_err("forget missing → error");
    match err {
        ClientError::Api { status, message } => {
            assert_eq!(status, 404);
            assert!(!message.is_empty(), "404 message should be surfaced");
        }
        other => panic!("expected Api 404, got {other:?}"),
    }
}

/// 2a. health() returns the daemon's schema version.
#[tokio::test]
async fn health_reports_schema_version() {
    let daemon = TestDaemon::start().await;
    let client = daemon.client(PROJECT_A);

    let health = client.health().await.expect("health");
    assert_eq!(health.schema_version, schema_version_u32());
    assert!(!health.version.is_empty());
}

/// 2b. A wrong expected schema version yields SchemaMismatch.
#[tokio::test]
async fn health_schema_mismatch() {
    let daemon = TestDaemon::start().await;
    let actual = schema_version_u32();
    let wrong = actual + 999;
    let client = daemon.client_with_schema(PROJECT_A, wrong);

    let err = client.health().await.expect_err("should mismatch");
    match err {
        ClientError::SchemaMismatch {
            expected,
            actual: got,
        } => {
            assert_eq!(expected, wrong);
            assert_eq!(got, actual);
        }
        other => panic!("expected SchemaMismatch, got {other:?}"),
    }
}

/// 3. Cold-spawn: socket does not exist initially; the spawner starts the
/// daemon on demand; the first call triggers spawn → poll → connect. Measures
/// cold-spawn latency.
#[tokio::test]
async fn cold_spawn_starts_daemon() {
    // Build a server config whose socket does NOT exist yet.
    let (config, _home, socket_path) = temp_server_config();
    // Hold the temp dir alive for the duration of the test.

    // The spawner starts the in-process daemon exactly when invoked.
    let spawned = Arc::new(AtomicBool::new(false));
    let spawned_flag = Arc::clone(&spawned);
    let config_for_spawn = config.clone();
    let spawner = Spawner::custom(move || {
        spawned_flag.store(true, Ordering::SeqCst);
        let cfg = config_for_spawn.clone();
        tokio::spawn(async move { serve(cfg).await });
        Ok(())
    });

    let client = Client::with_config(ClientConfig {
        socket_path: socket_path.clone(),
        project_root: PROJECT_A.to_string(),
        expected_schema_version: schema_version_u32(),
        connect_timeout: Duration::from_secs(1),
        poll_interval: Duration::from_millis(5),
        spawn: spawner,
    });

    assert!(!socket_path.exists(), "socket must not pre-exist");

    let start = Instant::now();
    // First call must trigger spawn, poll, connect, and succeed.
    let health = client.health().await.expect("cold-spawn health");
    let elapsed = start.elapsed();

    assert!(spawned.load(Ordering::SeqCst), "spawner should have run");
    assert_eq!(health.schema_version, schema_version_u32());
    // Generous bound; APS target is <100ms on a dev machine.
    assert!(
        elapsed < Duration::from_secs(1),
        "cold-spawn latency {elapsed:?} exceeded 1s"
    );
    eprintln!("cold-spawn latency: {elapsed:?}");

    // A follow-up data call should also succeed against the now-running daemon.
    let _ = client
        .open_capsule(
            CapsuleType::Session,
            "after cold spawn",
            ScopeIds::default(),
            None,
        )
        .await
        .expect("post-spawn open capsule");
}

/// 4. Connection refused without a binary: a nonexistent socket plus a spawner
/// that fails → ClientError::Unavailable within the budget, never a hang.
#[tokio::test]
async fn refused_without_binary_is_unavailable() {
    let dir = tempfile::tempdir().unwrap();
    let socket_path: PathBuf = dir.path().join("nonexistent.sock");

    let client = Client::with_config(ClientConfig {
        socket_path: socket_path.clone(),
        project_root: PROJECT_A.to_string(),
        expected_schema_version: schema_version_u32(),
        connect_timeout: Duration::from_millis(200),
        poll_interval: Duration::from_millis(10),
        // Spawner that fails like a missing binary would (ENOENT on `kindling`).
        spawn: Spawner::custom(|| {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "kindling binary not found",
            ))
        }),
    });

    // Wrap in a timeout so a hang fails the test rather than blocking forever.
    let result = tokio::time::timeout(Duration::from_secs(3), client.health()).await;
    let outcome = result.expect("call must not hang");
    match outcome {
        Err(ClientError::Unavailable(msg)) => {
            assert!(
                msg.contains("spawn") || msg.contains("not found"),
                "message should explain the failure: {msg}"
            );
        }
        other => panic!("expected Unavailable, got {other:?}"),
    }
}

/// 4b. Default `Spawner::Command` execs the (absent in CI) `kindling` binary;
/// the call must still fail cleanly with Unavailable, not panic or hang.
#[tokio::test]
async fn default_spawner_missing_binary_is_unavailable() {
    let dir = tempfile::tempdir().unwrap();
    let socket_path: PathBuf = dir.path().join("nonexistent.sock");

    let client = Client::with_config(ClientConfig {
        socket_path,
        project_root: PROJECT_A.to_string(),
        expected_schema_version: schema_version_u32(),
        connect_timeout: Duration::from_millis(200),
        poll_interval: Duration::from_millis(10),
        spawn: Spawner::Command, // execs `kindling`, not on PATH in CI
    });

    let result = tokio::time::timeout(Duration::from_secs(3), client.health()).await;
    let outcome = result.expect("call must not hang");
    assert!(
        matches!(outcome, Err(ClientError::Unavailable(_))),
        "expected Unavailable, got {outcome:?}"
    );
}

/// 5. API error mapping: closing a nonexistent capsule → Api { status: 404 }.
#[tokio::test]
async fn api_error_mapping_404() {
    let daemon = TestDaemon::start().await;
    let client = daemon.client(PROJECT_A);

    let err = client
        .close_capsule("does-not-exist", CloseCapsuleBody::default())
        .await
        .expect_err("should be 404");
    match err {
        ClientError::Api { status, message } => {
            assert_eq!(status, 404);
            assert!(!message.is_empty(), "404 message should be surfaced");
        }
        other => panic!("expected Api 404, got {other:?}"),
    }
}

/// `get_open_capsule` returns `None` before a capsule is open, then resolves
/// the open session capsule once one exists.
#[tokio::test]
async fn get_open_capsule_resolves_session_capsule() {
    let daemon = TestDaemon::start().await;
    let client = daemon.client(PROJECT_A);

    // No open capsule yet → None.
    let none = client
        .get_open_capsule("sess-x")
        .await
        .expect("get_open_capsule before open");
    assert!(none.is_none(), "no capsule open yet → None");

    // Open a session capsule.
    let opened = client
        .open_capsule(
            CapsuleType::Session,
            "resolve me",
            ScopeIds {
                session_id: Some("sess-x".to_string()),
                ..Default::default()
            },
            None,
        )
        .await
        .expect("open capsule");

    // Now it resolves.
    let found = client
        .get_open_capsule("sess-x")
        .await
        .expect("get_open_capsule after open")
        .expect("capsule should be open");
    assert_eq!(found.id, opened.id);
    assert_eq!(found.scope_ids.session_id.as_deref(), Some("sess-x"));

    // A different session id resolves to None.
    let other = client
        .get_open_capsule("nope")
        .await
        .expect("get_open_capsule other session");
    assert!(other.is_none(), "different session → None");
}

/// 5b. A 400 (empty intent) also maps to Api with a surfaced message.
#[tokio::test]
async fn api_error_mapping_400() {
    let daemon = TestDaemon::start().await;
    let client = daemon.client(PROJECT_A);

    let err = client
        .open_capsule(CapsuleType::PocketflowNode, "", ScopeIds::default(), None)
        .await
        .expect_err("empty intent should be 400");
    match err {
        ClientError::Api { status, message } => {
            assert_eq!(status, 400);
            assert!(!message.is_empty());
        }
        other => panic!("expected Api 400, got {other:?}"),
    }
}

/// 6. Per-project isolation via two clients on the same daemon.
#[tokio::test]
async fn per_project_isolation_via_client() {
    let daemon = TestDaemon::start().await;
    let client_a = daemon.client(PROJECT_A);
    let client_b = daemon.client(PROJECT_B);

    let mut input = message_input("secret alpha content");
    input.scope_ids = ScopeIds::default();
    client_a
        .append_observation(input, None, None)
        .await
        .expect("write under A");

    // Project B must not see A's data.
    let res_b = client_b
        .retrieve(kindling_types::RetrieveOptions {
            query: "alpha".to_string(),
            scope_ids: ScopeIds::default(),
            token_budget: None,
            max_candidates: None,
            include_redacted: None,
        })
        .await
        .expect("retrieve under B");
    assert!(
        res_b.candidates.is_empty(),
        "project B must not see project A's data: {:#?}",
        res_b.candidates
    );

    // Project A sees its own data.
    let res_a = client_a
        .retrieve(kindling_types::RetrieveOptions {
            query: "alpha".to_string(),
            scope_ids: ScopeIds::default(),
            token_budget: None,
            max_candidates: None,
            include_redacted: None,
        })
        .await
        .expect("retrieve under A");
    assert!(
        !res_a.candidates.is_empty(),
        "project A should see its own data"
    );
}

/// Sanity: provenance survives the round trip (object metadata preserved).
#[tokio::test]
async fn observation_provenance_round_trips() {
    let daemon = TestDaemon::start().await;
    let client = daemon.client(PROJECT_A);

    let mut prov = Map::new();
    prov.insert("tool".to_string(), serde_json::json!("bash"));
    let input = ObservationInput {
        id: None,
        kind: ObservationKind::ToolCall,
        content: "ls -la".to_string(),
        provenance: Some(prov),
        ts: None,
        scope_ids: ScopeIds::default(),
        redacted: None,
    };
    let obs = client
        .append_observation(input, None, None)
        .await
        .expect("append");
    assert_eq!(
        obs.provenance.get("tool").unwrap(),
        &serde_json::json!("bash")
    );
}

/// SessionStart context: writes scoped observations + a pin, then asserts the
/// daemon returns formatted markdown. The client derives the repo scope from its
/// project root, so observations must carry `repoId == PROJECT_A`.
#[tokio::test]
async fn session_start_context_returns_markdown() {
    let daemon = TestDaemon::start().await;
    let client = daemon.client(PROJECT_A);

    let repo_scope = ScopeIds {
        repo_id: Some(PROJECT_A.to_string()),
        ..Default::default()
    };
    let scoped = |content: &str| ObservationInput {
        id: None,
        kind: ObservationKind::Message,
        content: content.to_string(),
        provenance: None,
        ts: None,
        scope_ids: repo_scope.clone(),
        redacted: None,
    };

    let obs = client
        .append_observation(scoped("investigated the parser bug"), None, None)
        .await
        .expect("append");
    client
        .pin(CreatePinBody {
            target_type: PinTargetType::Observation,
            target_id: obs.id.clone(),
            note: Some("parser".to_string()),
            ttl_ms: None,
            scope_ids: Some(repo_scope.clone()),
        })
        .await
        .expect("pin");

    let ctx = client
        .session_start_context(Some(5))
        .await
        .expect("session start context")
        .expect("some context");

    assert!(ctx.starts_with("# Prior Context (from Kindling)"), "{ctx}");
    assert!(ctx.contains("## Pinned Items"), "{ctx}");
    assert!(
        ctx.contains("- **parser**: investigated the parser bug"),
        "{ctx}"
    );
    assert!(ctx.contains("## Recent Activity"), "{ctx}");
    assert!(
        ctx.contains("message: investigated the parser bug"),
        "{ctx}"
    );
}

/// SessionStart context with no data → `None`.
#[tokio::test]
async fn session_start_context_none_when_empty() {
    let daemon = TestDaemon::start().await;
    // A fresh project with nothing written.
    let client = daemon.client("/tmp/kindling-client-test/empty-ss");
    let ctx = client.session_start_context(None).await.expect("call ok");
    assert!(ctx.is_none(), "expected None, got {ctx:?}");
}

/// PreCompact context: a closed capsule with a summary + a pin yields markdown.
#[tokio::test]
async fn pre_compact_context_returns_markdown() {
    let daemon = TestDaemon::start().await;
    let client = daemon.client(PROJECT_B);

    let repo_scope = ScopeIds {
        repo_id: Some(PROJECT_B.to_string()),
        ..Default::default()
    };

    // Open a repo-scoped capsule, close with a summary.
    let capsule = client
        .open_capsule(
            CapsuleType::PocketflowNode,
            "do work",
            repo_scope.clone(),
            None,
        )
        .await
        .expect("open");
    client
        .close_capsule(
            &capsule.id,
            CloseCapsuleBody {
                generate_summary: Some(true),
                summary_content: Some("delivered the change".to_string()),
                confidence: Some(0.9),
            },
        )
        .await
        .expect("close");

    let ctx = client
        .pre_compact_context()
        .await
        .expect("pre compact context")
        .expect("some context");

    // No top-level Prior Context header on PreCompact.
    assert!(!ctx.contains("# Prior Context"), "{ctx}");
    assert!(ctx.contains("## Session Summary"), "{ctx}");
    assert!(ctx.contains("delivered the change"), "{ctx}");
}

/// PreCompact context with no data → `None`.
#[tokio::test]
async fn pre_compact_context_none_when_empty() {
    let daemon = TestDaemon::start().await;
    let client = daemon.client("/tmp/kindling-client-test/empty-pc");
    let ctx = client.pre_compact_context().await.expect("call ok");
    assert!(ctx.is_none(), "expected None, got {ctx:?}");
}

// ---- small helpers on the retrieval result for terse assertions -------------

trait PinResultExt {
    fn target_id(&self) -> kindling_types::Id;
}
impl PinResultExt for kindling_types::PinResult {
    fn target_id(&self) -> kindling_types::Id {
        entity_id(&self.target)
    }
}

trait CandidateExt {
    fn entity_id(&self) -> kindling_types::Id;
}
impl CandidateExt for kindling_types::CandidateResult {
    fn entity_id(&self) -> kindling_types::Id {
        entity_id(&self.entity)
    }
}

fn entity_id(e: &kindling_types::RetrievedEntity) -> kindling_types::Id {
    match e {
        kindling_types::RetrievedEntity::Observation(o) => o.id.clone(),
        kindling_types::RetrievedEntity::Summary(s) => s.id.clone(),
    }
}
