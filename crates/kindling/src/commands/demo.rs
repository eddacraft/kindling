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

    let (service, _) = open_service(Some(db_path.to_string_lossy().as_ref()))?;

    let bundle = ExportBundle::from_json(DEMO_FIXTURE)?;
    let result = service.import(&bundle, ImportOptions { dry_run: false })?;

    let try_next = vec![
        format!(
            "kindling search \"JWT\" --all-repos --db {}",
            shell_quote(&db_path)
        ),
        format!("kindling browse --db {}", shell_quote(&db_path)),
        format!("kindling list observations --db {}", shell_quote(&db_path)),
        format!("kindling status --db {}", shell_quote(&db_path)),
    ];

    if args.common.json {
        let out = DemoOutput {
            success: result.errors.is_empty(),
            database: db_path.to_string_lossy().into_owned(),
            imported: ImportCounts {
                observations: result.observations,
                capsules: result.capsules,
                summaries: result.summaries,
                pins: result.pins,
            },
            try_next,
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
        for line in &try_next {
            println!("  {line}");
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
/// This targets POSIX shells. On Windows the recipes are not quoted for
/// `cmd.exe`/PowerShell, which treat `'` as an ordinary character rather than a
/// grouping delimiter; run them through a POSIX shell (Git Bash, WSL) there.
fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::shell_quote;
    use std::path::Path;

    #[test]
    fn quotes_a_plain_path() {
        assert_eq!(
            shell_quote(Path::new("/home/user/kindling.db")),
            "'/home/user/kindling.db'"
        );
    }

    #[test]
    fn quotes_paths_with_spaces() {
        assert_eq!(
            shell_quote(Path::new("/tmp/kindling home/demo.db")),
            "'/tmp/kindling home/demo.db'"
        );
    }

    #[test]
    fn escapes_embedded_single_quotes() {
        assert_eq!(
            shell_quote(Path::new("/tmp/o'brien/demo.db")),
            "'/tmp/o'\\''brien/demo.db'"
        );
    }

    #[test]
    fn quotes_shell_metacharacters_literally() {
        assert_eq!(
            shell_quote(Path::new("/tmp/db $HOME;$(id)/demo.db")),
            "'/tmp/db $HOME;$(id)/demo.db'"
        );
    }
}
