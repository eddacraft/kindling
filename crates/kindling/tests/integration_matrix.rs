//! KFIT-002 integration-matrix checks: every integration the README or
//! `docs/integrations.md` calls Supported must name a test that exercises it,
//! the matrix links must resolve, and the Claude Code plugin's manifests must
//! agree with each other and with what `kindling init --claude-code` prints.
//!
//! These read the docs as written, so a row cannot be promoted to Supported
//! (or a test renamed away) without this suite noticing.

mod support;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use kindling::hook::HookType;
use serde_json::Value;
use support::{assert_success, json_stdout, CliEnv};

/// Statuses the docs define. Anything else is a typo or an undefined promise.
const KNOWN_STATUSES: &[&str] = &["Supported", "Planned", "Via CLI only"];

/// Evidence for each Supported integration: a test file (from the workspace
/// root) and a string that names a test in it. Each test drives the real
/// journey — through the daemon where the integration uses one.
const EVIDENCE: &[(&str, &[(&str, &str)])] = &[
    (
        "Plain CLI",
        &[
            (
                "crates/kindling/tests/cli.rs",
                "fn log_then_status_counts_observation",
            ),
            (
                "crates/kindling/tests/cli.rs",
                "fn search_finds_logged_observation",
            ),
            (
                "crates/kindling/tests/cli.rs",
                "fn browse_writes_html_without_opening_browser",
            ),
        ],
    ),
    (
        "Claude Code",
        &[
            (
                "crates/kindling/tests/e2e.rs",
                "fn post_tool_use_reads_claude_code_tool_response",
            ),
            (
                "crates/kindling/tests/e2e.rs",
                "fn post_tool_use_failure_lands_claude_code_error",
            ),
            (
                "crates/kindling/tests/e2e.rs",
                "fn session_start_emits_injection_envelope",
            ),
            (
                "crates/kindling/tests/cli.rs",
                "fn init_claude_code_reports_plugin_install_when_detected",
            ),
            (
                "plugins/kindling-claude-code/test/memory-commands.test.js",
                "'memory-search finds seeded content'",
            ),
        ],
    ),
    (
        "VS Code / Cursor / Windsurf",
        &[(
            "packages/kindling-adapter-vscode/test/session.live.spec.ts",
            "'VS Code capture journey (live daemon)'",
        )],
    ),
    (
        "OpenCode",
        &[(
            "packages/kindling-adapter-opencode/test/session.spec.ts",
            "'SessionManager (live daemon)'",
        )],
    ),
    (
        "PocketFlow",
        &[(
            "packages/kindling-adapter-pocketflow/test/lifecycle.spec.ts",
            "'KindlingNode (live daemon)'",
        )],
    ),
    (
        "Rust SDK",
        &[(
            "crates/kindling-client/tests/client.rs",
            "fn warm_call_round_trip_all_methods",
        )],
    ),
    (
        "Rust SDK (embedded)",
        &[(
            "crates/kindling-service/tests/service.rs",
            "fn open_capsule_happy_path",
        )],
    ),
    (
        "Node client",
        &[(
            "packages/kindling/test/client.spec.ts",
            "'Kindling thin client — warm round-trip'",
        )],
    ),
    (
        "Custom adapter",
        &[(
            "examples/adapter-minimal/test/example.test.js",
            "'minimal adapter example completes its journey against a live daemon'",
        )],
    ),
];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn read(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path.as_ref())
        .unwrap_or_else(|e| panic!("read {}: {e}", path.as_ref().display()))
}

fn read_json(path: impl AsRef<Path>) -> Value {
    let text = read(path.as_ref());
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", path.as_ref().display()))
}

/// One row of an integrations table.
#[derive(Debug)]
struct Row {
    name: String,
    status: String,
    /// The raw row, for link checks.
    line: String,
}

/// Replace `[text](target)` with `text` and drop `**` emphasis.
fn plain(cell: &str) -> String {
    let mut out = String::new();
    let mut rest = cell;
    while let Some(open) = rest.find('[') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find("](").and_then(|close| {
            after[close + 2..]
                .find(')')
                .map(|end| (close, close + 2 + end))
        }) {
            Some((close, end)) => {
                out.push_str(&after[..close]);
                rest = &after[end + 1..];
            }
            None => {
                out.push('[');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out.replace("**", "").trim().to_string()
}

/// The link targets in a markdown fragment.
fn link_targets(text: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("](") {
        let after = &rest[start + 2..];
        match after.find(')') {
            Some(end) => {
                targets.push(after[..end].to_string());
                rest = &after[end + 1..];
            }
            None => break,
        }
    }
    targets
}

/// The first table under `heading` in `doc`, minus its header and separator.
fn table_under(doc: &str, heading: &str) -> Vec<Row> {
    let mut lines = doc.lines().skip_while(|l| l.trim() != heading);
    assert!(lines.next().is_some(), "heading `{heading}` not found");
    let table: Vec<&str> = lines
        .skip_while(|l| !l.trim_start().starts_with('|'))
        .take_while(|l| l.trim_start().starts_with('|'))
        .collect();
    assert!(table.len() > 2, "no table rows under `{heading}`");
    table[2..]
        .iter()
        .map(|line| {
            let cells: Vec<&str> = line.trim().trim_matches('|').split('|').collect();
            assert!(cells.len() >= 2, "malformed row: {line}");
            Row {
                name: plain(cells[0]),
                status: plain(cells[1]),
                line: (*line).to_string(),
            }
        })
        .collect()
}

/// Both integration tables, each paired with the directory its links resolve from.
fn tables() -> Vec<(PathBuf, Vec<Row>)> {
    let root = workspace_root();
    vec![
        (
            root.clone(),
            table_under(&read(root.join("README.md")), "## Integrations"),
        ),
        (
            root.join("docs"),
            table_under(&read(root.join("docs/integrations.md")), "## Matrix"),
        ),
    ]
}

fn supported_names() -> BTreeSet<String> {
    tables()
        .into_iter()
        .flat_map(|(_, rows)| rows)
        .filter(|row| row.status == "Supported")
        .map(|row| row.name)
        .collect()
}

#[test]
fn integration_statuses_are_defined() {
    for (_, rows) in tables() {
        for row in rows {
            assert!(
                KNOWN_STATUSES.contains(&row.status.as_str()),
                "`{}` has undefined status `{}` (known: {KNOWN_STATUSES:?})",
                row.name,
                row.status
            );
        }
    }
}

#[test]
fn every_supported_integration_names_a_test_that_exists() {
    let root = workspace_root();
    for name in supported_names() {
        let evidence = EVIDENCE
            .iter()
            .find(|(entry, _)| *entry == name)
            .map(|(_, evidence)| *evidence)
            .unwrap_or_else(|| {
                panic!("`{name}` is Supported but has no evidence entry in this suite")
            });
        assert!(!evidence.is_empty(), "`{name}` has an empty evidence list");
        for (file, needle) in evidence {
            let path = root.join(file);
            assert!(
                path.is_file(),
                "`{name}` evidence file missing: {}",
                path.display()
            );
            assert!(
                read(&path).contains(needle),
                "`{name}` evidence `{needle}` not found in {file}"
            );
        }
    }
}

#[test]
fn evidence_registry_has_no_stale_entries() {
    let supported = supported_names();
    for (name, _) in EVIDENCE {
        assert!(
            supported.contains(*name),
            "evidence registered for `{name}`, which neither doc lists as Supported"
        );
    }
}

#[test]
fn integration_table_links_resolve() {
    for (base, rows) in tables() {
        for row in rows {
            for target in link_targets(&row.line) {
                if target.starts_with("http://")
                    || target.starts_with("https://")
                    || target.starts_with('#')
                {
                    continue;
                }
                let path = target.split('#').next().unwrap_or_default();
                let resolved = base.join(path);
                assert!(
                    resolved.exists(),
                    "`{}` links to `{target}`, which does not resolve from {} ({})",
                    row.name,
                    base.display(),
                    resolved.display()
                );
            }
        }
    }
}

/// The marketplace entry, the plugin directory it points to, and that
/// directory's `plugin.json`.
fn plugin_manifests() -> (Value, Value, PathBuf, Value) {
    let root = workspace_root();
    let marketplace = read_json(root.join(".claude-plugin/marketplace.json"));
    let plugins = marketplace["plugins"].as_array().expect("plugins array");
    assert_eq!(
        plugins.len(),
        1,
        "expected the single kindling plugin entry"
    );
    let entry = plugins[0].clone();
    let source = entry["source"].as_str().expect("plugin source path");
    let dir = root.join(source);
    let plugin = read_json(dir.join(".claude-plugin/plugin.json"));
    (marketplace, entry, dir, plugin)
}

#[test]
fn claude_code_plugin_manifests_agree() {
    let (marketplace, entry, dir, plugin) = plugin_manifests();
    assert_eq!(entry["name"], plugin["name"], "plugin name differs");

    let hooks = plugin["hooks"].as_str().expect("plugin.json hooks path");
    assert!(
        dir.join(hooks).is_file(),
        "plugin.json hooks path `{hooks}` does not exist"
    );

    let package = read_json(dir.join("package.json"));
    let version = package["version"].as_str().expect("package.json version");
    for (label, value) in [
        ("plugin.json", &plugin["version"]),
        ("marketplace plugin entry", &entry["version"]),
        ("marketplace metadata", &marketplace["metadata"]["version"]),
    ] {
        assert_eq!(
            value.as_str(),
            Some(version),
            "{label} version disagrees with package.json ({version})"
        );
    }
}

#[test]
fn claude_code_plugin_hooks_call_real_hook_types() {
    let (_, _, dir, plugin) = plugin_manifests();
    let hooks = read_json(dir.join(plugin["hooks"].as_str().unwrap()));
    let events = hooks["hooks"].as_object().expect("hooks map");
    assert!(!events.is_empty(), "hooks.json registers no hooks");
    let mut seen = 0;
    for (event, groups) in events {
        for group in groups.as_array().expect("hook groups") {
            for hook in group["hooks"].as_array().expect("hook list") {
                let command = hook["command"].as_str().expect("hook command");
                let kind = command.strip_prefix("kindling hook ").unwrap_or_else(|| {
                    panic!("{event}: `{command}` is not `kindling hook <type>`")
                });
                HookType::parse(kind)
                    .unwrap_or_else(|e| panic!("{event}: `{command}` names no hook type: {e}"));
                seen += 1;
            }
        }
    }
    assert!(seen > 0, "hooks.json has no hook commands");
}

#[test]
fn init_claude_code_prints_the_marketplace_install_commands() {
    let (marketplace, entry, _, _) = plugin_manifests();
    let repo = entry["repository"]
        .as_str()
        .and_then(|url| url.strip_prefix("https://github.com/"))
        .expect("plugin repository is a GitHub URL");
    let expected = [
        format!("/plugin marketplace add {repo}"),
        format!(
            "/plugin install {}@{}",
            entry["name"].as_str().unwrap(),
            marketplace["name"].as_str().unwrap()
        ),
    ];

    let env = CliEnv::new();
    let home = env.path("home");
    fs::create_dir_all(home.join(".claude")).unwrap();
    let out = env.run_db_with_env(
        &["init", "--claude-code", "--skip-db", "--json"],
        &[("HOME", &home), ("USERPROFILE", &home), ("PATH", &home)],
    );
    assert_success(&out);
    let printed: Vec<String> = json_stdout(&out)["claudeCode"]["installCommands"]
        .as_array()
        .expect("installCommands array")
        .iter()
        .map(|c| c.as_str().unwrap().to_string())
        .collect();
    assert_eq!(printed, expected);
}

#[test]
fn table_parsing_strips_links_and_emphasis() {
    assert_eq!(plain("**Claude Code**"), "Claude Code");
    assert_eq!(plain("[Claude Code](https://example.com/x)"), "Claude Code");
    assert_eq!(plain("**Via CLI only**"), "Via CLI only");
    assert_eq!(
        link_targets("a [b](../x/) c [d](./y.md#z)"),
        vec!["../x/".to_string(), "./y.md#z".to_string()]
    );
}

/// The `major.minor` the workspace publishes, e.g. `0.4` for `0.4.0`.
fn workspace_minor() -> String {
    let manifest = read(workspace_root().join("Cargo.toml"));
    let version = manifest
        .lines()
        .skip_while(|l| l.trim() != "[workspace.package]")
        .find_map(|l| l.trim().strip_prefix("version = "))
        .expect("[workspace.package] version")
        .trim_matches('"');
    let mut parts = version.split('.');
    format!("{}.{}", parts.next().unwrap(), parts.next().unwrap())
}

#[test]
fn readme_dependency_pins_match_the_workspace_version() {
    let root = workspace_root();
    let minor = workspace_minor();
    let mut readmes = vec![root.join("README.md")];
    for entry in fs::read_dir(root.join("crates")).unwrap() {
        let readme = entry.unwrap().path().join("README.md");
        if readme.is_file() {
            readmes.push(readme);
        }
    }
    for readme in readmes {
        for line in read(&readme).lines() {
            let line = line.trim_start_matches(['#', ' ']);
            let Some((name, spec)) = line.split_once(" = ") else {
                continue;
            };
            if !name.starts_with("kindling") && !name.starts_with("eddacraft-kindling") {
                continue;
            }
            let Some(pin) = spec.split('"').nth(1) else {
                continue;
            };
            assert_eq!(
                pin,
                minor,
                "{}: `{name}` is pinned to {pin}, but the workspace publishes {minor}",
                readme.display()
            );
        }
    }
}
