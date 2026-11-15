//! `demo` — load sample memory so you can try search and browse immediately.

use std::path::{Path, PathBuf};

use serde::Serialize;

use kindling_service::{ExportBundle, ImportOptions};
use kindling_store::default_kindling_home;

use crate::cli::DemoArgs;
use crate::output::format_json;
use crate::{open_service, CliError, CliResult};

const DEMO_FIXTURE: &str = include_str!("../../fixtures/demo-export.json");

#[derive(Serialize)]
struct DemoOutput {
    success: bool,
    database: String,
    imported: ImportCounts,
    #[serde(rename = "tryNext")]
    try_next: Vec<String>,
}

struct DemoRecipe {
    label: &'static str,
    command: String,
}

#[derive(Serialize)]
struct ImportCounts {
    observations: usize,
    capsules: usize,
    summaries: usize,
    pins: usize,
}

/// Default demo database: `~/.kindling/demo/kindling.db`.
pub fn demo_db_path() -> Result<PathBuf, CliError> {
    let home = default_kindling_home().ok_or_else(|| {
        CliError::Invalid(
            "could not resolve demo database path: no HOME or USERPROFILE set".to_string(),
        )
    })?;
    Ok(home.join("demo").join("kindling.db"))
}

pub fn run(args: DemoArgs) -> CliResult {
    let db_path = match args.common.db.as_deref() {
        Some(path) => PathBuf::from(path),
        None => demo_db_path()?,
    };

    if args.reset && db_path.exists() {
        std::fs::remove_file(&db_path)?;
    }

    let database = database_string(&db_path)?;
    let (service, _) = open_service(Some(database))?;

    let bundle = ExportBundle::from_json(DEMO_FIXTURE)?;
    let result = service.import(&bundle, ImportOptions { dry_run: false })?;

    let try_next = try_next_commands(database);

    if args.common.json {
        let out = DemoOutput {
            success: result.errors.is_empty(),
            database: database.to_owned(),
            imported: ImportCounts {
                observations: result.observations,
                capsules: result.capsules,
                summaries: result.summaries,
                pins: result.pins,
            },
            try_next: try_next
                .iter()
                .map(|recipe| recipe.command.clone())
                .collect(),
        };
        println!("{}", format_json(&out, true)?);
    } else {
        println!("\nDemo memory loaded");
        println!("Database: {}", db_path.display());
        println!("\nImported:");
        println!("  Observations: {}", result.observations);
        println!("  Capsules:     {}", result.capsules);
        println!("  Summaries:    {}", result.summaries);
        println!("  Pins:         {}", result.pins);
        if !result.errors.is_empty() {
            println!("\nWarnings ({}):", result.errors.len());
            for error in &result.errors {
                println!("  - {error}");
            }
        }
        println!("\nTry next:");
        let mut label = None;
        for recipe in &try_next {
            if label != Some(recipe.label) {
                println!("\n{}:", recipe.label);
                label = Some(recipe.label);
            }
            println!("  {}", recipe.command);
        }
        println!();
    }

    Ok(())
}

/// Quote a database path for a POSIX shell (`sh`/`bash`/`zsh`).
///
/// The demo recipes are copied by hand, so the path must survive a shell
/// word-splitting pass verbatim. Single quotes make every character literal
/// except `'` itself, so embedded single quotes are rewritten as the shell
/// sequence `'\''` (close quote, escaped quote, reopen quote). Always quoting
/// keeps paths with spaces, apostrophes or metacharacters (`$`, `;`, `` ` ``,
/// `*`, …) literal without having to enumerate them.
///
/// Unix emits POSIX recipes. Windows emits labelled `cmd.exe` and PowerShell
/// recipes because those shells use different quoting rules.
fn try_next_commands(database: &str) -> Vec<DemoRecipe> {
    let prefixes = [
        "kindling search \"JWT\" --all-repos --db=",
        "kindling browse --db=",
        "kindling list observations --db=",
        "kindling status --db=",
    ];
    #[cfg(windows)]
    {
        let mut recipes = Vec::with_capacity(prefixes.len() * 2);
        for prefix in prefixes {
            recipes.push(DemoRecipe {
                label: "cmd.exe",
                command: format!("{prefix}{}", cmd_quote(database)),
            });
        }
        for prefix in prefixes {
            recipes.push(DemoRecipe {
                label: "PowerShell",
                command: format!("{prefix}{}", powershell_quote(database)),
            });
        }
        recipes
    }
    #[cfg(not(windows))]
    {
        prefixes
            .into_iter()
            .map(|prefix| DemoRecipe {
                label: "POSIX",
                command: format!("{prefix}{}", posix_quote(database)),
            })
            .collect()
    }
}

fn database_string(path: &Path) -> Result<&str, CliError> {
    path.to_str()
        .ok_or_else(|| CliError::Invalid("demo database path is not valid UTF-8".to_string()))
}

#[cfg(not(windows))]
fn posix_quote(path: &str) -> String {
    format!("'{}'", path.replace('\'', "'\\''"))
}

#[cfg(windows)]
fn cmd_quote(path: &str) -> String {
    if needs_windows_quote(path) {
        format!("\"{path}\"")
    } else {
        path.to_owned()
    }
}

#[cfg(windows)]
fn powershell_quote(path: &str) -> String {
    format!("'{}'", path.replace('\'', "''"))
}

#[cfg(windows)]
fn needs_windows_quote(path: &str) -> bool {
    path.chars().any(|character| {
        character.is_whitespace() || matches!(character, '&' | '(' | ')' | '%' | '!' | '^' | '|')
    })
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use super::database_string;
    #[cfg(windows)]
    use super::{cmd_quote, powershell_quote};
    #[cfg(not(windows))]
    use super::{posix_quote, try_next_commands};
    #[cfg(unix)]
    use std::path::PathBuf;

    #[cfg(not(windows))]
    #[test]
    fn quotes_a_plain_path() {
        assert_eq!(
            posix_quote("/home/user/kindling.db"),
            "'/home/user/kindling.db'"
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn quotes_paths_with_spaces() {
        assert_eq!(
            posix_quote("/tmp/kindling home/demo.db"),
            "'/tmp/kindling home/demo.db'"
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn escapes_embedded_single_quotes() {
        assert_eq!(
            posix_quote("/tmp/o'brien/demo.db"),
            "'/tmp/o'\\''brien/demo.db'"
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn quotes_shell_metacharacters_literally() {
        assert_eq!(
            posix_quote("/tmp/db $HOME;$(id)/demo.db"),
            "'/tmp/db $HOME;$(id)/demo.db'"
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn posix_recipe_round_trips_through_shell() {
        let path = "/tmp/db $HOME;$(id)/demo.db";
        let recipe = try_next_commands(path).remove(0);
        assert_eq!(recipe.label, "POSIX");
        let quoted = recipe.command.split_once("--db=").unwrap().1;
        let script = format!("printf '%s' {quoted}");
        let output = std::process::Command::new("sh")
            .args(["-c", &script])
            .output()
            .expect("POSIX shell should run");
        assert_eq!(String::from_utf8(output.stdout).unwrap(), path);
    }

    #[cfg(not(windows))]
    #[test]
    fn recipes_bind_leading_dash_paths_to_db_option() {
        let recipes = try_next_commands("-scratch.db");
        assert!(recipes
            .iter()
            .all(|recipe| recipe.command.contains("--db='-scratch.db'")));
        assert!(recipes
            .iter()
            .all(|recipe| !recipe.command.contains("POSIX:")));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_non_utf8_database_paths() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let path = PathBuf::from(OsString::from_vec(vec![b'd', b'b', 0xff]));
        assert!(database_string(&path).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn windows_recipes_quote_cmd_and_powershell_paths() {
        assert_eq!(
            cmd_quote(r"C:\Users\alice\.kindling\demo.db"),
            r"C:\Users\alice\.kindling\demo.db"
        );
        assert_eq!(
            powershell_quote(r"C:\tmp\o'brien\demo.db"),
            r#"'C:\tmp\o''brien\demo.db'"#
        );
    }
}
