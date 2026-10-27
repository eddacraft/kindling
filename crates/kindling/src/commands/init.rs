//! `init` — create the database (and optionally report Claude Code setup).
//!
//! Ports `packages/kindling-cli/src/commands/init.ts` for the directory +
//! database steps. The `--json` shape keeps `{ directory, database, claudeCode }`.
//!
//! Claude Code step: the capture hooks ship as a Claude Code plugin
//! (`plugins/kindling-claude-code/`) published through this repository's
//! marketplace (`.claude-plugin/marketplace.json`), and its hooks run
//! `kindling hook <type>`. A plugin is installed from inside Claude Code, so
//! `--claude-code` does not edit `~/.claude/`. It reports whether Claude Code is
//! detected, whether the `kindling` binary the hooks call is on `PATH`, and the
//! exact `/plugin` commands that install the plugin. `configured` therefore
//! stays `false`: init never claims an install it did not perform.

use std::path::PathBuf;

use serde::Serialize;

use crate::cli::InitArgs;
use crate::output::format_json;
use crate::{resolve_db_path, CliResult};

#[derive(Serialize)]
struct InitOutput {
    directory: DirectoryResult,
    database: Option<DatabaseResult>,
    #[serde(rename = "claudeCode")]
    claude_code: Option<ClaudeCodeResult>,
}

#[derive(Serialize)]
struct DirectoryResult {
    created: bool,
    path: String,
}

#[derive(Serialize)]
struct DatabaseResult {
    created: bool,
    path: String,
    existed: bool,
}

#[derive(Serialize)]
struct ClaudeCodeResult {
    configured: bool,
    /// Whether `~/.claude/` exists.
    detected: bool,
    /// Whether a `kindling` executable resolves on `PATH`; the plugin's hooks
    /// run `kindling hook <type>` and no-op without it.
    #[serde(rename = "binaryOnPath")]
    binary_on_path: bool,
    #[serde(rename = "pluginPath")]
    plugin_path: String,
    /// The Claude Code commands that install the plugin, in order.
    #[serde(rename = "installCommands")]
    install_commands: Vec<String>,
    message: String,
}

/// The commands that add this repository's marketplace and install the plugin
/// from it. The names match `.claude-plugin/marketplace.json`.
const PLUGIN_INSTALL_COMMANDS: [&str; 2] = [
    "/plugin marketplace add eddacraft/kindling",
    "/plugin install kindling@kindling-plugins",
];

pub fn run(args: InitArgs) -> CliResult {
    let db_path = resolve_db_path(args.db.as_deref())?;
    // Validate before mkdir: `--skip-db` never opens the store, but must not
    // create parent directories from a traversal / SQLite-URI path.
    kindling_store::validate_db_path(&db_path)?;
    let kindling_dir = db_path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    // Step 1: directory.
    let dir_existed = kindling_dir.exists();
    if !dir_existed {
        std::fs::create_dir_all(&kindling_dir)?;
    }
    let directory = DirectoryResult {
        created: !dir_existed,
        path: kindling_dir.to_string_lossy().into_owned(),
    };

    // Step 2: database (unless --skip-db). Opening the store runs migrations and
    // creates the file, matching the TS `openDatabase` behaviour.
    let database = if args.skip_db {
        None
    } else {
        let db_existed = db_path.exists();
        // Opening + dropping the service initializes the schema.
        let _service = kindling_service::KindlingService::open(&db_path)?;
        Some(DatabaseResult {
            created: !db_existed,
            path: db_path.to_string_lossy().into_owned(),
            existed: db_existed,
        })
    };

    // Step 3: Claude Code (stubbed — see module docs / PORT-015).
    let claude_code = if args.claude_code {
        Some(configure_claude_code())
    } else {
        None
    };

    let output = InitOutput {
        directory,
        database,
        claude_code,
    };

    if args.json {
        println!("{}", format_json(&output, true)?);
    } else {
        print_human(&output, args.claude_code);
    }
    Ok(())
}

/// Report Claude Code setup: detection, the binary the hooks call, and the
/// plugin install commands. Never edits Claude Code's own configuration.
fn configure_claude_code() -> ClaudeCodeResult {
    let detected = home_dir()
        .map(|h| h.join(".claude"))
        .is_some_and(|dir| dir.is_dir());
    let binary_on_path = kindling_on_path();
    // The commands themselves travel in `install_commands`; the message only
    // says what state was found, so human output lists each command once.
    let message = match (detected, binary_on_path) {
        (true, true) => {
            "Claude Code detected. Install the kindling plugin from inside Claude Code."
        }
        (true, false) => {
            "Claude Code detected, but `kindling` is not on PATH, so the plugin's hooks \
             would no-op. Add the binary to PATH, then install the plugin from inside \
             Claude Code."
        }
        (false, _) => {
            "Claude Code not detected (~/.claude/ does not exist). After installing \
             Claude Code, install the kindling plugin from inside it."
        }
    }
    .to_string();
    ClaudeCodeResult {
        configured: false,
        detected,
        binary_on_path,
        plugin_path: String::new(),
        install_commands: PLUGIN_INSTALL_COMMANDS
            .iter()
            .map(|c| c.to_string())
            .collect(),
        message,
    }
}

/// Whether an executable named `kindling` resolves on `PATH`.
fn kindling_on_path() -> bool {
    let names: &[&str] = if cfg!(windows) {
        &["kindling.exe", "kindling.cmd", "kindling"]
    } else {
        &["kindling"]
    };
    std::env::var_os("PATH")
        .map(|path| {
            std::env::split_paths(&path)
                .any(|dir| names.iter().any(|n| is_executable(&dir.join(n))))
        })
        .unwrap_or(false)
}

#[cfg(unix)]
fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &std::path::Path) -> bool {
    path.is_file()
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var_os("USERPROFILE").filter(|v| !v.is_empty()))
        .map(PathBuf::from)
}

fn print_human(output: &InitOutput, claude_code_requested: bool) {
    println!("\nKindling Setup");
    println!("==============\n");

    if output.directory.created {
        println!("Created directory {}", output.directory.path);
    } else {
        println!("Directory exists {}", output.directory.path);
    }

    if let Some(db) = &output.database {
        if db.created {
            println!("Created database {}", db.path);
        } else if db.existed {
            println!("Database exists {}", db.path);
        }
    }

    if claude_code_requested {
        if let Some(cc) = &output.claude_code {
            println!("\nClaude Code Integration");
            println!("-----------------------");
            println!("{}", cc.message);
            println!("\nIn Claude Code, run:");
            for command in &cc.install_commands {
                println!("  {command}");
            }
        }
    }

    println!("\nKindling is ready!\n");
    println!("Next steps:");
    println!("  kindling status     - Check database status");
    println!("  kindling search     - Search your memory");
    println!("  kindling serve      - Start the daemon");
    println!();
}
