//! Per-command integration tests driving the `kindling-cli` binary against a
//! temp `--db`. Exercises the round-trips (log→status, open→close, pin→unpin→
//! list, search, export→import) and the `--json` shapes.

mod support;

use std::path::PathBuf;

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
    assert_status_capability_block(&v);
}

#[test]
fn status_json_capability_block_is_consistent_across_runs() {
    let env = CliEnv::new();
    let _ = env.run_db(&["log", "consistency probe", "--kind", "message"]);

    let first = json_stdout(&env.run_db(&["status", "--json"]));
    let second = json_stdout(&env.run_db(&["status", "--json"]));
    assert_status_capability_block(&first);
    assert_status_capability_block(&second);
    assert_eq!(first["supportedKinds"], second["supportedKinds"]);
    assert_eq!(first["kindRegistry"], second["kindRegistry"]);
    assert_eq!(first["schemaVersion"], second["schemaVersion"]);
    assert_eq!(first["version"], second["version"]);
}

fn assert_status_capability_block(v: &serde_json::Value) {
    assert!(v["version"].is_string());
    assert!(v["schemaVersion"].is_number());
    let kinds = v["supportedKinds"].as_array().expect("supportedKinds");
    assert_eq!(kinds.len(), 9);
    let storage = v["storagePath"].as_str().expect("storagePath");
    assert!(!storage.is_empty());
    let registry = v["kindRegistry"].as_array().expect("kindRegistry");
    assert_eq!(registry.len(), 9);
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

/// The two commands that install the Claude Code plugin from this
/// repository's marketplace (`.claude-plugin/marketplace.json`).
const PLUGIN_INSTALL: [&str; 2] = [
    "/plugin marketplace add eddacraft/kindling",
    "/plugin install kindling@kindling-plugins",
];

/// A fresh profile: an isolated HOME (optionally with `~/.claude/`) and a PATH
/// directory (optionally holding a stub `kindling` executable).
fn claude_profile(env: &CliEnv, with_claude_dir: bool, with_binary: bool) -> (PathBuf, PathBuf) {
    let home = env.path("home");
    std::fs::create_dir_all(&home).unwrap();
    if with_claude_dir {
        std::fs::create_dir_all(home.join(".claude")).unwrap();
    }
    let bin_dir = env.path("bin");
    std::fs::create_dir_all(&bin_dir).unwrap();
    if with_binary {
        let stub = bin_dir.join(if cfg!(windows) {
            "kindling.exe"
        } else {
            "kindling"
        });
        std::fs::write(&stub, "#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
    (home, bin_dir)
}

#[test]
fn init_claude_code_reports_plugin_install_when_detected() {
    let env = CliEnv::new();
    let (home, bin_dir) = claude_profile(&env, true, true);
    let out = env.run_db_with_env(
        &["init", "--claude-code", "--json"],
        &[("HOME", &home), ("USERPROFILE", &home), ("PATH", &bin_dir)],
    );
    assert_success(&out);
    let cc = &json_stdout(&out)["claudeCode"];
    // init does not edit Claude Code's settings; the plugin is installed from
    // inside Claude Code, so `configured` stays false and the commands say how.
    assert_eq!(cc["configured"], json!(false));
    assert_eq!(cc["detected"], json!(true));
    assert_eq!(cc["binaryOnPath"], json!(true));
    assert_eq!(cc["installCommands"], json!(PLUGIN_INSTALL));
    let message = cc["message"].as_str().unwrap();
    assert!(
        !message.contains("PORT-015"),
        "no stale cutover note: {message}"
    );
}

#[test]
fn init_claude_code_flags_missing_claude_and_binary() {
    let env = CliEnv::new();
    let (home, bin_dir) = claude_profile(&env, false, false);
    let out = env.run_db_with_env(
        &["init", "--claude-code", "--json"],
        &[("HOME", &home), ("USERPROFILE", &home), ("PATH", &bin_dir)],
    );
    assert_success(&out);
    let cc = &json_stdout(&out)["claudeCode"];
    assert_eq!(cc["configured"], json!(false));
    assert_eq!(cc["detected"], json!(false));
    assert_eq!(cc["binaryOnPath"], json!(false));
    assert_eq!(cc["installCommands"], json!(PLUGIN_INSTALL));
}

#[test]
fn init_claude_code_human_output_prints_install_commands() {
    let env = CliEnv::new();
    let (home, bin_dir) = claude_profile(&env, true, true);
    let out = env.run_db_with_env(
        &["init", "--claude-code"],
        &[("HOME", &home), ("USERPROFILE", &home), ("PATH", &bin_dir)],
    );
    assert_success(&out);
    let text = stdout(&out);
    for command in PLUGIN_INSTALL {
        assert_eq!(
            text.matches(command).count(),
            1,
            "{command:?} should be listed exactly once in:\n{text}"
        );
    }
}

#[test]
fn demo_loads_sample_memory_and_search_finds_jwt() {
    let env = CliEnv::new();
    let out = env.run_db(&["demo", "--reset", "--json"]);
    assert_success(&out);
    let v = json_stdout(&out);
    assert!(v["success"].as_bool().unwrap_or(false));
    assert!(v["imported"]["observations"].as_u64().unwrap_or(0) >= 5);

    // Demo memory is scoped to its own repoId, so search across all repos
    // rather than the git repository the test happens to run inside.
    let search = env.run_db(&["search", "JWT", "--all-repos", "--json"]);
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

#[test]
fn stats_json_on_empty_database_is_zero_filled() {
    let env = CliEnv::new();
    let out = env.run_db(&["stats", "--json"]);
    assert_success(&out);
    let v = json_stdout(&out);
    assert_eq!(v["observations"]["total"], json!(0));
    assert_eq!(v["observations"]["latestTimestamp"], json!(null));
    let by_kind = v["observations"]["byKind"].as_object().unwrap();
    assert_eq!(by_kind.len(), 9);
    assert!(by_kind.values().all(|n| n == &json!(0)));
    assert_eq!(v["capsules"]["byType"]["session"], json!(0));
    assert_eq!(v["capsules"]["byType"]["pocketflow_node"], json!(0));
    assert_eq!(v["pins"]["total"], json!(0));
    assert_eq!(v["summaries"], json!(0));
}

#[test]
fn stats_json_counts_kinds_capsules_pins_and_latest_ts() {
    let env = CliEnv::new();

    let open = env.run_db(&["capsule", "open", "--intent", "stats probe", "--json"]);
    assert_success(&open);
    let cap_id = json_stdout(&open)["id"].as_str().unwrap().to_string();
    let log = env.run_db(&["log", "keep me", "--kind", "message", "--json"]);
    assert_success(&log);
    let keep_id = json_stdout(&log)["id"].as_str().unwrap().to_string();
    assert_success(&env.run_db(&["log", "cargo test", "--kind", "command"]));
    let secret = env.run_db(&["log", "hush-hush-stats-probe", "--kind", "error", "--json"]);
    assert_success(&secret);
    let secret_id = json_stdout(&secret)["id"].as_str().unwrap().to_string();
    assert_success(&env.run_db(&["forget", &secret_id]));
    assert_success(&env.run_db(&["pin", "observation", &keep_id]));
    assert_success(&env.run_db(&["capsule", "close", &cap_id, "--summary", "done"]));

    let out = env.run_db(&["stats", "--json"]);
    assert_success(&out);
    let raw = stdout(&out);
    assert!(
        !raw.contains("hush-hush-stats-probe"),
        "stats must not leak redacted content"
    );
    let v = json_stdout(&out);
    assert_eq!(v["observations"]["total"], json!(3));
    assert_eq!(v["observations"]["redacted"], json!(1));
    assert_eq!(v["observations"]["byKind"]["message"], json!(1));
    assert_eq!(v["observations"]["byKind"]["command"], json!(1));
    assert_eq!(v["observations"]["byKind"]["error"], json!(1));
    assert_eq!(v["observations"]["byKind"]["tool_call"], json!(0));
    assert!(v["observations"]["latestTimestamp"].is_number());
    assert!(v["observations"]["latestDate"].is_string());
    assert_eq!(v["capsules"]["total"], json!(1));
    assert_eq!(v["capsules"]["closed"], json!(1));
    assert_eq!(v["capsules"]["byType"]["session"], json!(1));
    assert_eq!(v["summaries"], json!(1));
    assert_eq!(v["pins"]["total"], json!(1));
    assert_eq!(v["pins"]["active"], json!(1));
    assert_eq!(v["pins"]["byTarget"]["observation"], json!(1));

    // Read-only and deterministic: a second run is identical.
    let again = env.run_db(&["stats", "--json"]);
    assert_eq!(raw, stdout(&again));
}

#[test]
fn stats_text_output_has_table_sections() {
    let env = CliEnv::new();
    assert_success(&env.run_db(&["log", "table probe", "--kind", "message"]));
    let out = env.run_db(&["stats"]);
    assert_success(&out);
    let text = stdout(&out);
    for heading in [
        "Kindling Memory Stats",
        "Observations",
        "Capsules",
        "Pins",
        "Activity",
    ] {
        assert!(text.contains(heading), "missing {heading} in:\n{text}");
    }
    assert!(text.contains("message"));
    assert!(text.contains("Latest"));
}

// --- C3: workspace auto-detection for the default search repo ---------------

/// Candidate observation contents from a `search --json` result.
fn candidate_contents(v: &serde_json::Value) -> Vec<String> {
    v["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["entity"]["content"].as_str().unwrap().to_string())
        .collect()
}

/// Seed one observation scoped to `repo` and one scoped to another repo, both
/// matching the query "lantern".
fn seed_two_repos(env: &CliEnv, repo: &str) {
    assert_success(&env.run_db(&["log", "lantern here", "--repo", repo]));
    assert_success(&env.run_db(&["log", "lantern elsewhere", "--repo", "other-repo"]));
}

#[test]
fn search_defaults_repo_to_git_root() {
    let env = CliEnv::new();
    let root = env.git_repo("proj");
    let root_str = root.to_string_lossy().into_owned();
    seed_two_repos(&env, &root_str);

    let out = env.run_db_in(&root, &["search", "lantern", "--json"]);
    assert_success(&out);
    let v = json_stdout(&out);
    assert_eq!(candidate_contents(&v), vec!["lantern here".to_string()]);
    assert_eq!(v["provenance"]["scopeIds"]["repoId"], json!(root_str));
}

#[test]
fn search_defaults_repo_to_git_root_from_nested_dir() {
    let env = CliEnv::new();
    let root = env.git_repo("proj");
    let nested = root.join("src").join("deep");
    std::fs::create_dir_all(&nested).unwrap();
    seed_two_repos(&env, &root.to_string_lossy());

    let out = env.run_db_in(&nested, &["search", "lantern", "--json"]);
    assert_success(&out);
    assert_eq!(
        candidate_contents(&json_stdout(&out)),
        vec!["lantern here".to_string()]
    );
}

#[test]
fn search_explicit_repo_overrides_detection() {
    let env = CliEnv::new();
    let root = env.git_repo("proj");
    seed_two_repos(&env, &root.to_string_lossy());

    let out = env.run_db_in(
        &root,
        &["search", "lantern", "--repo", "other-repo", "--json"],
    );
    assert_success(&out);
    let v = json_stdout(&out);
    assert_eq!(
        candidate_contents(&v),
        vec!["lantern elsewhere".to_string()]
    );
    assert_eq!(v["provenance"]["scopeIds"]["repoId"], json!("other-repo"));
}

#[test]
fn search_all_repos_disables_detection() {
    let env = CliEnv::new();
    let root = env.git_repo("proj");
    seed_two_repos(&env, &root.to_string_lossy());

    let out = env.run_db_in(&root, &["search", "lantern", "--all-repos", "--json"]);
    assert_success(&out);
    let v = json_stdout(&out);
    let mut contents = candidate_contents(&v);
    contents.sort();
    assert_eq!(contents, vec!["lantern elsewhere", "lantern here"]);
    assert!(v["provenance"]["scopeIds"].get("repoId").is_none());
}

#[test]
fn search_rejects_repo_with_all_repos() {
    let env = CliEnv::new();
    let out = env.run_db(&["search", "x", "--repo", "r", "--all-repos"]);
    assert!(!out.status.success());
    assert!(support::stderr(&out).contains("cannot be used with"));
}

#[test]
fn search_text_output_names_the_repo_scope() {
    let env = CliEnv::new();
    let root = env.git_repo("proj");
    let out = env.run_db_in(&root, &["search", "lantern"]);
    assert_success(&out);
    let text = stdout(&out);
    assert!(
        text.contains(&format!("Repo: {}", root.display())),
        "{text}"
    );
    assert!(text.contains("--all-repos"), "{text}");
}

#[test]
fn log_defaults_repo_to_git_root_and_search_finds_it() {
    let env = CliEnv::new();
    let root = env.git_repo("proj");
    let nested = root.join("sub");
    std::fs::create_dir_all(&nested).unwrap();

    let log = env.run_db_in(&nested, &["log", "lantern logged", "--json"]);
    assert_success(&log);
    assert_eq!(
        json_stdout(&log)["scopeIds"]["repoId"],
        json!(root.to_string_lossy())
    );

    let out = env.run_db_in(&root, &["search", "lantern", "--json"]);
    assert_success(&out);
    assert_eq!(
        candidate_contents(&json_stdout(&out)),
        vec!["lantern logged".to_string()]
    );
}

#[test]
fn search_outside_git_applies_no_repo_scope() {
    let env = CliEnv::new();
    let plain = env.path("plain");
    std::fs::create_dir_all(&plain).unwrap();
    // Skip on hosts where the temp dir itself sits inside a git checkout.
    let probe = std::process::Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(&plain)
        .output()
        .unwrap();
    if probe.status.success() {
        return;
    }
    seed_two_repos(&env, "some-repo");

    let out = env.run_db_in(&plain, &["search", "lantern", "--json"]);
    assert_success(&out);
    let v = json_stdout(&out);
    assert_eq!(candidate_contents(&v).len(), 2);
    assert!(v["provenance"]["scopeIds"].get("repoId").is_none());
}
