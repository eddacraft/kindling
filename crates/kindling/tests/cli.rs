//! Per-command integration tests driving the `kindling-cli` binary against a
//! temp `--db`. Exercises the round-trips (log→status, open→close, pin→unpin→
//! list, search, export→import) and the `--json` shapes.

mod support;

use serde_json::json;
use support::{assert_success, json_stdout, read, stdout, CliEnv};

#[test]
fn log_then_status_counts_observation() {
    let env = CliEnv::new();

    let out = env.run_db(&["log", "hello world", "--kind", "message"]);
    assert_success(&out);
    assert!(stdout(&out).contains("Observation logged"));

    let status = env.run_db(&["status", "--json"]);
    assert_success(&status);
    let v = json_stdout(&status);
    assert_eq!(v["counts"]["observations"], json!(1));
    assert_eq!(v["counts"]["capsules"], json!(0));
    // Activity timestamp present after a log.
    assert!(v["activity"]["latestTimestamp"].is_number());
}

#[test]
fn log_json_has_expected_fields() {
    let env = CliEnv::new();
    let out = env.run_db(&["log", "json shape check", "--json", "--session", "s1"]);
    assert_success(&out);
    let v = json_stdout(&out);
    assert_eq!(v["kind"], json!("message"));
    assert_eq!(v["content"], json!("json shape check"));
    assert_eq!(v["provenance"]["source"], json!("cli"));
    assert_eq!(v["scopeIds"]["sessionId"], json!("s1"));
    assert_eq!(v["redacted"], json!(false));
    assert!(v["id"].is_string());
    assert!(v["ts"].is_number());
}

#[test]
fn invalid_kind_errors_in_json() {
    let env = CliEnv::new();
    let out = env.run_db(&["log", "x", "--kind", "bogus", "--json"]);
    assert!(!out.status.success());
    let stderr = support::stderr(&out);
    let v: serde_json::Value = serde_json::from_str(stderr.trim()).unwrap();
    assert!(v["error"]
        .as_str()
        .unwrap()
        .contains("Invalid kind: 'bogus'"));
}

#[test]
fn capsule_open_then_close_roundtrip() {
    let env = CliEnv::new();

    let open = env.run_db(&["capsule", "open", "--intent", "do a thing", "--json"]);
    assert_success(&open);
    let opened = json_stdout(&open);
    assert_eq!(opened["status"], json!("open"));
    assert_eq!(opened["type"], json!("session"));
    assert_eq!(opened["intent"], json!("do a thing"));
    let id = opened["id"].as_str().unwrap().to_string();

    let close = env.run_db(&["capsule", "close", &id, "--summary", "wrapped up", "--json"]);
    assert_success(&close);
    let closed = json_stdout(&close);
    assert_eq!(closed["status"], json!("closed"));
    assert!(closed["closedAt"].is_number());

    // A summary now exists.
    let status = env.run_db(&["status", "--json"]);
    let v = json_stdout(&status);
    assert_eq!(v["counts"]["summaries"], json!(1));
    assert_eq!(v["counts"]["capsules"], json!(1));
    assert_eq!(v["counts"]["openCapsules"], json!(0));
}

#[test]
fn pin_unpin_and_list_pins() {
    let env = CliEnv::new();

    // Need a real observation to pin to.
    let log = env.run_db(&["log", "pin me", "--json"]);
    assert_success(&log);
    let obs_id = json_stdout(&log)["id"].as_str().unwrap().to_string();

    let pin = env.run_db(&[
        "pin",
        "observation",
        &obs_id,
        "--note",
        "important",
        "--json",
    ]);
    assert_success(&pin);
    let pinned = json_stdout(&pin);
    assert_eq!(pinned["targetType"], json!("observation"));
    assert_eq!(pinned["targetId"], json!(obs_id));
    assert_eq!(pinned["reason"], json!("important"));
    let pin_id = pinned["id"].as_str().unwrap().to_string();

    // list pins shows it (camelCase Pin shape).
    let list = env.run_db(&["list", "pins", "--json"]);
    assert_success(&list);
    let pins = json_stdout(&list);
    assert_eq!(pins.as_array().unwrap().len(), 1);
    assert_eq!(pins[0]["targetType"], json!("observation"));
    assert_eq!(pins[0]["reason"], json!("important"));

    // unpin removes it.
    let unpin = env.run_db(&["unpin", &pin_id, "--json"]);
    assert_success(&unpin);
    let r = json_stdout(&unpin);
    assert_eq!(r["success"], json!(true));
    assert_eq!(r["pinId"], json!(pin_id));

    let list2 = env.run_db(&["list", "pins", "--json"]);
    assert_eq!(json_stdout(&list2).as_array().unwrap().len(), 0);
}

#[test]
fn forget_redacts_observation() {
    let env = CliEnv::new();

    // Log a searchable observation.
    let log = env.run_db(&["log", "forgettable cli needle", "--json"]);
    assert_success(&log);
    let obs_id = json_stdout(&log)["id"].as_str().unwrap().to_string();

    // It surfaces in search before forgetting.
    let search = env.run_db(&["search", "needle", "--json"]);
    assert_success(&search);
    let before = json_stdout(&search);
    assert!(
        before["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["entity"]["id"] == json!(obs_id)),
        "observation should surface before forget: {before:#}"
    );

    // Forget it (text mode).
    let forget = env.run_db(&["forget", &obs_id]);
    assert_success(&forget);
    assert!(stdout(&forget).contains(&format!("Redacted observation {obs_id}")));

    // It no longer surfaces in search.
    let search2 = env.run_db(&["search", "needle", "--json"]);
    assert_success(&search2);
    let after = json_stdout(&search2);
    assert!(
        !after["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["entity"]["id"] == json!(obs_id)),
        "redacted observation must not surface after forget: {after:#}"
    );

    // The raw row shows redacted = 1 and the placeholder content.
    let list = env.run_db(&["list", "observations", "--json"]);
    assert_success(&list);
    let rows = json_stdout(&list);
    let row = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == json!(obs_id))
        .expect("row still present after redact");
    assert_eq!(row["redacted"], json!(1));
    assert_eq!(row["content"], json!("[redacted]"));
}

#[test]
fn forget_json_shape() {
    let env = CliEnv::new();
    let log = env.run_db(&["log", "json forget target", "--json"]);
    let obs_id = json_stdout(&log)["id"].as_str().unwrap().to_string();

    let forget = env.run_db(&["forget", &obs_id, "--json"]);
    assert_success(&forget);
    let v = json_stdout(&forget);
    assert_eq!(v["redacted"], json!(true));
    assert_eq!(v["id"], json!(obs_id));
}

#[test]
fn forget_unknown_observation_errors() {
    let env = CliEnv::new();
    let out = env.run_db(&["forget", "does-not-exist", "--json"]);
    assert!(!out.status.success());
    let v: serde_json::Value = serde_json::from_str(support::stderr(&out).trim()).unwrap();
    assert!(
        v["error"].as_str().unwrap().contains("does-not-exist")
            || v["error"]
                .as_str()
                .unwrap()
                .to_lowercase()
                .contains("not found"),
        "error should mention the missing observation: {v}"
    );
}

#[test]
fn list_observations_uses_raw_row_shape() {
    let env = CliEnv::new();
    env.run_db(&["log", "row shape", "--session", "sess"]);

    let list = env.run_db(&["list", "observations", "--json"]);
    assert_success(&list);
    let rows = json_stdout(&list);
    let row = &rows[0];
    // Raw SQLite columns: snake_case, redacted as integer, scope_ids as string.
    assert_eq!(row["kind"], json!("message"));
    assert_eq!(row["content"], json!("row shape"));
    assert_eq!(row["redacted"], json!(0));
    assert!(row["ts"].is_number());
    assert!(row["scope_ids"].is_string());
    let scope: serde_json::Value =
        serde_json::from_str(row["scope_ids"].as_str().unwrap()).unwrap();
    assert_eq!(scope["sessionId"], json!("sess"));
}

#[test]
fn list_capsules_uses_raw_row_shape() {
    let env = CliEnv::new();
    let open = env.run_db(&["capsule", "open", "--intent", "cap", "--json"]);
    assert_success(&open);

    let list = env.run_db(&["list", "capsules", "--json"]);
    assert_success(&list);
    let rows = json_stdout(&list);
    let row = &rows[0];
    assert_eq!(row["type"], json!("session"));
    assert_eq!(row["intent"], json!("cap"));
    assert_eq!(row["status"], json!("open"));
    assert!(row["opened_at"].is_number());
    assert!(row["closed_at"].is_null());
    assert!(row["scope_ids"].is_string());
}

#[test]
fn search_finds_logged_observation() {
    let env = CliEnv::new();
    env.run_db(&["log", "the quick brown fox jumps"]);

    let search = env.run_db(&["search", "brown fox", "--json"]);
    assert_success(&search);
    let v = json_stdout(&search);
    let candidates = v["candidates"].as_array().unwrap();
    assert!(!candidates.is_empty(), "expected at least one candidate");
    let content = candidates[0]["entity"]["content"].as_str().unwrap();
    assert!(content.contains("brown fox"));
    // Provenance shape present.
    assert_eq!(v["provenance"]["query"], json!("brown fox"));
}

#[test]
fn export_then_import_dry_run_then_import_roundtrip() {
    let src = CliEnv::new();

    // Seed: one observation + one capsule with a summary.
    src.run_db(&["log", "exported observation", "--session", "s"]);
    let open = src.run_db(&["capsule", "open", "--intent", "exported capsule", "--json"]);
    let cap_id = json_stdout(&open)["id"].as_str().unwrap().to_string();
    src.run_db(&[
        "capsule",
        "close",
        &cap_id,
        "--summary",
        "a summary",
        "--json",
    ]);

    let bundle_path = src.path("bundle.json");
    let bundle_str = bundle_path.to_string_lossy().into_owned();
    let export = src.run_db(&[
        "export",
        &bundle_str,
        "--pretty",
        "--timestamp",
        "1700000000000",
        "--json",
    ]);
    assert_success(&export);
    let meta = json_stdout(&export);
    assert_eq!(meta["success"], json!(true));
    assert_eq!(meta["stats"]["observations"], json!(1));
    assert_eq!(meta["stats"]["capsules"], json!(1));
    assert_eq!(meta["stats"]["summaries"], json!(1));

    // The written bundle has the TS-compatible top-level shape.
    let written: serde_json::Value = serde_json::from_str(&read(&bundle_path)).unwrap();
    assert_eq!(written["bundleVersion"], json!("1.0"));
    assert_eq!(written["exportedAt"], json!(1_700_000_000_000i64));
    assert_eq!(written["dataset"]["version"], json!("1.0"));
    assert_eq!(
        written["dataset"]["observations"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        written["metadata"]["description"],
        json!("Kindling memory export")
    );

    // Import into a FRESH db: dry-run first, then real.
    let dest = CliEnv::new();
    let dry = dest.run_db(&["import", &bundle_str, "--dry-run", "--json"]);
    assert_success(&dry);
    let dryv = json_stdout(&dry);
    assert_eq!(dryv["dryRun"], json!(true));
    assert_eq!(dryv["observations"], json!(1));
    assert_eq!(dryv["capsules"], json!(1));
    assert_eq!(dryv["summaries"], json!(1));
    // Dry run wrote nothing.
    let status_after_dry = dest.run_db(&["status", "--json"]);
    assert_eq!(
        json_stdout(&status_after_dry)["counts"]["observations"],
        json!(0)
    );

    // Real import.
    let imp = dest.run_db(&["import", &bundle_str, "--json"]);
    assert_success(&imp);
    let impv = json_stdout(&imp);
    assert_eq!(impv["dryRun"], json!(false));
    assert_eq!(impv["observations"], json!(1));
    assert_eq!(impv["capsules"], json!(1));
    assert_eq!(impv["summaries"], json!(1));
    assert!(impv["errors"].as_array().unwrap().is_empty());

    // Data is now present in the destination DB.
    let status = dest.run_db(&["status", "--json"]);
    let v = json_stdout(&status);
    assert_eq!(v["counts"]["observations"], json!(1));
    assert_eq!(v["counts"]["capsules"], json!(1));
    assert_eq!(v["counts"]["summaries"], json!(1));

    // Re-import is idempotent (INSERT OR IGNORE → zero new rows).
    let reimp = dest.run_db(&["import", &bundle_str, "--json"]);
    let rv = json_stdout(&reimp);
    assert_eq!(rv["observations"], json!(0));
    assert_eq!(rv["capsules"], json!(0));
}

#[test]
fn import_via_daemon_is_rejected() {
    let env = CliEnv::new();
    // Build any bundle file first.
    env.run_db(&["log", "x"]);
    let bundle = env.path("b.json");
    let bundle_s = bundle.to_string_lossy().into_owned();
    env.run_db(&["export", &bundle_s, "--timestamp", "1"]);

    let out = env.run(&["--via-daemon", "import", &bundle_s, "--db", &env.db()]);
    assert!(!out.status.success());
    assert!(support::stderr(&out).contains("--via-daemon is not supported for import"));
}

#[test]
fn unknown_list_entity_errors() {
    let env = CliEnv::new();
    let out = env.run_db(&["list", "widgets", "--json"]);
    assert!(!out.status.success());
    let v: serde_json::Value = serde_json::from_str(support::stderr(&out).trim()).unwrap();
    assert!(v["error"]
        .as_str()
        .unwrap()
        .contains("Unknown entity type: widgets"));
}

#[test]
fn init_creates_database() {
    let env = CliEnv::new();
    let out = env.run_db(&["init", "--json"]);
    assert_success(&out);
    let v = json_stdout(&out);
    assert_eq!(v["database"]["created"], json!(true));
    assert_eq!(v["database"]["existed"], json!(false));
    assert!(v["claudeCode"].is_null());
    assert!(env.db_path.exists());

    // Second init: db now exists.
    let out2 = env.run_db(&["init", "--json"]);
    let v2 = json_stdout(&out2);
    assert_eq!(v2["database"]["created"], json!(false));
    assert_eq!(v2["database"]["existed"], json!(true));
}

#[test]
fn init_claude_code_is_stubbed() {
    let env = CliEnv::new();
    let out = env.run_db(&["init", "--claude-code", "--json"]);
    assert_success(&out);
    let v = json_stdout(&out);
    // The Claude Code step never claims to configure (PORT-015 owns the cutover).
    assert_eq!(v["claudeCode"]["configured"], json!(false));
    assert!(v["claudeCode"]["message"].is_string());
}

#[test]
fn demo_loads_sample_memory_and_search_finds_jwt() {
    let env = CliEnv::new();
    let out = env.run_db(&["demo", "--reset", "--json"]);
    assert_success(&out);
    let v = json_stdout(&out);
    assert!(v["success"].as_bool().unwrap_or(false));
    assert!(v["imported"]["observations"].as_u64().unwrap_or(0) >= 5);

    let search = env.run_db(&["search", "JWT", "--json"]);
    assert_success(&search);
    let hits = json_stdout(&search);
    assert!(
        hits["candidates"]
            .as_array()
            .map(|a| !a.is_empty())
            .unwrap_or(false)
            || hits["pins"]
                .as_array()
                .map(|a| !a.is_empty())
                .unwrap_or(false)
    );
}

#[test]
fn browse_writes_html_without_opening_browser() {
    let env = CliEnv::new();
    assert_success(&env.run_db(&["demo", "--reset"]));

    let html_path = env.path("browse.html");
    let path = html_path.to_string_lossy();
    let out = env.run_db(&["browse", "--no-open", "--output", &path]);
    assert_success(&out);
    let html = read(&html_path);
    assert!(html.contains("kindling memory"));
    assert!(html.contains("obs-demo-4"));
}

#[test]
fn browse_escapes_script_breakout_in_embedded_json() {
    let env = CliEnv::new();
    let payload = "note </script><script>alert(1)</script> end";
    assert_success(&env.run_db(&["log", payload, "--json"]));

    let html_path = env.path("browse.html");
    let path = html_path.to_string_lossy();
    assert_success(&env.run_db(&["browse", "--no-open", "--output", &path]));
    let html = read(&html_path);

    let script_start = html
        .find("const bundle = ")
        .expect("bundle assignment in HTML");
    let script_end = script_start
        + html[script_start..]
            .find("</script>")
            .expect("bundle script close tag");
    let script_body = &html[script_start..script_end];
    assert!(
        !script_body.contains("</script>"),
        "bundle script block must not contain unescaped </script>: {script_body}"
    );

    let json_start = script_start + "const bundle = ".len();
    let rest = &html[json_start..script_end];
    let json_end = rest
        .find(";\n")
        .or_else(|| rest.find(';'))
        .expect("bundle assignment terminator");
    let bundle: serde_json::Value =
        serde_json::from_str(&rest[..json_end]).expect("bundle must be valid JSON");
    let dataset = bundle.get("dataset").unwrap_or(&bundle);
    let observations = dataset["observations"]
        .as_array()
        .expect("observations array in bundle");
    assert!(
        observations
            .iter()
            .any(|o| o["content"].as_str() == Some(payload)),
        "payload must round-trip in embedded JSON: {bundle:#}"
    );
}
