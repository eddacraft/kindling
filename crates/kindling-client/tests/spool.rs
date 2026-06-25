#![cfg(feature = "spool")]
//! Integration tests for the `spool` module against an in-process daemon.
//!
//! "Daemon up" cases use a real `kindling-server` on a temp socket
//! ([`support::TestDaemon`]). "Daemon down" cases point the client at a socket
//! with no listener plus a spawner that fails, so every call resolves to
//! `ClientError::Unavailable` ([`support::down_client`]).

#[path = "spool_support.rs"]
mod support;

use std::io::Write;
use std::path::PathBuf;

use kindling_client::spool::{AppendOutcome, SpoolConfig, SpoolEntry, SpoolError, SpooledClient};
use kindling_client::ClientError;
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
        AppendOutcome::Delivered(result) => {
            assert_eq!(result.observation.content, "delivered needle one");
            assert!(!result.deduplicated, "first delivery is not a dedup");
        }
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

/// Daemon-side dedup: appending the same observation id twice is exactly-once.
/// The first append writes (deduplicated=false); the second, carrying DIFFERENT
/// content, is ignored by the daemon — it returns the original stored row with
/// deduplicated=true, and retrieval still surfaces exactly one row.
#[tokio::test]
async fn replay_of_delivered_id_is_a_noop() {
    let daemon = TestDaemon::start().await;
    let (_dir, spool_path) = spool_tempdir();
    let spooled = SpooledClient::new(daemon.client(PROJECT), spool_path);

    // Fixed id so the two appends collide.
    let mut first = message_input("idempotent replay needle");
    first.id = Some("replay-fixed-id".to_string());

    let first_outcome = spooled
        .append_observation(first, None, None)
        .await
        .expect("first append");
    match first_outcome {
        AppendOutcome::Delivered(result) => {
            assert!(!result.deduplicated, "first delivery is a fresh write");
            assert_eq!(result.observation.id, "replay-fixed-id");
            assert_eq!(result.observation.content, "idempotent replay needle");
        }
        AppendOutcome::Spooled => panic!("daemon is up; expected Delivered"),
    }

    // Replay the SAME id with different content (simulating a post-crash spool
    // replay of an already-committed entry).
    let mut replay = message_input("DIFFERENT body that must be ignored");
    replay.id = Some("replay-fixed-id".to_string());

    let replay_outcome = spooled
        .append_observation(replay, None, None)
        .await
        .expect("replay append");
    match replay_outcome {
        AppendOutcome::Delivered(result) => {
            assert!(result.deduplicated, "replay of a stored id must dedup");
            assert_eq!(
                result.observation.content, "idempotent replay needle",
                "dedup returns the original stored row, not the replayed body"
            );
        }
        AppendOutcome::Spooled => panic!("daemon is up; expected Delivered"),
    }

    // Retrieval surfaces exactly ONE matching observation (no duplicate row,
    // and the ignored replay body never landed).
    let res = spooled
        .client()
        .retrieve(retrieve_opts("idempotent"))
        .await
        .expect("retrieve");
    let matches: Vec<&str> = res
        .candidates
        .iter()
        .filter_map(|c| match &c.entity {
            kindling_types::RetrievedEntity::Observation(o) if o.id == "replay-fixed-id" => {
                Some(o.content.as_str())
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        matches,
        vec!["idempotent replay needle"],
        "exactly one row for the id, carrying the original content: {res:#?}"
    );
    assert!(
        !res.candidates.iter().any(|c| matches!(&c.entity,
            kindling_types::RetrievedEntity::Observation(o)
                if o.content == "DIFFERENT body that must be ignored")),
        "the ignored replay body must never have been stored"
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

/// Spool status surfaces pending count, path, flush time, errors, and replay attempts.
#[tokio::test]
async fn spool_status_after_outage_and_flush() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("nope.sock");
    let (_spool_dir, spool_path) = spool_tempdir();

    let down = SpooledClient::new(down_client(socket, PROJECT), spool_path.clone());
    down.append_observation(message_input("status probe"), None, None)
        .await
        .unwrap();

    let while_down = down.spool_status().await.expect("status while down");
    assert_eq!(while_down.pending_count, 1);
    assert_eq!(while_down.spool_path, spool_path);
    assert!(while_down.last_error.is_some());
    assert_eq!(while_down.replay_attempts, 0);
    assert!(while_down.last_flush_time_ms.is_none());

    drop(down);

    let daemon = TestDaemon::start().await;
    let spooled = SpooledClient::new(daemon.client(PROJECT), spool_path.clone());
    spooled.flush().await.expect("flush after outage");

    let after_flush = spooled.spool_status().await.expect("status after flush");
    assert_eq!(after_flush.pending_count, 0);
    assert_eq!(after_flush.spool_path, spool_path);
    assert!(after_flush.last_flush_time_ms.is_some());
    assert!(after_flush.last_error.is_none());
    assert!(after_flush.replay_attempts >= 1);

    drop(spooled);

    // Passive inspection (CLI path) reads the persisted sidecar.
    let passive = SpooledClient::spool_status_from_path(&spool_path).expect("passive status");
    assert_eq!(passive.pending_count, 0);
    assert_eq!(passive.spool_path, spool_path);
    assert!(passive.last_flush_time_ms.is_some());
    assert!(passive.last_error.is_none());
    assert!(passive.replay_attempts >= 1);
}

/// Spooling succeeds even when the status sidecar cannot be written.
#[tokio::test]
async fn spool_append_succeeds_when_sidecar_unwritable() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("nope.sock");
    let spool_path = dir.path().join("spool.ndjson");
    // Block sidecar writes by occupying the target path with a directory.
    std::fs::create_dir(dir.path().join("spool.ndjson.status.json")).unwrap();

    let spooled = SpooledClient::new(down_client(socket, PROJECT), spool_path.clone());
    let outcome = spooled
        .append_observation(message_input("sidecar blocked"), None, None)
        .await
        .expect("spool must succeed even when sidecar write fails");
    assert!(matches!(outcome, AppendOutcome::Spooled));
    assert_eq!(spooled.pending_count().unwrap(), 1);
}

/// Passive status always reports pending count; corrupt sidecar is ignored.
#[test]
fn spool_status_from_path_tolerates_corrupt_sidecar() {
    let dir = tempfile::tempdir().unwrap();
    let spool_path = dir.path().join("spool.ndjson");
    let entry = serde_json::json!({
        "input": message_input("corrupt sidecar probe"),
    });
    std::fs::write(
        &spool_path,
        format!("{}\n", serde_json::to_string(&entry).unwrap()),
    )
    .unwrap();
    std::fs::write(
        dir.path().join("spool.ndjson.status.json"),
        "not valid json {{{",
    )
    .unwrap();

    let status = SpooledClient::spool_status_from_path(&spool_path).expect("status");
    assert_eq!(status.pending_count, 1);
    assert_eq!(status.replay_attempts, 0);
    assert!(status.last_flush_time_ms.is_none());
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

// --- Retention cap (KINTEG-009) ---------------------------------------------

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

const DAY_MS: i64 = 86_400_000;

/// Build a spool entry with an explicit `spooled_at` stamp.
fn spool_entry(content: &str, spooled_at: Option<i64>) -> SpoolEntry {
    let mut input = message_input(content);
    input.id = Some(format!("id-{content}"));
    SpoolEntry {
        input,
        capsule_id: None,
        validate: None,
        spooled_at,
    }
}

/// Hand-write a spool file (NDJSON, one entry per line) so age can be controlled.
fn write_spool(path: &PathBuf, entries: &[SpoolEntry]) {
    let mut f = std::fs::File::create(path).unwrap();
    for e in entries {
        f.write_all(serde_json::to_string(e).unwrap().as_bytes())
            .unwrap();
        f.write_all(b"\n").unwrap();
    }
    f.flush().unwrap();
}

/// The `content` of each entry still buffered in the spool file, in order.
fn spool_contents(path: &PathBuf) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| {
            let v: Value = serde_json::from_str(l).unwrap();
            v["input"]["content"].as_str().unwrap().to_string()
        })
        .collect()
}

/// A byte cap trims the oldest entries on flush even while the daemon is down,
/// keeping a contiguous newest suffix; the retained remainder then drains
/// cleanly once the daemon is up. Covers order preservation + `dropped_count`.
#[tokio::test]
async fn byte_cap_trims_oldest_on_flush_then_drains() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("nope.sock");
    let (_spool_dir, spool_path) = spool_tempdir();

    // Spool four entries while down (no cap yet).
    let down = SpooledClient::new(down_client(socket.clone(), PROJECT), spool_path.clone());
    let all = ["one", "two", "three", "four"];
    for c in all {
        down.append_observation(message_input(c), None, None)
            .await
            .unwrap();
    }
    assert_eq!(down.pending_count().unwrap(), 4);
    drop(down);

    // Cap to ~half the file so the oldest entries must be shed.
    let file_len = std::fs::metadata(&spool_path).unwrap().len();
    let capped = SpooledClient::with_config(
        down_client(socket.clone(), PROJECT),
        SpoolConfig::new(spool_path.clone()).with_max_bytes(file_len / 2),
    );

    // Flush while STILL down: replays nothing, but trims the retained remainder.
    let report = capped.flush().await.expect("flush while down ok");
    assert_eq!(report.replayed, 0);

    let kept = spool_contents(&spool_path);
    assert!(
        !kept.is_empty() && kept.len() < all.len(),
        "expected a partial trim, got {kept:?}"
    );
    // Survivors are the newest contiguous suffix of the original order.
    let expected: Vec<String> = all[all.len() - kept.len()..]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(kept, expected);
    assert!(!kept.contains(&"one".to_string()), "oldest must be dropped");

    let status = capped.spool_status().await.unwrap();
    assert_eq!(status.dropped_count as usize, all.len() - kept.len());
    drop(capped);

    // Daemon up: the retained remainder still drains to zero.
    let daemon = TestDaemon::start().await;
    let live = SpooledClient::with_config(
        daemon.client(PROJECT),
        SpoolConfig::new(spool_path.clone()).with_max_bytes(file_len / 2),
    );
    let report = live.flush().await.expect("drain");
    assert_eq!(report.replayed, kept.len());
    assert_eq!(live.pending_count().unwrap(), 0);
}

/// An age cap drops the leading run of over-age entries and spares the rest.
#[tokio::test]
async fn age_cap_trims_old_entries_on_flush() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("nope.sock");
    let (_spool_dir, spool_path) = spool_tempdir();

    let now = now_ms();
    write_spool(
        &spool_path,
        &[
            spool_entry("ancient", Some(now - 10 * DAY_MS)),
            spool_entry("stale", Some(now - 8 * DAY_MS)),
            spool_entry("fresh", Some(now - 1_000)),
        ],
    );

    let capped = SpooledClient::with_config(
        down_client(socket, PROJECT),
        SpoolConfig::new(spool_path.clone()).with_max_age_ms(7 * DAY_MS),
    );
    let report = capped.flush().await.expect("flush while down ok");
    assert_eq!(report.replayed, 0);
    assert_eq!(spool_contents(&spool_path), vec!["fresh"]);
    assert_eq!(capped.spool_status().await.unwrap().dropped_count, 2);
}

/// A legacy entry (no `spooled_at`) at the front blocks the age trim — it is
/// never age-dropped, and it shields the entries behind it too.
#[tokio::test]
async fn legacy_entry_without_stamp_blocks_age_trim() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("nope.sock");
    let (_spool_dir, spool_path) = spool_tempdir();

    let now = now_ms();
    write_spool(
        &spool_path,
        &[
            spool_entry("legacy", None),
            spool_entry("ancient", Some(now - 30 * DAY_MS)),
        ],
    );

    let capped = SpooledClient::with_config(
        down_client(socket, PROJECT),
        SpoolConfig::new(spool_path.clone()).with_max_age_ms(7 * DAY_MS),
    );
    capped.flush().await.expect("flush while down ok");
    assert_eq!(spool_contents(&spool_path), vec!["legacy", "ancient"]);
    assert_eq!(capped.spool_status().await.unwrap().dropped_count, 0);
}

/// The default (`SpooledClient::new`, no caps) never trims — existing behaviour.
#[tokio::test]
async fn unbounded_default_keeps_all_on_flush() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("nope.sock");
    let (_spool_dir, spool_path) = spool_tempdir();

    let down = SpooledClient::new(down_client(socket, PROJECT), spool_path.clone());
    for c in ["a", "b", "c"] {
        down.append_observation(message_input(c), None, None)
            .await
            .unwrap();
    }
    let report = down.flush().await.expect("flush while down ok");
    assert_eq!(report.remaining, 3);
    assert_eq!(down.spool_status().await.unwrap().dropped_count, 0);
}
