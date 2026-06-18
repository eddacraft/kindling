//! `init` — create the database (and optionally note Claude Code setup).
//!
//! Ports `packages/kindling-cli/src/commands/init.ts` for the directory +
//! database steps. The `--json` shape matches field-for-field:
//! `{ directory, database, claudeCode }`.
//!
//! Claude Code step (deviation, documented): the TS command copied the **Node**
//! plugin (`plugins/kindling-claude-code/`) into `~/.claude/plugins/kindling`
//! and toggled `enabledPlugins` in `~/.claude/settings.json`. That plugin is a
//! JS hook bundle tied to the TypeScript packages, not the Rust binary, and the
//! plugin cutover to the Rust hook/daemon is owned by PORT-015. So here
//! `--claude-code` does NOT copy a plugin; it detects whether `~/.claude/`
//! exists and returns a not-configured result whose `message` points at
//! PORT-015. The DB/directory creation is done faithfully. When PORT-015 lands
//! the plugin cutover, this step can perform the real install.

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
    #[serde(rename = "pluginPath")]
    plugin_path: String,
    message: String,
}

pub fn run(args: InitArgs) -> CliResult {
    let db_path = resolve_db_path(args.db.as_deref())?;
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

/// Detect Claude Code but defer the actual plugin install to PORT-015.
fn configure_claude_code() -> ClaudeCodeResult {
    let claude_dir = home_dir().map(|h| h.join(".claude"));
    match claude_dir {
        Some(dir) if dir.exists() => ClaudeCodeResult {
            configured: false,
            plugin_path: String::new(),
            message: "Claude Code detected, but the Rust plugin cutover is not yet \
                      available (tracked by PORT-015). No plugin was installed."
                .to_string(),
        },
        _ => ClaudeCodeResult {
            configured: false,
            plugin_path: String::new(),
            message: "Claude Code not detected (~/.claude/ does not exist)".to_string(),
        },
    }
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
        }
    }

    println!("\nKindling is ready!\n");
    println!("Next steps:");
    println!("  kindling status     - Check database status");
    println!("  kindling search     - Search your memory");
    println!("  kindling serve      - Start the daemon");
    println!();
}
