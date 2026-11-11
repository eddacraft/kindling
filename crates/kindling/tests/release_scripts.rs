//! Failure-injection tests for GH-194: release evidence capture and publish
//! dry-run must record each check's status and must not report success after
//! a failed fmt, clippy, install, or unexpected cargo error.

#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn write_exec(path: &Path, body: &str) {
    fs::write(path, body).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn combined(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn git(cwd: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env("GIT_AUTHOR_NAME", "kindling-test")
        .env("GIT_AUTHOR_EMAIL", "kindling-test@example.com")
        .env("GIT_COMMITTER_NAME", "kindling-test")
        .env("GIT_COMMITTER_EMAIL", "kindling-test@example.com")
        .output()
        .expect("git");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

const APS_SNIPPET: &str = "\
## Purpose
## Work Items
**Last reviewed:** 2026-01-01
| ID     | Owner  | Status      |
";

const CARGO_STUB: &str = r#"#!/bin/sh
printf 'cargo %s\n' "$*" >> "${STUB_DIR}/invocations"
args=" $* "
case "$args" in
  *" fmt "*)
    if [ "${FAIL_FMT:-0}" = "1" ]; then
      echo "Diff in src/lib.rs" >&2
      exit 1
    fi
    exit 0
    ;;
  *" clippy "*)
    if [ "${FAIL_CLIPPY:-0}" = "1" ]; then
      echo "clippy denied warnings" >&2
      exit 1
    fi
    echo "clippy ok"
    exit 0
    ;;
  *" --workspace "*)
    if [ "${FAIL_TEST:-0}" = "1" ]; then
      echo "test result: FAILED" >&2
      exit 1
    fi
    echo "test result: ok. 1 passed"
    exit 0
    ;;
  *" test "*)
    echo "running 1 test"
    echo "test result: ok. 1 passed; 0 failed"
    exit 0
    ;;
esac
echo "cargo stub unhandled: $*" >&2
exit 0
"#;

const PNPM_STUB: &str = r#"#!/bin/sh
printf 'pnpm %s\n' "$*" >> "${STUB_DIR}/invocations"
if [ "$1" = "install" ]; then
  count_file="${STUB_DIR}/install_count"
  n=0
  if [ -f "$count_file" ]; then
    n=$(cat "$count_file")
  fi
  n=$((n + 1))
  echo "$n" > "$count_file"
  if [ "${FAIL_INSTALL:-0}" = "1" ] && [ "$n" -ge 2 ]; then
    echo "pnpm install failed" >&2
    exit 1
  fi
  echo "Scope: all 7 workspace projects"
  echo "Packages: 7"
  echo "Done in 1.2s using 1 worker"
  echo "install stderr diagnostic" >&2
  exit 0
fi
if [ "${FAIL_BUILD:-0}" = "1" ] && [ "$1" = "run" ] && [ "$2" = "build" ]; then
  echo "pnpm run build failed" >&2
  exit 1
fi
echo "pnpm $* ok"
exit 0
"#;

struct CaptureFixture {
    _tmp: TempDir,
    root: PathBuf,
    stub_dir: PathBuf,
    scratch: PathBuf,
}

impl CaptureFixture {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        let stub_dir = tmp.path().join("stub");
        let scratch = tmp.path().join("scratch");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(root.join("scripts")).unwrap();
        fs::create_dir_all(root.join("plans/modules")).unwrap();
        fs::create_dir_all(root.join("packages/kindling")).unwrap();
        fs::create_dir_all(&stub_dir).unwrap();
        fs::create_dir_all(&scratch).unwrap();

        fs::copy(
            workspace_root().join("scripts/capture-release-evidence.sh"),
            root.join("scripts/capture-release-evidence.sh"),
        )
        .unwrap();
        write_exec(
            &root.join("scripts/sync-vendored-schema.sh"),
            "#!/bin/sh\necho 'vendored schema copies are up to date.'\n",
        );
        fs::write(
            root.join("plans/modules/06-downstream-integration-surface.aps.md"),
            APS_SNIPPET,
        )
        .unwrap();
        fs::write(
            root.join("plans/modules/08-conversion-surface.aps.md"),
            APS_SNIPPET,
        )
        .unwrap();
        fs::write(root.join("tsconfig.base.json"), "{}\n").unwrap();
        fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();
        fs::write(root.join("package.json"), "{}\n").unwrap();
        fs::write(root.join("CHANGELOG.md"), "# Changelog\n").unwrap();

        write_exec(&stub_dir.join("cargo"), CARGO_STUB);
        write_exec(&stub_dir.join("pnpm"), PNPM_STUB);
        write_exec(
            &stub_dir.join("node"),
            "#!/bin/sh\necho 'tsconfig clean'\nexit 0\n",
        );

        git(&root, &["init", "-q"]);
        git(&root, &["checkout", "-q", "-B", "main"]);
        fs::write(root.join("README.md"), "fixture\n").unwrap();
        git(&root, &["add", "README.md"]);
        git(
            &root,
            &[
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-q",
                "-m",
                "conversion surface",
            ],
        );

        Self {
            _tmp: tmp,
            root,
            stub_dir,
            scratch,
        }
    }

    fn run(&self, extra_env: &[(&str, &str)]) -> Output {
        let path = format!(
            "{}:{}",
            self.stub_dir.display(),
            std::env::var("PATH").unwrap()
        );
        let mut cmd = Command::new("bash");
        cmd.arg(self.root.join("scripts/capture-release-evidence.sh"))
            .current_dir(&self.root)
            .env("SCRATCH", &self.scratch)
            .env("PATH", &path)
            .env("STUB_DIR", &self.stub_dir)
            .env("FAIL_FMT", "0")
            .env("FAIL_CLIPPY", "0")
            .env("FAIL_TEST", "0")
            .env("FAIL_INSTALL", "0")
            .env("FAIL_BUILD", "0");
        for (key, value) in extra_env {
            cmd.env(key, value);
        }
        cmd.output().expect("run capture-release-evidence.sh")
    }

    fn read_scratch(&self, name: &str) -> String {
        fs::read_to_string(self.scratch.join(name)).unwrap_or_else(|e| {
            panic!("read {}: {e}", self.scratch.join(name).display());
        })
    }
}

#[test]
fn capture_succeeds_when_every_stubbed_check_passes() {
    let fx = CaptureFixture::new();
    let out = fx.run(&[]);
    assert!(
        out.status.success(),
        "expected success, got {:?}\n{}",
        out.status.code(),
        combined(&out)
    );
    let rust = fx.read_scratch("preflight-rust.txt");
    assert!(rust.contains("[fmt exit=0]"), "{rust}");
    assert!(rust.contains("[clippy exit=0]"), "{rust}");
    assert!(rust.contains("[test exit=0]"), "{rust}");
    assert!(rust.contains("[exit=0]"), "{rust}");
    let install = fx.read_scratch("pnpm-install-full.txt");
    assert!(
        install.contains("install stderr diagnostic"),
        "install stderr must be kept in the transcript:\n{install}"
    );
}

#[test]
fn capture_reports_fmt_failure_even_when_later_rust_checks_pass() {
    let fx = CaptureFixture::new();
    let out = fx.run(&[("FAIL_FMT", "1")]);
    assert!(
        !out.status.success(),
        "fmt failure must fail the script\n{}",
        combined(&out)
    );
    let rust = fx.read_scratch("preflight-rust.txt");
    assert!(rust.contains("[fmt exit=1]"), "{rust}");
    assert!(
        rust.contains("[clippy exit=0]"),
        "later checks must still be recorded:\n{rust}"
    );
    assert!(rust.contains("[test exit=0]"), "{rust}");
    assert!(rust.contains("[exit=1]"), "{rust}");
    let text = combined(&out);
    assert!(
        text.contains("Preflight commands failed") || text.contains("rust=1"),
        "{text}"
    );
}

#[test]
fn capture_reports_clippy_failure_even_when_later_tests_pass() {
    let fx = CaptureFixture::new();
    let out = fx.run(&[("FAIL_CLIPPY", "1")]);
    assert!(
        !out.status.success(),
        "clippy failure must fail the script\n{}",
        combined(&out)
    );
    let rust = fx.read_scratch("preflight-rust.txt");
    assert!(rust.contains("[clippy exit=1]"), "{rust}");
    assert!(rust.contains("[test exit=0]"), "{rust}");
    assert!(rust.contains("[exit=1]"), "{rust}");
}

#[test]
fn capture_reports_preflight_install_failure_even_when_later_pnpm_checks_pass() {
    let fx = CaptureFixture::new();
    let out = fx.run(&[("FAIL_INSTALL", "1")]);
    assert!(
        !out.status.success(),
        "preflight install failure must fail the script\n{}",
        combined(&out)
    );
    let npm = fx.read_scratch("preflight-npm.txt");
    assert!(npm.contains("[install exit=1]"), "{npm}");
    assert!(
        npm.contains("[build exit=0]"),
        "later pnpm phases must still be recorded:\n{npm}"
    );
    assert!(npm.contains("[exit=1]"), "{npm}");
    assert!(
        npm.contains("pnpm install failed"),
        "install diagnostics must be retained:\n{npm}"
    );
}

#[test]
fn capture_reports_build_failure_even_when_later_pnpm_checks_pass() {
    let fx = CaptureFixture::new();
    let out = fx.run(&[("FAIL_BUILD", "1")]);
    assert!(
        !out.status.success(),
        "build failure must fail the script\n{}",
        combined(&out)
    );
    let npm = fx.read_scratch("preflight-npm.txt");
    assert!(npm.contains("[build exit=1]"), "{npm}");
    assert!(npm.contains("[lint exit=0]"), "{npm}");
    assert!(npm.contains("[exit=1]"), "{npm}");
}

fn run_publish(stub_body: &str) -> Output {
    let tmp = tempfile::tempdir().unwrap();
    let stub_dir = tmp.path().join("stub");
    fs::create_dir_all(&stub_dir).unwrap();
    write_exec(&stub_dir.join("cargo"), stub_body);
    let path = format!("{}:{}", stub_dir.display(), std::env::var("PATH").unwrap());
    Command::new("sh")
        .arg(workspace_root().join("scripts/publish.sh"))
        .env("DRY_RUN", "1")
        .env("PATH", path)
        .output()
        .expect("run publish.sh")
}

#[test]
fn publish_dry_run_succeeds_when_every_crate_dry_run_succeeds() {
    let out = run_publish("#!/bin/sh\necho \"dry-run ok $*\"\nexit 0\n");
    assert!(
        out.status.success(),
        "expected success, got {:?}\n{}",
        out.status.code(),
        combined(&out)
    );
    let text = combined(&out);
    assert!(text.contains("done."), "{text}");
    assert!(!text.contains("unexpected dry-run failure"), "{text}");
}

#[test]
fn publish_dry_run_treats_missing_crates_io_deps_as_expected() {
    let out = run_publish(
        "#!/bin/sh\necho 'error: no matching package named `kindling-types` found' >&2\nexit 1\n",
    );
    assert!(
        out.status.success(),
        "dependency-resolution failures are expected until deps are on crates.io\n{}",
        combined(&out)
    );
    let text = combined(&out);
    assert!(
        text.contains("may fail until its deps are on crates.io"),
        "{text}"
    );
    assert!(text.contains("done."), "{text}");
}

#[test]
fn publish_dry_run_fails_on_unexpected_cargo_errors() {
    let out = run_publish("#!/bin/sh\necho 'error: failed to compile crate' >&2\nexit 101\n");
    assert!(
        !out.status.success(),
        "compile failures must not be converted into a successful dry-run\n{}",
        combined(&out)
    );
    let text = combined(&out);
    assert!(text.contains("unexpected dry-run failure"), "{text}");
    assert!(text.contains("failed to compile crate"), "{text}");
    assert!(!text.contains("done."), "{text}");
}

#[test]
fn publish_dry_run_does_not_mask_a_compile_error_after_an_expected_dep_miss() {
    let out = run_publish(
        r#"#!/bin/sh
crate=""
prev=""
for arg in "$@"; do
  if [ "$prev" = "-p" ]; then
    crate="$arg"
  fi
  prev="$arg"
done
case "$crate" in
  kindling-types)
    echo "ok"
    exit 0
    ;;
  kindling-store)
    echo "error: no matching package named \`kindling-types\` found" >&2
    exit 1
    ;;
  *)
    echo "error: failed to compile $crate" >&2
    exit 101
    ;;
esac
"#,
    );
    assert!(
        !out.status.success(),
        "an unexpected cargo error after an expected dep miss must still fail\n{}",
        combined(&out)
    );
    let text = combined(&out);
    assert!(
        text.contains("may fail until its deps are on crates.io"),
        "{text}"
    );
    assert!(text.contains("unexpected dry-run failure"), "{text}");
}
