//! Integration tests for `kindling-spool` against an in-process daemon.
//!
//! "Daemon up" cases use a real `kindling-server` on a temp socket
//! ([`support::TestDaemon`]). "Daemon down" cases point the client at a socket
//! with no listener plus a spawner that fails, so every call resolves to
//! `ClientError::Unavailable` ([`support::down_client`]).

mod support;

use std::io::Write;
use std::path::PathBuf;

use kindling_client::ClientError;
use kindling_spool::{AppendOutcome, SpoolError, SpooledClient};
use kindling_types::{ObservationInput, ObservationKind, RetrieveOptions, ScopeIds};
use serde_json::Value;
use support::{down_client, TestDaemon};
use tempfile::TempDir;

const PROJECT: &str = "/tmp/kindling-spool-test/project";

fn message_input(content: &str) -> ObservationInput {
    ObservationInput {
        id: None,
        kind: ObservationKind::Message,
        content: content.to_string(),
        provenance: None,
        ts: None,
        scope_ids: ScopeIds {
            session_id: Some("s1".to_string()),
            repo_id: Some(PROJECT.to_string()),
            ..Default::default()
        },
        redacted: None,
    }
}

fn spool_tempdir() -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("spool.ndjson");
    (dir, path)
}

fn retrieve_opts(query: &str) -> RetrieveOptions {
    RetrieveOptions {
        query: query.to_string(),
        scope_ids: ScopeIds {
            session_id: Some("s1".to_string()),
            repo_id: Some(PROJECT.to_string()),
            ..Default::default()
        },
        token_budget: None,
        max_candidates: None,
        include_redacted: None,
    }
}

/// 1. Delivered when the daemon is up: returns `Delivered`, spool stays empty,
/// the observation is retrievable.
#[tokio::test]
async fn delivered_when_daemon_up() {
    let daemon = TestDaemon::start().await;
    let (_dir, spool_path) = spool_tempdir();
    let spooled = SpooledClient::new(daemon.client(PROJECT), spool_path.clone());

    let outcome = spooled
        .append_observation(message_input("delivered needle one"), None, None)
        .await
        .expect("append should not error when daemon is up");

    match outcome {
        AppendOutcome::Delivered(obs) => assert_eq!(obs.content, "delivered needle one"),
        AppendOutcome::Spooled => panic!("expected Delivered, got Spooled"),
    }

    assert!(!spool_path.exists(), "spool file must stay empty when up");
    assert_eq!(spooled.pending_count().unwrap(), 0);

    let res = spooled
        .client()
        .retrieve(retrieve_opts("needle"))
        .await
        .expect("retrieve");
    assert!(
        res.candidates
            .iter()
            .any(|c| matches!(&c.entity,
                kindling_types::RetrievedEntity::Observation(o) if o.content == "delivered needle one")),
        "delivered observation must be retrievable: {res:#?}"
    );
}

/// 2. Spooled when the daemon is down: returns `Ok(Spooled)` (no error), one
/// NDJSON line with the stable id.
#[tokio::test]
async fn spooled_when_daemon_down() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("nope.sock");
    let (_spool_dir, spool_path) = spool_tempdir();
    let spooled = SpooledClient::new(down_client(socket, PROJECT), spool_path.clone());

    let outcome = spooled
        .append_observation(message_input("buffered while down"), None, None)
        .await
        .expect("append must NOT error on outage");
    assert!(matches!(outcome, AppendOutcome::Spooled));

    let contents = std::fs::read_to_string(&spool_path).expect("spool file written");
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(lines.len(), 1, "exactly one NDJSON line: {contents:?}");

    // The line carries a stable, populated id.
    let entry: Value = serde_json::from_str(lines[0]).expect("valid json line");
    let id = entry["input"]["id"].as_str().expect("input.id present");
    assert!(
        !id.is_empty(),
        "stable id must be populated before spooling"
    );
    assert_eq!(spooled.pending_count().unwrap(), 1);
}

/// 3. Flush replays all spooled entries, in order, into the daemon.
#[tokio::test]
async fn flush_replays_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("nope.sock");
    let (_spool_dir, spool_path) = spool_tempdir();

    // Spool several while down.
    let down = SpooledClient::new(down_client(socket, PROJECT), spool_path.clone());
    let contents = ["flush alpha", "flush bravo", "flush charlie"];
    for c in contents {
        let outcome = down
            .append_observation(message_input(c), None, None)
            .await
            .unwrap();
        assert!(matches!(outcome, AppendOutcome::Spooled));
    }
    assert_eq!(down.pending_count().unwrap(), 3);
    drop(down);

    // Bring the daemon up; build a live spooled client on the same spool file.
    let daemon = TestDaemon::start().await;
    let spooled = SpooledClient::new(daemon.client(PROJECT), spool_path.clone());

    let report = spooled.flush().await.expect("flush");
    assert_eq!(report.replayed, 3);
    assert_eq!(report.remaining, 0);
    assert_eq!(spooled.pending_count().unwrap(), 0);
    assert!(!spool_path.exists(), "emptied spool file removed");

    // All retrievable.
    let res = spooled
        .client()
        .retrieve(retrieve_opts("flush"))
        .await
        .expect("retrieve");
    for c in contents {
        assert!(
            res.candidates.iter().any(|cand| matches!(&cand.entity,
                kindling_types::RetrievedEntity::Observation(o) if o.content == c)),
            "replayed observation {c:?} must be retrievable"
        );
    }
}

/// 4. Opportunistic drain: a NEW successful append also drains the backlog;
/// both land, in append order.
#[tokio::test]
async fn opportunistic_drain_on_successful_append() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("nope.sock");
    let (_spool_dir, spool_path) = spool_tempdir();

    // Spool one while down.
    let down = SpooledClient::new(down_client(socket, PROJECT), spool_path.clone());
    down.append_observation(message_input("drain backlog first"), None, None)
        .await
        .unwrap();
    assert_eq!(down.pending_count().unwrap(), 1);
    drop(down);

    // Daemon up: a NEW append should drain the backlog then deliver the new one.
    let daemon = TestDaemon::start().await;
    let spooled = SpooledClient::new(daemon.client(PROJECT), spool_path.clone());

    let outcome = spooled
        .append_observation(message_input("drain new one second"), None, None)
        .await
        .expect("append");
    assert!(matches!(outcome, AppendOutcome::Delivered(_)));
    assert_eq!(spooled.pending_count().unwrap(), 0, "backlog drained");

    // Both landed; the backlog observation came first (lower ts or equal).
    let res = spooled
        .client()
        .retrieve(retrieve_opts("drain"))
        .await
        .expect("retrieve");
    let landed: Vec<&str> = res
        .candidates
        .iter()
        .filter_map(|c| match &c.entity {
            kindling_types::RetrievedEntity::Observation(o) => Some(o.content.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        landed.contains(&"drain backlog first"),
        "backlog observation landed: {landed:?}"
    );
    assert!(
        landed.contains(&"drain new one second"),
        "new observation landed: {landed:?}"
    );
}

/// 5. An Api error (validation rejection) propagates and is NOT spooled.
#[tokio::test]
async fn api_error_propagates_not_spooled() {
    let daemon = TestDaemon::start().await;
    let (_dir, spool_path) = spool_tempdir();
    let spooled = SpooledClient::new(daemon.client(PROJECT), spool_path.clone());

    // Empty content fails service-side validation → 400.
    let bad = ObservationInput {
        id: None,
        kind: ObservationKind::Message,
        content: String::new(),
        provenance: None,
        ts: None,
        scope_ids: ScopeIds {
            session_id: Some("s1".to_string()),
            repo_id: Some(PROJECT.to_string()),
            ..Default::default()
        },
        redacted: None,
    };

    let err = spooled
        .append_observation(bad, None, Some(true))
        .await
        .expect_err("rejected observation must propagate, not spool");
    match err {
        SpoolError::Client(ClientError::Api { status, .. }) => assert_eq!(status, 400),
        other => panic!("expected Client(Api 400), got {other:?}"),
    }

    assert!(
        !spool_path.exists(),
        "rejected observation must NOT be spooled"
    );
    assert_eq!(spooled.pending_count().unwrap(), 0);
}

/// 6. Torn trailing line: a hand-written spool whose last line is truncated
/// JSON replays the good entries and does not fail on the torn tail.
#[tokio::test]
async fn flush_tolerates_torn_trailing_line() {
    let (_spool_dir, spool_path) = spool_tempdir();

    // Two good entries + a torn (truncated) trailing line.
    let good = |content: &str| {
        let entry = serde_json::json!({
            "input": message_input(content),
            "capsuleId": Value::Null,
            "validate": Value::Null,
        });
        serde_json::to_string(&entry).unwrap()
    };
    {
        let mut f = std::fs::File::create(&spool_path).unwrap();
        writeln!(f, "{}", good("torn good one")).unwrap();
        writeln!(f, "{}", good("torn good two")).unwrap();
        // Truncated JSON: no closing brace, mid-write crash.
        write!(f, "{{\"input\":{{\"kind\":\"message\",\"content\":\"torn").unwrap();
    }

    // read_spool tolerates the torn tail: pending_count sees 2.
    let daemon = TestDaemon::start().await;
    let spooled = SpooledClient::new(daemon.client(PROJECT), spool_path.clone());
    assert_eq!(
        spooled.pending_count().unwrap(),
        2,
        "torn trailing line skipped"
    );

    let report = spooled
        .flush()
        .await
        .expect("flush must not fail on torn tail");
    assert_eq!(report.replayed, 2);
    assert_eq!(report.remaining, 0);

    let res = spooled
        .client()
        .retrieve(retrieve_opts("torn"))
        .await
        .expect("retrieve");
    let landed: Vec<&str> = res
        .candidates
        .iter()
        .filter_map(|c| match &c.entity {
            kindling_types::RetrievedEntity::Observation(o) => Some(o.content.as_str()),
            _ => None,
        })
        .collect();
    assert!(landed.contains(&"torn good one"), "{landed:?}");
    assert!(landed.contains(&"torn good two"), "{landed:?}");
}

/// 7. `pending_count` reflects spool size as entries are buffered.
#[tokio::test]
async fn pending_count_reflects_spool_size() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("nope.sock");
    let (_spool_dir, spool_path) = spool_tempdir();
    let spooled = SpooledClient::new(down_client(socket, PROJECT), spool_path.clone());

    assert_eq!(
        spooled.pending_count().unwrap(),
        0,
        "empty before any append"
    );

    for expected in 1..=3 {
        spooled
            .append_observation(message_input(&format!("count {expected}")), None, None)
            .await
            .unwrap();
        assert_eq!(spooled.pending_count().unwrap(), expected);
    }
}

/// Flush stops at the first connectivity failure, keeping the remainder.
#[tokio::test]
async fn flush_keeps_remainder_when_down() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("nope.sock");
    let (_spool_dir, spool_path) = spool_tempdir();
    let spooled = SpooledClient::new(down_client(socket, PROJECT), spool_path.clone());

    for c in ["a", "b"] {
        spooled
            .append_observation(message_input(c), None, None)
            .await
            .unwrap();
    }

    // Daemon still down: flush replays nothing, keeps both.
    let report = spooled.flush().await.expect("flush while down is ok");
    assert_eq!(report.replayed, 0);
    assert_eq!(report.remaining, 2);
    assert_eq!(spooled.pending_count().unwrap(), 2);
}
