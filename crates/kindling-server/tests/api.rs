//! Integration tests for the v1 HTTP API over a Unix domain socket.

mod support;

use hyper::StatusCode;
use serde_json::json;
use support::TestDaemon;

const PROJECT_A: &str = "/tmp/kindling-test/project-a";
const PROJECT_B: &str = "/tmp/kindling-test/project-b";

#[tokio::test]
async fn full_round_trip_per_route() {
    let daemon = TestDaemon::start().await;
    let mut c = daemon.connect().await;

    // Open a capsule.
    let resp = c
        .send(
            "POST",
            "/v1/capsules",
            Some(PROJECT_A),
            Some(json!({
                "kind": "session",
                "intent": "round trip",
                "scopeIds": { "sessionId": "s1" }
            })),
        )
        .await;
    assert_eq!(resp.status, StatusCode::CREATED, "open capsule");
    let capsule = resp.json();
    let capsule_id = capsule["id"].as_str().unwrap().to_string();
    assert_eq!(capsule["status"], "open");
    assert_eq!(capsule["intent"], "round trip");

    // Append an observation attached to that capsule.
    let resp = c
        .send(
            "POST",
            "/v1/observations",
            Some(PROJECT_A),
            Some(json!({
                "kind": "message",
                "content": "the quick brown fox jumps",
                "scopeIds": { "sessionId": "s1" },
                "capsuleId": capsule_id,
            })),
        )
        .await;
    assert_eq!(resp.status, StatusCode::CREATED, "append observation");
    let observation = resp.json();
    let obs_id = observation["id"].as_str().unwrap().to_string();
    assert_eq!(observation["kind"], "message");
    assert_eq!(observation["content"], "the quick brown fox jumps");

    // Pin the observation.
    let resp = c
        .send(
            "POST",
            "/v1/pins",
            Some(PROJECT_A),
            Some(json!({
                "targetType": "observation",
                "targetId": obs_id,
                "note": "important",
                "scopeIds": { "sessionId": "s1" },
            })),
        )
        .await;
    assert_eq!(resp.status, StatusCode::CREATED, "create pin");
    let pin = resp.json();
    let pin_id = pin["id"].as_str().unwrap().to_string();
    assert_eq!(pin["targetId"], obs_id);

    // A second, unpinned observation to exercise the candidate path.
    let resp = c
        .send(
            "POST",
            "/v1/observations",
            Some(PROJECT_A),
            Some(json!({
                "kind": "message",
                "content": "another brown fox sighting",
                "scopeIds": { "sessionId": "s1" },
                "capsuleId": capsule_id,
            })),
        )
        .await;
    assert_eq!(resp.status, StatusCode::CREATED);
    let obs2_id = resp.json()["id"].as_str().unwrap().to_string();

    // Retrieve — the unpinned observation should surface as a candidate.
    let resp = c
        .send(
            "POST",
            "/v1/retrieve",
            Some(PROJECT_A),
            Some(json!({
                "query": "brown fox",
                "scopeIds": { "sessionId": "s1" }
            })),
        )
        .await;
    assert_eq!(resp.status, StatusCode::OK, "retrieve");
    let result = resp.json();
    // The pinned observation surfaces under `pins` (pins are non-evictable and
    // are excluded from `candidates` to avoid duplication).
    let pins = result["pins"].as_array().unwrap();
    assert!(
        pins.iter()
            .any(|p| p["pin"]["id"] == pin_id && p["target"]["id"] == obs_id),
        "pinned observation should surface in retrieval pins: {result:#}"
    );
    // The unpinned observation surfaces as a ranked candidate.
    let candidates = result["candidates"].as_array().unwrap();
    assert!(
        candidates.iter().any(|c| c["entity"]["id"] == obs2_id),
        "unpinned observation should surface in retrieval candidates: {result:#}"
    );

    // Close the capsule.
    let resp = c
        .send(
            "PATCH",
            &format!("/v1/capsules/{capsule_id}/close"),
            Some(PROJECT_A),
            Some(json!({})),
        )
        .await;
    assert_eq!(resp.status, StatusCode::OK, "close capsule");
    assert_eq!(resp.json()["status"], "closed");

    // Unpin.
    let resp = c
        .send(
            "DELETE",
            &format!("/v1/pins/{pin_id}"),
            Some(PROJECT_A),
            None,
        )
        .await;
    assert_eq!(resp.status, StatusCode::NO_CONTENT, "unpin");
}

#[tokio::test]
async fn forget_redacts_observation() {
    let daemon = TestDaemon::start().await;
    let mut c = daemon.connect().await;

    // Append an observation that retrieve can find.
    let resp = c
        .send(
            "POST",
            "/v1/observations",
            Some(PROJECT_A),
            Some(json!({
                "kind": "message",
                "content": "forgettable needle phrase",
                "scopeIds": { "sessionId": "fs" },
            })),
        )
        .await;
    assert_eq!(resp.status, StatusCode::CREATED, "append observation");
    let obs_id = resp.json()["id"].as_str().unwrap().to_string();

    // It surfaces in retrieval before forgetting.
    let resp = c
        .send(
            "POST",
            "/v1/retrieve",
            Some(PROJECT_A),
            Some(json!({ "query": "needle", "scopeIds": { "sessionId": "fs" } })),
        )
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    let before = resp.json()["candidates"].as_array().unwrap().clone();
    assert!(
        before.iter().any(|c| c["entity"]["id"] == obs_id),
        "observation should surface before forget: {before:#?}"
    );

    // Forget it → 204 No Content.
    let resp = c
        .send(
            "POST",
            &format!("/v1/observations/{obs_id}/forget"),
            Some(PROJECT_A),
            None,
        )
        .await;
    assert_eq!(resp.status, StatusCode::NO_CONTENT, "forget → 204");

    // It no longer surfaces in retrieval (the redact trigger drops it from FTS).
    let resp = c
        .send(
            "POST",
            "/v1/retrieve",
            Some(PROJECT_A),
            Some(json!({ "query": "needle", "scopeIds": { "sessionId": "fs" } })),
        )
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    let after = resp.json()["candidates"].as_array().unwrap().clone();
    assert!(
        !after.iter().any(|c| c["entity"]["id"] == obs_id),
        "redacted observation must not surface in retrieval: {after:#?}"
    );

    // Forgetting an unknown observation → 404.
    let resp = c
        .send(
            "POST",
            "/v1/observations/does-not-exist/forget",
            Some(PROJECT_A),
            None,
        )
        .await;
    assert_eq!(
        resp.status,
        StatusCode::NOT_FOUND,
        "forget missing observation → 404"
    );

    // Missing project header → 400.
    let resp = c
        .send(
            "POST",
            &format!("/v1/observations/{obs_id}/forget"),
            None,
            None,
        )
        .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST, "no project → 400");
}

#[tokio::test]
async fn health_reports_version_and_schema() {
    let daemon = TestDaemon::start().await;
    let mut c = daemon.connect().await;

    // Before any project is touched.
    let resp = c.send("GET", "/v1/health", None, None).await;
    assert_eq!(resp.status, StatusCode::OK);
    let body = resp.json();
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(
        body["schemaVersion"],
        kindling_store::schema_version().version
    );
    assert_capability_block(&body);
    assert_eq!(body["projects"].as_array().unwrap().len(), 0);

    // Touch a project.
    let _ = c
        .send(
            "POST",
            "/v1/capsules",
            Some(PROJECT_A),
            Some(json!({
                "kind": "pocketflow_node",
                "intent": "touch",
                "scopeIds": {}
            })),
        )
        .await;

    let resp = c.send("GET", "/v1/health", None, None).await;
    let body = resp.json();
    let projects = body["projects"].as_array().unwrap();
    let expected = kindling_store::project_id(PROJECT_A);
    assert!(
        projects.iter().any(|p| p == &expected),
        "touched project id should appear in /v1/health: {body:#}"
    );
}

#[tokio::test]
async fn per_project_isolation() {
    let daemon = TestDaemon::start().await;
    let mut c = daemon.connect().await;

    // Write an observation under project A.
    let resp = c
        .send(
            "POST",
            "/v1/observations",
            Some(PROJECT_A),
            Some(json!({
                "kind": "message",
                "content": "secret alpha content",
                "scopeIds": {}
            })),
        )
        .await;
    assert_eq!(resp.status, StatusCode::CREATED);

    // Retrieve under project B — must not see it.
    let resp = c
        .send(
            "POST",
            "/v1/retrieve",
            Some(PROJECT_B),
            Some(json!({ "query": "alpha", "scopeIds": {} })),
        )
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    let candidates = resp.json()["candidates"].as_array().unwrap().clone();
    assert!(
        candidates.is_empty(),
        "project B must not see project A's data: {candidates:#?}"
    );

    // Retrieve under project A — should see it.
    let resp = c
        .send(
            "POST",
            "/v1/retrieve",
            Some(PROJECT_A),
            Some(json!({ "query": "alpha", "scopeIds": {} })),
        )
        .await;
    let candidates = resp.json()["candidates"].as_array().unwrap().clone();
    assert!(!candidates.is_empty(), "project A should see its own data");
}

#[tokio::test]
async fn missing_project_header_is_bad_request() {
    let daemon = TestDaemon::start().await;
    let mut c = daemon.connect().await;

    let resp = c
        .send(
            "POST",
            "/v1/observations",
            None, // no X-Kindling-Project
            Some(json!({
                "kind": "message",
                "content": "x",
                "scopeIds": {}
            })),
        )
        .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST);
    assert!(resp.json()["error"].as_str().unwrap().contains("project"));

    // Empty header value also fails.
    let resp = c
        .send(
            "POST",
            "/v1/observations",
            Some("   "),
            Some(json!({ "kind": "message", "content": "x", "scopeIds": {} })),
        )
        .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn concurrent_writes_same_project_all_land() {
    let daemon = TestDaemon::start().await;

    // Two independent connections writing to the SAME project concurrently.
    let socket = daemon.socket_path.clone();
    let socket2 = socket.clone();

    let h1 = tokio::spawn(async move {
        let mut c = support::Client::connect(&socket).await;
        c.send(
            "POST",
            "/v1/observations",
            Some(PROJECT_A),
            Some(json!({ "kind": "message", "content": "concurrent one", "scopeIds": {} })),
        )
        .await
    });
    let h2 = tokio::spawn(async move {
        let mut c = support::Client::connect(&socket2).await;
        c.send(
            "POST",
            "/v1/observations",
            Some(PROJECT_A),
            Some(json!({ "kind": "message", "content": "concurrent two", "scopeIds": {} })),
        )
        .await
    });

    let r1 = h1.await.unwrap();
    let r2 = h2.await.unwrap();
    assert_eq!(r1.status, StatusCode::CREATED, "first concurrent write");
    assert_eq!(r2.status, StatusCode::CREATED, "second concurrent write");

    // Both rows must be retrievable.
    let mut c = daemon.connect().await;
    let resp = c
        .send(
            "POST",
            "/v1/retrieve",
            Some(PROJECT_A),
            Some(json!({ "query": "concurrent", "scopeIds": {}, "maxCandidates": 10 })),
        )
        .await;
    let candidates = resp.json()["candidates"].as_array().unwrap().clone();
    assert_eq!(
        candidates.len(),
        2,
        "both concurrent observations should be present: {candidates:#?}"
    );
}

#[tokio::test]
async fn session_start_context_exact_markdown() {
    let daemon = TestDaemon::start().await;
    let mut c = daemon.connect().await;
    let scope = json!({ "repoId": PROJECT_A });

    // The daemon renders timestamps in the *host* local zone (matching the Node
    // hook). Rather than mutate the process `TZ` (forbidden: unsafe + races), we
    // compute the expected dates with the same public formatter + offset the
    // daemon uses, so the assertion is host-independent.
    // Distinct timestamps so `ORDER BY ts DESC` is unambiguous: c < a < b.
    let ts_c = 1_700_000_000_000_i64; // pinned message
    let ts_a = 1_700_000_001_000_i64; // git status
    let ts_b = 1_700_049_600_000_i64; // ran tests (newest)
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let offset = kindling_server::inject::local_offset_seconds(now_ms);
    let date_c = kindling_server::inject::format_local_datetime(ts_c, offset);
    let date_a = kindling_server::inject::format_local_datetime(ts_a, offset);
    let date_b = kindling_server::inject::format_local_datetime(ts_b, offset);

    for (content, ts) in [("git status", ts_a), ("ran tests", ts_b)] {
        let resp = c
            .send(
                "POST",
                "/v1/observations",
                Some(PROJECT_A),
                Some(json!({
                    "kind": "command",
                    "content": content,
                    "ts": ts,
                    "scopeIds": { "repoId": PROJECT_A },
                })),
            )
            .await;
        assert_eq!(resp.status, StatusCode::CREATED, "seed obs");
    }

    // A pinned observation to exercise the Pinned Items block.
    let resp = c
        .send(
            "POST",
            "/v1/observations",
            Some(PROJECT_A),
            Some(json!({
                "kind": "message",
                "content": "use argon2id for hashing",
                "ts": ts_c,
                "scopeIds": { "repoId": PROJECT_A },
            })),
        )
        .await;
    let pin_target = resp.json()["id"].as_str().unwrap().to_string();
    let resp = c
        .send(
            "POST",
            "/v1/pins",
            Some(PROJECT_A),
            Some(json!({
                "targetType": "observation",
                "targetId": pin_target,
                "note": "auth decision",
                "scopeIds": { "repoId": PROJECT_A },
            })),
        )
        .await;
    assert_eq!(resp.status, StatusCode::CREATED);

    // Call the endpoint.
    let resp = c
        .send(
            "POST",
            "/v1/context/session-start",
            Some(PROJECT_A),
            Some(json!({ "maxResults": 10, "scopeIds": scope })),
        )
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    let additional = resp.json()["additionalContext"]
        .as_str()
        .expect("additionalContext string")
        .to_string();

    // Recent activity is newest-first (b > a > c); all three observations appear
    // (the pinned one is NOT excluded from recent in the Node hook).
    let expected = format!(
        "# Prior Context (from Kindling)\n\n\
The following is prior session context for this project:\n\
## Pinned Items\n\
- **auth decision**: use argon2id for hashing\n\
## Recent Activity\n\
- [{date_b}] command: ran tests\n\
- [{date_a}] command: git status\n\
- [{date_c}] message: use argon2id for hashing"
    );
    assert_eq!(additional, expected);
}

#[tokio::test]
async fn session_start_context_empty_returns_null() {
    let daemon = TestDaemon::start().await;
    let mut c = daemon.connect().await;
    let resp = c
        .send(
            "POST",
            "/v1/context/session-start",
            Some(PROJECT_A),
            Some(json!({ "scopeIds": { "repoId": "/no/such/repo" } })),
        )
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert!(resp.json()["additionalContext"].is_null());
}

#[tokio::test]
async fn pre_compact_context_exact_markdown() {
    let daemon = TestDaemon::start().await;
    let mut c = daemon.connect().await;
    let scope = json!({ "repoId": PROJECT_B });

    // Open a capsule scoped to the repo, then close it with a summary.
    let resp = c
        .send(
            "POST",
            "/v1/capsules",
            Some(PROJECT_B),
            Some(json!({
                "kind": "pocketflow_node",
                "intent": "work",
                "scopeIds": { "repoId": PROJECT_B },
            })),
        )
        .await;
    let capsule_id = resp.json()["id"].as_str().unwrap().to_string();
    let resp = c
        .send(
            "PATCH",
            &format!("/v1/capsules/{capsule_id}/close"),
            Some(PROJECT_B),
            Some(json!({
                "generateSummary": true,
                "summaryContent": "we shipped the feature",
                "confidence": 0.9,
            })),
        )
        .await;
    assert_eq!(resp.status, StatusCode::OK);

    // A pinned summary.
    let resp = c
        .send(
            "POST",
            "/v1/observations",
            Some(PROJECT_B),
            Some(json!({
                "kind": "message",
                "content": "remember the migration step",
                "scopeIds": { "repoId": PROJECT_B },
            })),
        )
        .await;
    let obs_id = resp.json()["id"].as_str().unwrap().to_string();
    let resp = c
        .send(
            "POST",
            "/v1/pins",
            Some(PROJECT_B),
            Some(json!({
                "targetType": "observation",
                "targetId": obs_id,
                "note": "migration",
                "scopeIds": { "repoId": PROJECT_B },
            })),
        )
        .await;
    assert_eq!(resp.status, StatusCode::CREATED);

    let resp = c
        .send(
            "POST",
            "/v1/context/pre-compact",
            Some(PROJECT_B),
            Some(json!({ "scopeIds": scope })),
        )
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    let additional = resp.json()["additionalContext"]
        .as_str()
        .expect("additionalContext string")
        .to_string();

    let expected = "## Pinned Items (preserve across compaction)\n\
- **migration**: remember the migration step\n\
## Session Summary\n\
we shipped the feature";
    assert_eq!(additional, expected);
}

#[tokio::test]
async fn pre_compact_context_empty_returns_null() {
    let daemon = TestDaemon::start().await;
    let mut c = daemon.connect().await;
    let resp = c
        .send(
            "POST",
            "/v1/context/pre-compact",
            Some(PROJECT_A),
            Some(json!({ "scopeIds": { "repoId": "/no/such/repo" } })),
        )
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert!(resp.json()["additionalContext"].is_null());
}

#[tokio::test]
async fn context_endpoints_require_project_header() {
    let daemon = TestDaemon::start().await;
    let mut c = daemon.connect().await;
    for path in ["/v1/context/session-start", "/v1/context/pre-compact"] {
        let resp = c.send("POST", path, None, Some(json!({}))).await;
        assert_eq!(resp.status, StatusCode::BAD_REQUEST, "{path}");
    }
}

#[tokio::test]
async fn error_mapping() {
    let daemon = TestDaemon::start().await;
    let mut c = daemon.connect().await;

    // Close a nonexistent capsule → 404.
    let resp = c
        .send(
            "PATCH",
            "/v1/capsules/does-not-exist/close",
            Some(PROJECT_A),
            Some(json!({})),
        )
        .await;
    assert_eq!(resp.status, StatusCode::NOT_FOUND, "close missing → 404");

    // Open a duplicate session capsule → 409.
    let body = json!({
        "kind": "session",
        "intent": "dup",
        "scopeIds": { "sessionId": "dup-session" }
    });
    let first = c
        .send("POST", "/v1/capsules", Some(PROJECT_A), Some(body.clone()))
        .await;
    assert_eq!(first.status, StatusCode::CREATED);
    let second = c
        .send("POST", "/v1/capsules", Some(PROJECT_A), Some(body))
        .await;
    assert_eq!(
        second.status,
        StatusCode::CONFLICT,
        "duplicate open session → 409"
    );

    // Invalid body (empty intent) → 400.
    let resp = c
        .send(
            "POST",
            "/v1/capsules",
            Some(PROJECT_A),
            Some(json!({ "kind": "pocketflow_node", "intent": "", "scopeIds": {} })),
        )
        .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST, "empty intent → 400");
}

#[tokio::test]
async fn get_open_capsule_round_trip() {
    let daemon = TestDaemon::start().await;
    let mut c = daemon.connect().await;

    // No open capsule for the session yet → 200 with JSON null.
    let resp = c
        .send(
            "GET",
            "/v1/capsules/open?sessionId=sess-1",
            Some(PROJECT_A),
            None,
        )
        .await;
    assert_eq!(resp.status, StatusCode::OK, "no open capsule → 200");
    assert!(resp.json().is_null(), "no open capsule → null body");

    // Open a session capsule.
    let opened = c
        .send(
            "POST",
            "/v1/capsules",
            Some(PROJECT_A),
            Some(json!({
                "kind": "session",
                "intent": "resolve me",
                "scopeIds": { "sessionId": "sess-1" }
            })),
        )
        .await;
    assert_eq!(opened.status, StatusCode::CREATED);
    let opened_id = opened.json()["id"].as_str().unwrap().to_string();

    // Now the open capsule resolves by session id.
    let resp = c
        .send(
            "GET",
            "/v1/capsules/open?sessionId=sess-1",
            Some(PROJECT_A),
            None,
        )
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    let cap = resp.json();
    assert_eq!(cap["id"], opened_id);
    assert_eq!(cap["status"], "open");
    assert_eq!(cap["scopeIds"]["sessionId"], "sess-1");

    // The session id may also arrive via the X-Kindling-Session header.
    let resp = c
        .send_with_headers(
            "GET",
            "/v1/capsules/open",
            Some(PROJECT_A),
            &[("x-kindling-session", "sess-1")],
            None,
        )
        .await;
    assert_eq!(resp.status, StatusCode::OK, "header session id resolves");
    assert_eq!(resp.json()["id"], opened_id);

    // A different session has no open capsule → null.
    let resp = c
        .send(
            "GET",
            "/v1/capsules/open?sessionId=other",
            Some(PROJECT_A),
            None,
        )
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert!(resp.json().is_null());

    // Missing session id (neither query nor header) → 400.
    let resp = c
        .send("GET", "/v1/capsules/open", Some(PROJECT_A), None)
        .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST, "no session id → 400");

    // Missing project header → 400.
    let resp = c
        .send("GET", "/v1/capsules/open?sessionId=sess-1", None, None)
        .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST, "no project → 400");
}

fn assert_capability_block(body: &serde_json::Value) {
    let kinds = body["supportedKinds"].as_array().expect("supportedKinds");
    assert_eq!(kinds.len(), 9);
    let expected: Vec<serde_json::Value> = [
        "tool_call",
        "command",
        "file_diff",
        "error",
        "message",
        "node_start",
        "node_end",
        "node_output",
        "node_error",
    ]
    .into_iter()
    .map(serde_json::Value::from)
    .collect();
    assert_eq!(kinds, &expected);
    let storage = body["storagePath"].as_str().expect("storagePath");
    assert!(!storage.is_empty(), "storagePath must be non-empty");
    let registry = body["kindRegistry"].as_array().expect("kindRegistry");
    assert_eq!(registry.len(), 9);
    for entry in registry {
        assert!(entry["kind"].is_string());
        let fields = entry["requiredFields"].as_array().expect("requiredFields");
        assert!(!fields.is_empty());
    }
}

/// `POST /v1/observations` surfaces redaction evidence (count + classes, never
/// the matched values) on the response — KINTEG-006.
#[tokio::test]
async fn append_response_carries_redaction_evidence() {
    let daemon = TestDaemon::start().await;
    let mut c = daemon.connect().await;

    let resp = c
        .send(
            "POST",
            "/v1/observations",
            Some(PROJECT_A),
            Some(json!({
                "kind": "message",
                "content": "api_key=abcdef123456789 and Bearer abcdefghijklmnopqrstuvwxyz",
                "scopeIds": { "sessionId": "s1" },
            })),
        )
        .await;
    assert_eq!(resp.status, StatusCode::CREATED, "append observation");
    let body = resp.json();

    // Content came back masked; the raw secrets never appear anywhere in the
    // response body.
    let serialized = body.to_string();
    assert!(
        !serialized.contains("abcdef123456789"),
        "raw credential leaked"
    );
    assert!(
        !serialized.contains("abcdefghijklmnopqrstuvwxyz"),
        "raw bearer token leaked"
    );

    let redaction = &body["redaction"];
    assert_eq!(redaction["count"], 2, "two secrets masked");
    let classes = redaction["classes"].as_array().expect("classes array");
    assert_eq!(
        classes,
        &vec![
            serde_json::Value::from("credentialAssignment"),
            serde_json::Value::from("bearerToken"),
        ]
    );

    // A clean append reports empty evidence (the block is always present).
    let resp = c
        .send(
            "POST",
            "/v1/observations",
            Some(PROJECT_A),
            Some(json!({
                "kind": "message",
                "content": "nothing sensitive here",
                "scopeIds": { "sessionId": "s1" },
            })),
        )
        .await;
    assert_eq!(resp.status, StatusCode::CREATED);
    let body = resp.json();
    assert_eq!(body["redaction"]["count"], 0);
    assert!(body["redaction"]["classes"].as_array().unwrap().is_empty());
}
