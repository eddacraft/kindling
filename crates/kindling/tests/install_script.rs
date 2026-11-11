//! Regression tests for `install.sh` cargo fallback (GH-193).
//!
//! The installer is POSIX sh. These tests source it with stub `curl` and
//! `cargo` on PATH so a failed archive download, a version pin, and a
//! nonstandard Cargo root can be asserted without a real crates.io install.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn write_exec(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, body).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

fn assert_success(output: &Output, context: &str) {
    assert!(
        output.status.success(),
        "{context}: expected success, got {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

struct Harness {
    dir: tempfile::TempDir,
    stub_bin: PathBuf,
    cargo_home: PathBuf,
    install_root: PathBuf,
    install_dir: PathBuf,
    log_dir: PathBuf,
}

impl Harness {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let stub_bin = dir.path().join("stub-bin");
        let cargo_home = dir.path().join("cargo-home");
        let install_root = dir.path().join("cargo-install-root");
        let install_dir = dir.path().join("install-dir");
        let log_dir = dir.path().join("logs");
        fs::create_dir_all(&stub_bin).unwrap();
        fs::create_dir_all(&log_dir).unwrap();
        fs::create_dir_all(&install_dir).unwrap();

        let decoy = format!(
            "#!/bin/sh\nprintf 'decoy:%s\\n' \"$0\" >> '{}/decoy-invoked'\nexit 0\n",
            log_dir.display()
        );
        write_exec(&stub_bin.join("kindling"), &decoy);

        let curl = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" >> '{}/curl-args'\necho 'curl: simulated download failure' >&2\nexit 22\n",
            log_dir.display()
        );
        write_exec(&stub_bin.join("curl"), &curl);

        let cargo = format!(
            r#"#!/bin/sh
printf '%s\n' "$@" > '{log}/cargo-args'
if [ -n "${{CARGO_INSTALL_ROOT:-}}" ]; then
  dest="${{CARGO_INSTALL_ROOT}}/bin"
elif [ -n "${{CARGO_HOME:-}}" ]; then
  dest="${{CARGO_HOME}}/bin"
else
  dest="${{HOME}}/.cargo/bin"
fi
mkdir -p "$dest"
printf '%s\n' '#!/bin/sh' 'printf "cargo:%s\n" "$0" >> "{log}/kindling-invoked"' 'exit 0' > "$dest/kindling"
chmod 0755 "$dest/kindling"
"#,
            log = log_dir.display()
        );
        write_exec(&stub_bin.join("cargo"), &cargo);

        Self {
            dir,
            stub_bin,
            cargo_home,
            install_root,
            install_dir,
            log_dir,
        }
    }

    fn run(&self, extra_env: &[(&str, &str)]) -> Output {
        let install_sh = workspace_root().join("install.sh");
        let cmd_out = self.log_dir.join("kindling-cmd");
        let script = format!(
            r#"
set -eu
export KINDLING_INSTALL_SOURCED=1
. "{install_sh}"
detect_target
resolve_version
install_binary
printf '%s\n' "$KINDLING_CMD" > "{cmd_out}"
"$KINDLING_CMD" init --claude-code
"#,
            install_sh = install_sh.display(),
            cmd_out = cmd_out.display(),
        );

        let path = format!(
            "{}:{}",
            self.stub_bin.display(),
            std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into())
        );

        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg(script)
            .env("PATH", path)
            .env("HOME", self.dir.path())
            .env("KINDLING_INSTALL_DIR", &self.install_dir)
            .env("KINDLING_NO_DEMO", "1")
            .env("CARGO_HOME", &self.cargo_home)
            .env_remove("CARGO_INSTALL_ROOT")
            .env_remove("KINDLING_REPO")
            .env_remove("KINDLING_VERSION");
        for (key, value) in extra_env {
            cmd.env(key, value);
        }
        cmd.output().expect("failed to run install.sh fragment")
    }

    fn cargo_args(&self) -> String {
        read(&self.log_dir.join("cargo-args"))
    }

    fn curl_args(&self) -> String {
        read(&self.log_dir.join("curl-args"))
    }

    fn kindling_cmd(&self) -> String {
        read(&self.log_dir.join("kindling-cmd")).trim().to_string()
    }

    fn invoked(&self) -> String {
        read(&self.log_dir.join("kindling-invoked"))
    }

    fn decoy_invoked(&self) -> String {
        read(&self.log_dir.join("decoy-invoked"))
    }
}

#[test]
fn failed_download_with_pin_uses_version_and_cargo_home_binary() {
    let harness = Harness::new();
    let output = harness.run(&[("KINDLING_VERSION", "v0.4.0")]);
    assert_success(&output, "pinned cargo fallback");

    let curl_args = harness.curl_args();
    assert!(
        curl_args.contains("/v0.4.0/kindling-0.4.0-"),
        "download should request the pinned archive, got:\n{curl_args}"
    );

    let cargo_args = harness.cargo_args();
    assert_eq!(
        cargo_args.lines().collect::<Vec<_>>(),
        ["install", "--version", "=0.4.0", "eddacraft-kindling"],
        "pinned fallback must pass an exact crates.io version, got:\n{cargo_args}"
    );

    let expected = harness.cargo_home.join("bin").join("kindling");
    assert_eq!(harness.kindling_cmd(), expected.to_string_lossy());
    assert!(
        harness
            .invoked()
            .contains(&expected.to_string_lossy().into_owned()),
        "setup must invoke the cargo-installed binary, got:\n{}",
        harness.invoked()
    );
    assert!(
        harness.decoy_invoked().is_empty(),
        "must not invoke a kindling from PATH, got:\n{}",
        harness.decoy_invoked()
    );
}

#[test]
fn failed_download_with_pin_uses_cargo_install_root() {
    let harness = Harness::new();
    let root = harness.install_root.to_string_lossy().into_owned();
    let output = harness.run(&[
        ("KINDLING_VERSION", "v0.4.0"),
        ("CARGO_INSTALL_ROOT", root.as_str()),
    ]);
    assert_success(&output, "CARGO_INSTALL_ROOT cargo fallback");

    let expected = harness.install_root.join("bin").join("kindling");
    assert_eq!(harness.kindling_cmd(), expected.to_string_lossy());
    assert!(
        harness
            .invoked()
            .contains(&expected.to_string_lossy().into_owned()),
        "setup must invoke CARGO_INSTALL_ROOT/bin/kindling, got:\n{}",
        harness.invoked()
    );
    assert!(
        !harness.cargo_home.join("bin").join("kindling").exists(),
        "binary must not be written to CARGO_HOME when CARGO_INSTALL_ROOT is set"
    );
    assert!(harness.decoy_invoked().is_empty());
}

#[test]
fn failed_download_rejects_nondefault_repo() {
    let harness = Harness::new();
    let output = harness.run(&[
        ("KINDLING_VERSION", "v0.4.0"),
        ("KINDLING_REPO", "acme/kindling-fork"),
    ]);
    assert!(
        !output.status.success(),
        "non-default KINDLING_REPO must not fall back to crates.io"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("acme/kindling-fork"),
        "error should name the requested repository, got:\n{stderr}"
    );
    assert!(
        stderr.contains("crates.io"),
        "error should say cargo fallback is crates.io-only, got:\n{stderr}"
    );
    assert!(
        harness.cargo_args().is_empty(),
        "cargo must not be invoked for a non-default repository, got:\n{}",
        harness.cargo_args()
    );
    assert!(harness.decoy_invoked().is_empty());
}
