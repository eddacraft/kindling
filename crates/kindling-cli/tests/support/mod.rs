//! Shared test support: run the `kindling-cli` binary against a temp DB and
//! capture its output.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

/// A temp workspace holding a DB path the CLI writes to.
pub struct CliEnv {
    pub dir: TempDir,
    pub db_path: PathBuf,
}

impl CliEnv {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("kindling.db");
        Self { dir, db_path }
    }

    /// The `--db <path>` string for this env.
    pub fn db(&self) -> String {
        self.db_path.to_string_lossy().into_owned()
    }

    /// Path inside the temp dir.
    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    /// Run the CLI with the given args (the `--db` is NOT auto-added; pass it).
    pub fn run(&self, args: &[&str]) -> Output {
        let bin = env!("CARGO_BIN_EXE_kindling-cli");
        Command::new(bin)
            .args(args)
            .output()
            .expect("failed to run kindling-cli")
    }

    /// Run with `--db <this env's db>` prepended after the subcommand-friendly
    /// position (we just append `--db <path>` to the end — clap accepts global
    /// position-independent flags via `CommonOpts`).
    pub fn run_db(&self, args: &[&str]) -> Output {
        let db = self.db();
        let mut full: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        full.push("--db".to_string());
        full.push(db);
        let refs: Vec<&str> = full.iter().map(String::as_str).collect();
        self.run(&refs)
    }
}

/// stdout as a UTF-8 string.
pub fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// stderr as a UTF-8 string.
pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Parse stdout as JSON, panicking with context on failure.
pub fn json_stdout(output: &Output) -> serde_json::Value {
    let s = stdout(output);
    serde_json::from_str(&s)
        .unwrap_or_else(|e| panic!("stdout was not valid JSON: {e}\n---\n{s}\n---"))
}

/// Assert the process exited successfully, dumping stderr otherwise.
pub fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "expected success, got {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        stdout(output),
        stderr(output)
    );
}

/// Read a file in the env as a string.
pub fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap()
}
