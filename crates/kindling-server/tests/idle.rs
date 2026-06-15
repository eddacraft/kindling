//! Idle-shutdown lifecycle tests.

mod support;

use std::time::Duration;

use hyper::StatusCode;
use serde_json::json;
use support::TestDaemon;

#[tokio::test]
async fn shuts_down_after_idle_with_no_requests() {
    let daemon = TestDaemon::start_with_idle(Duration::from_millis(200)).await;

    // No requests at all: serve() should resolve well within a couple seconds.
    let result = tokio::time::timeout(Duration::from_secs(3), daemon.join()).await;
    assert!(
        result.is_ok(),
        "daemon should shut down on idle within the timeout"
    );
    assert!(result.unwrap().is_ok(), "serve should resolve Ok on idle");
}

#[tokio::test]
async fn early_request_keeps_alive_then_shuts_down() {
    let daemon = TestDaemon::start_with_idle(Duration::from_millis(250)).await;

    // Make an early request that pushes last-activity past the first interval.
    {
        let mut c = daemon.connect().await;
        let resp = c.send("GET", "/v1/health", None, None).await;
        assert_eq!(resp.status, StatusCode::OK);
    }

    // It must still be alive shortly after the request (not yet idle for 250ms).
    tokio::time::sleep(Duration::from_millis(120)).await;

    // One more request to reset the idle clock again.
    {
        let mut c = daemon.connect().await;
        let resp = c
            .send(
                "POST",
                "/v1/capsules",
                Some("/tmp/idle-test/proj"),
                Some(json!({ "kind": "pocketflow_node", "intent": "keepalive", "scopeIds": {} })),
            )
            .await;
        assert_eq!(resp.status, StatusCode::CREATED);
    }

    // After going quiet, it should still shut down within a generous window.
    let result = tokio::time::timeout(Duration::from_secs(3), daemon.join()).await;
    assert!(
        result.is_ok(),
        "daemon should eventually shut down after going idle"
    );
    assert!(result.unwrap().is_ok());
}
