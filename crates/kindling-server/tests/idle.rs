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

    // Make an early request that touches last-activity.
    {
        let mut c = daemon.connect().await;
        let resp = c.send("GET", "/v1/health", None, None).await;
        assert_eq!(resp.status, StatusCode::OK);
    }

    // Sleep partway through the first idle interval.
    tokio::time::sleep(Duration::from_millis(150)).await;

    // A second request resets the idle clock (deadline moves to ~400ms).
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

    // Sleep PAST the original idle timeout (250ms) — to ~300ms — and confirm
    // the keepalive held: the daemon must still be up.
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(
        !daemon.is_finished(),
        "daemon must stay alive across the original idle deadline after keepalive requests"
    );

    // After going quiet, it should still shut down within a generous window.
    let result = tokio::time::timeout(Duration::from_secs(3), daemon.join()).await;
    assert!(
        result.is_ok(),
        "daemon should eventually shut down after going idle"
    );
    assert!(result.unwrap().is_ok());
}

#[tokio::test]
async fn cancelled_in_flight_request_does_not_pin_daemon_busy() {
    use std::io::Write;
    use std::os::unix::net::UnixStream;

    let daemon = TestDaemon::start_with_idle(Duration::from_millis(200)).await;

    // Headers + a partial body: the JSON extractor waits, so the activity
    // middleware has entered and the handler future is still pending.
    let mut stream = UnixStream::connect(&daemon.socket_path).expect("connect uds");
    stream
        .write_all(
            b"POST /v1/observations HTTP/1.1\r\n\
              Host: kindling.local\r\n\
              Content-Type: application/json\r\n\
              X-Kindling-Project: /tmp/idle-cancel\r\n\
              Content-Length: 1000000\r\n\
              \r\n\
              {",
        )
        .expect("write partial request");
    stream.flush().expect("flush partial request");

    // Well past the idle timeout: the daemon must stay up while this request
    // is in flight, which also proves enter() ran.
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(
        !daemon.is_finished(),
        "an in-flight request must keep the daemon from going idle"
    );

    // Dropping the connection cancels the handler future. Without a drop
    // guard, leave() is skipped and the daemon stays busy forever.
    drop(stream);

    let result = tokio::time::timeout(Duration::from_secs(3), daemon.join()).await;
    assert!(
        result.is_ok(),
        "cancelling a request must not leak in-flight activity"
    );
    assert!(result.unwrap().is_ok(), "serve should resolve Ok on idle");
}
