//! `demo` — load sample memory so you can try search and browse immediately.

use std::path::PathBuf;

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
    let result = service.import(
        &bundle,
        ImportOptions {
            dry_run: false,
        },
    )?;

    let try_next = vec![
        format!(
            "kindling search \"JWT\" --db {}",
            shell_quote(&db_path)
        ),
        format!(
            "kindling browse --db {}",
            shell_quote(&db_path)
        ),
        format!(
            "kindling list observations --db {}",
            shell_quote(&db_path)
        ),
        format!(
            "kindling status --db {}",
            shell_quote(&db_path)
        ),
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

fn shell_quote(path: &PathBuf) -> String {
    let s = path.to_string_lossy();
    if s.contains(' ') {
        format!("'{s}'")
    } else {
        s.into_owned()
    }
}