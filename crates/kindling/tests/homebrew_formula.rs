//! Regression tests for `scripts/generate-homebrew-formula.sh` (GH-192).
//!
//! The generator is invoked on Linux CI and by maintainers on macOS. Default
//! tag resolution must not capture diagnostic lines, and formula rewrites must
//! not depend on bash 4 associative arrays or GNU sed.

#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

const LATEST_TAG: &str = "v9.9.9";
const LATEST_VERSION: &str = "9.9.9";

const SHA_AARCH64_APPLE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const SHA_X86_64_APPLE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const SHA_AARCH64_LINUX: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const SHA_X86_64_LINUX: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";

const SHA_AARCH64_APPLE_RERUN: &str =
    "1111111111111111111111111111111111111111111111111111111111111111";
const SHA_X86_64_APPLE_RERUN: &str =
    "2222222222222222222222222222222222222222222222222222222222222222";
const SHA_AARCH64_LINUX_RERUN: &str =
    "3333333333333333333333333333333333333333333333333333333333333333";
const SHA_X86_64_LINUX_RERUN: &str =
    "4444444444444444444444444444444444444444444444444444444444444444";

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn script_path() -> PathBuf {
    workspace_root().join("scripts/generate-homebrew-formula.sh")
}

fn template_formula() -> String {
    fs::read_to_string(workspace_root().join("packaging/homebrew/kindling.rb"))
        .expect("packaging/homebrew/kindling.rb")
}

fn find_bin(name: &str) -> PathBuf {
    let path = std::env::var("PATH").expect("PATH");
    for dir in path.split(':') {
        let candidate = Path::new(dir).join(name);
        if candidate.is_file() {
            return candidate;
        }
    }
    panic!("{name} not found on PATH");
}

fn write_executable(path: &Path, contents: &str) {
    fs::write(path, contents).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    let mut perms = fs::metadata(path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).unwrap();
}

/// Wrap the host `sed` so GNU-only `sed -i` (no backup suffix) and `+N`
/// addresses fail. Portable `sed -n` / `sed -E` still reach the real binary.
fn install_bsd_sed_shim(bin_dir: &Path) {
    let real_sed = find_bin("sed");
    write_executable(
        &bin_dir.join("sed"),
        &format!(
            "#!/bin/sh\n\
             for arg in \"$@\"; do\n\
               case \"$arg\" in\n\
                 -i)\n\
                   printf '%s\\n' 'error: BSD sed requires an extension with -i' >&2\n\
                   exit 2\n\
                   ;;\n\
                 *,+*)\n\
                   printf '%s\\n' 'error: GNU sed address +N is not portable' >&2\n\
                   exit 2\n\
                   ;;\n\
               esac\n\
             done\n\
             exec {} \"$@\"\n",
            real_sed.display()
        ),
    );
}

fn mock_curl_script(log_path: &Path, version: &str, hashes: [&str; 4]) -> String {
    format!(
        r#"#!/bin/sh
url=""
for arg in "$@"; do
  case "$arg" in
    -*) ;;
    *) url="$arg" ;;
  esac
done
printf '%s\n' "$url" >> "{log}"
case "$url" in
  *releases?per_page=1*)
    printf '%s\n' '{{"tag_name": "{tag}", "name": "{tag}"}}'
    ;;
  *kindling-{version}-aarch64-apple-darwin.tar.gz.sha256)
    printf '%s\n' '{sha0}  kindling-{version}-aarch64-apple-darwin.tar.gz'
    ;;
  *kindling-{version}-x86_64-apple-darwin.tar.gz.sha256)
    printf '%s\n' '{sha1}  kindling-{version}-x86_64-apple-darwin.tar.gz'
    ;;
  *kindling-{version}-aarch64-unknown-linux-gnu.tar.gz.sha256)
    printf '%s\n' '{sha2}  kindling-{version}-aarch64-unknown-linux-gnu.tar.gz'
    ;;
  *kindling-{version}-x86_64-unknown-linux-gnu.tar.gz.sha256)
    printf '%s\n' '{sha3}  kindling-{version}-x86_64-unknown-linux-gnu.tar.gz'
    ;;
  *)
    printf 'unexpected curl url: %s\n' "$url" >&2
    exit 1
    ;;
esac
"#,
        log = log_path.display(),
        tag = format!("v{version}"),
        version = version,
        sha0 = hashes[0],
        sha1 = hashes[1],
        sha2 = hashes[2],
        sha3 = hashes[3],
    )
}

struct Harness {
    dir: TempDir,
    formula: PathBuf,
    curl_log: PathBuf,
    bin_dir: PathBuf,
}

impl Harness {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let formula = dir.path().join("kindling.rb");
        fs::write(&formula, template_formula()).unwrap();
        let bin_dir = dir.path().join("bin");
        fs::create_dir(&bin_dir).unwrap();
        install_bsd_sed_shim(&bin_dir);
        let curl_log = dir.path().join("curl.log");
        Self {
            dir,
            formula,
            curl_log,
            bin_dir,
        }
    }

    fn install_curl(&self, version: &str, hashes: [&str; 4]) {
        write_executable(
            &self.bin_dir.join("curl"),
            &mock_curl_script(&self.curl_log, version, hashes),
        );
    }

    fn run(&self, args: &[&str], extra_env: &[(&str, &str)]) -> Output {
        let mut path = self.bin_dir.as_os_str().to_os_string();
        path.push(":");
        path.push(std::env::var_os("PATH").unwrap());

        let mut cmd = Command::new("bash");
        cmd.arg(script_path())
            .args(args)
            .env("PATH", path)
            .env("KINDLING_FORMULA", &self.formula)
            .env_remove("KINDLING_VERSION")
            .env_remove("KINDLING_TAP_DIR");
        for (k, v) in extra_env {
            cmd.env(k, v);
        }
        cmd.output().expect("run generate-homebrew-formula.sh")
    }

    fn formula_text(&self) -> String {
        fs::read_to_string(&self.formula).unwrap()
    }

    fn curl_urls(&self) -> String {
        fs::read_to_string(&self.curl_log).unwrap_or_default()
    }
}

fn assert_success(out: &Output) {
    assert!(
        out.status.success(),
        "generator failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn assert_formula_checksums(text: &str, version: &str, hashes: [&str; 4]) {
    assert!(
        text.contains(&format!("version \"{version}\"")),
        "missing version {version} in:\n{text}"
    );
    assert!(
        !text.contains("REPLACE_WITH_SHA256"),
        "placeholders remain:\n{text}"
    );
    assert!(
        !text.contains("==>"),
        "diagnostic marker leaked into the formula:\n{text}"
    );
    assert!(text.contains(&format!("sha256 \"{}\"", hashes[0])));
    assert!(text.contains(&format!("sha256 \"{}\"", hashes[1])));
    assert!(text.contains(&format!("sha256 \"{}\"", hashes[2])));
    assert!(text.contains(&format!("sha256 \"{}\"", hashes[3])));
}

fn uncommented_source(src: &str) -> String {
    src.lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn generator_avoids_macos_incompatible_constructs() {
    let src = uncommented_source(&fs::read_to_string(script_path()).unwrap());
    assert!(
        !src.contains("declare -A"),
        "associative arrays need bash 4; macOS ships bash 3.2"
    );
    assert!(
        !src.contains("sed -i"),
        "GNU sed -i without a backup suffix fails on BSD sed"
    );
    assert!(
        !src.contains(",+1"),
        "GNU sed address +N is not portable to BSD sed"
    );
}

#[test]
fn default_tag_resolution_does_not_capture_diagnostics() {
    let h = Harness::new();
    h.install_curl(
        LATEST_VERSION,
        [
            SHA_AARCH64_APPLE,
            SHA_X86_64_APPLE,
            SHA_AARCH64_LINUX,
            SHA_X86_64_LINUX,
        ],
    );

    let out = h.run(&[], &[]);
    assert_success(&out);

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Resolving latest release tag"),
        "expected resolve diagnostic on stderr, got:\n{stderr}"
    );
    assert!(
        !stdout.contains("Resolving latest release tag"),
        "resolve diagnostic leaked onto stdout:\n{stdout}"
    );
    assert!(
        stderr.contains(&format!("Release tag: {LATEST_TAG}")),
        "expected clean tag on stderr, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("diagnostics mixed into stdout"),
        "tag sanity check fired:\n{stderr}"
    );

    let formula = h.formula_text();
    assert_formula_checksums(
        &formula,
        LATEST_VERSION,
        [
            SHA_AARCH64_APPLE,
            SHA_X86_64_APPLE,
            SHA_AARCH64_LINUX,
            SHA_X86_64_LINUX,
        ],
    );

    let urls = h.curl_urls();
    assert!(
        urls.contains(&format!(
            "/releases/download/{LATEST_TAG}/kindling-{LATEST_VERSION}-aarch64-apple-darwin.tar.gz.sha256"
        )),
        "checksum URL used a contaminated tag:\n{urls}"
    );
    assert!(
        !urls.contains("==>"),
        "diagnostic marker leaked into curl URLs:\n{urls}"
    );
}

#[test]
fn explicit_tag_rewrites_placeholders_and_existing_checksums() {
    let h = Harness::new();
    h.install_curl(
        LATEST_VERSION,
        [
            SHA_AARCH64_APPLE,
            SHA_X86_64_APPLE,
            SHA_AARCH64_LINUX,
            SHA_X86_64_LINUX,
        ],
    );

    let first = h.run(&[LATEST_TAG], &[]);
    assert_success(&first);
    assert_formula_checksums(
        &h.formula_text(),
        LATEST_VERSION,
        [
            SHA_AARCH64_APPLE,
            SHA_X86_64_APPLE,
            SHA_AARCH64_LINUX,
            SHA_X86_64_LINUX,
        ],
    );

    // Re-run with new sidecars: the GNU `/,+1` path used to update hashes that
    // were already filled in. awk must do the same on BSD sed.
    fs::write(&h.curl_log, "").unwrap();
    h.install_curl(
        LATEST_VERSION,
        [
            SHA_AARCH64_APPLE_RERUN,
            SHA_X86_64_APPLE_RERUN,
            SHA_AARCH64_LINUX_RERUN,
            SHA_X86_64_LINUX_RERUN,
        ],
    );
    let second = h.run(&[LATEST_TAG], &[]);
    assert_success(&second);
    assert_formula_checksums(
        &h.formula_text(),
        LATEST_VERSION,
        [
            SHA_AARCH64_APPLE_RERUN,
            SHA_X86_64_APPLE_RERUN,
            SHA_AARCH64_LINUX_RERUN,
            SHA_X86_64_LINUX_RERUN,
        ],
    );
}

#[test]
fn sync_tap_copies_the_updated_formula() {
    let h = Harness::new();
    h.install_curl(
        LATEST_VERSION,
        [
            SHA_AARCH64_APPLE,
            SHA_X86_64_APPLE,
            SHA_AARCH64_LINUX,
            SHA_X86_64_LINUX,
        ],
    );
    let tap = h.dir.path().join("homebrew-tap");
    let out = h.run(
        &[LATEST_TAG, "--sync-tap"],
        &[("KINDLING_TAP_DIR", tap.to_str().unwrap())],
    );
    assert_success(&out);
    let copied = fs::read_to_string(tap.join("Formula/kindling.rb")).unwrap();
    assert_formula_checksums(
        &copied,
        LATEST_VERSION,
        [
            SHA_AARCH64_APPLE,
            SHA_X86_64_APPLE,
            SHA_AARCH64_LINUX,
            SHA_X86_64_LINUX,
        ],
    );
}
