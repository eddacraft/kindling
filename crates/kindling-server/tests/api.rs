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
