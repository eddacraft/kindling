//! Isolated-mode profile propagation: a customized `Profile` must reach each
//! child process in full, not be reconstructed from its name as a preset.

use std::process::Command;

use kindling_bench::profile::Profile;

#[test]
fn isolated_run_honours_a_customized_profile() {
    // A customized profile that keeps the "smoke" name but shrinks the write
    // workload. Before the fix, each child reconstructed the preset from the
    // name (write_operations = 200), silently discarding the customization.
    let profile = Profile {
        name: "smoke".to_string(),
        cold_starts: 1,
        warmup_operations: 1,
        write_operations: 7,
        seed_observations: 1,
        read_repetitions: 1,
        page_limit: 10,
        concurrent_writers: 1,
        writes_per_task: 1,
        spool_operations: 1,
    };
    let json = serde_json::to_string(&profile).expect("serialize profile");

    let output = Command::new(env!("CARGO_BIN_EXE_kindling-bench"))
        .args(["--profile-json", &json, "--isolated-process"])
        .output()
        .expect("run isolated benchmark");

    assert!(
        output.status.success(),
        "isolated benchmark failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse report JSON");

    // The report keeps the customized profile...
    assert_eq!(report["profile"]["writeOperations"].as_u64(), Some(7));

    // ...and the direct-service child actually ran the customized write count,
    // not the preset's 200.
    let direct = report["groups"]
        .as_array()
        .expect("groups array")
        .iter()
        .find(|group| group["name"] == "direct-service")
        .expect("direct-service group");
    let append = direct["metrics"]
        .as_array()
        .expect("metrics array")
        .iter()
        .find(|metric| metric["name"] == "append")
        .expect("append metric");
    assert_eq!(append["operationCount"].as_u64(), Some(7));
}

#[test]
fn profile_and_profile_json_together_are_rejected() {
    // `--profile-json` replaces the whole profile, so combining it with
    // `--profile` would silently drop the named preset. Refuse instead.
    let json = serde_json::to_string(&Profile::from_name("smoke").expect("smoke preset"))
        .expect("serialize profile");
    let output = Command::new(env!("CARGO_BIN_EXE_kindling-bench"))
        .args(["--profile", "stress", "--profile-json", &json])
        .output()
        .expect("run benchmark");

    assert!(!output.status.success(), "combination must be rejected");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--profile and --profile-json cannot be used together"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn help_lists_profile_json() {
    let output = Command::new(env!("CARGO_BIN_EXE_kindling-bench"))
        .arg("--help")
        .output()
        .expect("run --help");
    let help = String::from_utf8_lossy(&output.stderr);
    assert!(help.contains("--profile-json JSON"), "help: {help}");
}
