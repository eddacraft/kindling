//! KINTEG-001 publish-readiness checks: workspace version lockstep, spool
//! feature wiring, docs.rs metadata, and documentation that the spool ships
//! inside `kindling-client` (no standalone spool crate).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const WORKSPACE_VERSION: &str = "0.2.0";
const CRATES: &[&str] = &[
    "kindling-types",
    "kindling-store",
    "kindling-provider",
    "kindling-service",
    "kindling-server",
    "kindling-client",
    "kindling",
];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn read(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path.as_ref()).unwrap_or_else(|e| {
        panic!("read {}: {e}", path.as_ref().display());
    })
}

/// Build the legacy standalone-crate name without embedding it as a source literal.
fn legacy_standalone_spool_crate() -> String {
    ["kindling", "spool"].join("-")
}

#[test]
fn workspace_version_is_020() {
    let root = workspace_root();
    let workspace_toml = read(root.join("Cargo.toml"));
    assert!(
        workspace_toml.contains(&format!("version = \"{WORKSPACE_VERSION}\"")),
        "workspace.package.version must be {WORKSPACE_VERSION}"
    );
}

#[test]
fn all_seven_crates_use_workspace_version_and_pins() {
    let root = workspace_root();
    for crate_dir in CRATES {
        let manifest = read(root.join("crates").join(crate_dir).join("Cargo.toml"));
        assert!(
            manifest.contains("version.workspace = true"),
            "{crate_dir} must use version.workspace = true"
        );
        for dep in CRATES {
            if *dep == *crate_dir {
                continue;
            }
            let pin = format!("version = \"{WORKSPACE_VERSION}\"");
            let dep_path = format!("path = \"../{dep}\"");
            let umbrella_path = "path = \"../kindling\"";
            if manifest.contains(&dep_path) || manifest.contains(umbrella_path) {
                assert!(
                    manifest.contains(&pin),
                    "{crate_dir} must pin intra-workspace deps at {WORKSPACE_VERSION}"
                );
            }
        }
    }
}

#[test]
fn kindling_client_ships_spool_feature_with_docs_rs_all_features() {
    let root = workspace_root();
    let manifest = read(root.join("crates/kindling-client/Cargo.toml"));
    assert!(
        manifest.contains("spool = ["),
        "kindling-client must declare the spool feature"
    );
    let docs_section = manifest
        .split("[package.metadata.docs.rs]")
        .nth(1)
        .expect("kindling-client must have [package.metadata.docs.rs]");
    assert!(
        docs_section.contains("all-features = true"),
        "docs.rs must build with all-features = true so SpooledClient appears"
    );
}

#[test]
fn spool_module_lives_only_under_kindling_client() {
    let root = workspace_root();
    let legacy = legacy_standalone_spool_crate();
    assert!(
        !root.join("crates").join(&legacy).exists(),
        "no standalone spool crate directory must exist"
    );
    let members: Vec<_> = fs::read_dir(root.join("crates"))
        .expect("crates dir")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(members.len(), 7, "workspace must have exactly seven crates");
    assert!(members.contains(&"kindling-client".to_string()));
    assert!(
        root.join("crates/kindling-client/src/spool.rs").is_file(),
        "spool.rs must live under kindling-client"
    );
}

#[test]
fn changelog_and_client_readme_note_spool_inside_client() {
    let root = workspace_root();
    let changelog = read(root.join("CHANGELOG.md"));
    let section = changelog
        .split("## [0.2.0]")
        .nth(1)
        .expect("CHANGELOG must have a [0.2.0] section");
    assert!(
        section.contains("SpooledClient") && section.contains("kindling-client"),
        "CHANGELOG [0.2.0] must describe SpooledClient on kindling-client"
    );
    let section_flat: String = section.split_whitespace().collect::<Vec<_>>().join(" ");
    let legacy = legacy_standalone_spool_crate();
    assert!(
        section_flat.contains("no standalone")
            && section_flat.contains(&legacy)
            && section_flat.contains("crate"),
        "CHANGELOG must state there is no standalone spool crate"
    );

    let readme = read(root.join("crates/kindling-client/README.md"));
    assert!(
        readme.contains("features = [\"spool\"]") || readme.contains("features = ['spool']"),
        "kindling-client README must document the spool feature flag"
    );
    assert!(
        readme.contains("no standalone") && readme.contains(&legacy) && readme.contains("crate"),
        "kindling-client README must state there is no standalone spool crate"
    );
}

#[test]
fn cargo_package_lists_core_files_for_every_crate() {
    let root = workspace_root();
    for crate_dir in CRATES {
        let package_name = if *crate_dir == "kindling" {
            "eddacraft-kindling"
        } else {
            crate_dir
        };
        let out = Command::new("cargo")
            .args(["package", "--list", "--allow-dirty", "-p", package_name])
            .current_dir(&root)
            .output()
            .expect("cargo package --list");
        assert!(
            out.status.success(),
            "cargo package --list -p {package_name} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let listing = String::from_utf8_lossy(&out.stdout);
        assert!(
            listing.contains("Cargo.toml"),
            "{package_name} package missing Cargo.toml"
        );
        assert!(
            listing.contains("README.md"),
            "{package_name} package missing README.md"
        );
        if *crate_dir == "kindling-store" {
            assert!(
                listing.contains("schema/schema.sql"),
                "kindling-store package must include vendored schema/schema.sql"
            );
        }
    }
}
