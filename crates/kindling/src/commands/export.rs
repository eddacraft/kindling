//! `export [output]` / `import <file>` — data portability. In-process only.
//!
//! Ports `packages/kindling-cli/src/commands/export.ts`. Export/import operate
//! on the DB file directly, so `--via-daemon` does not apply: passing it with
//! either verb is a clear error (documented deviation — the brief allowed
//! either erroring or warning; erroring is the louder, safer choice).
//!
//! The written bundle JSON is byte-compatible with the TS `ExportBundle` so it
//! round-trips through the TS importer. Export metadata mirrors the TS command:
//! `{ description: "Kindling memory export", exportedAt: <ISO-8601> }`.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use kindling_service::{ExportBundleOptions, ImportOptions};
use kindling_types::ScopeIds;

use crate::cli::{ExportArgs, ImportArgs};
use crate::output::{format_json, iso8601_utc};
use crate::{open_service, CliError, CliResult};

#[derive(Serialize)]
struct ExportMetaOutput {
    success: bool,
    #[serde(rename = "outputPath")]
    output_path: String,
    stats: StatsOutput,
}

#[derive(Serialize)]
struct StatsOutput {
    observations: usize,
    capsules: usize,
    summaries: usize,
    pins: usize,
    #[serde(rename = "totalSize")]
    total_size: usize,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn run_export(args: ExportArgs, via_daemon: bool) -> CliResult {
    if via_daemon {
        return Err(CliError::Invalid(
            "export operates on the database file directly and cannot be routed \
             through the daemon (--via-daemon is not supported for export)"
                .to_string(),
        ));
    }

    let (service, _db) = open_service(args.common.db.as_deref())?;

    let timestamp = args.timestamp.unwrap_or_else(now_ms);

    // Scope is set only when at least one dimension is provided (matching the
    // TS command, which always passes `{ sessionId, repoId }` but with possibly
    // undefined values — an all-undefined scope serializes to `{}` there; here
    // we omit it entirely when empty, which is the cleaner equivalent).
    let scope = match (args.session.as_deref(), args.repo.as_deref()) {
        (None, None) => None,
        (session, repo) => Some(ScopeIds {
            session_id: session.map(str::to_string),
            repo_id: repo.map(str::to_string),
            ..Default::default()
        }),
    };

    // Metadata: `{ description, exportedAt }`. `exportedAt` here is the ISO-8601
    // string the TS command used (distinct from the numeric bundle `exportedAt`).
    let mut metadata = serde_json::Map::new();
    metadata.insert(
        "description".to_string(),
        serde_json::Value::String("Kindling memory export".to_string()),
    );
    metadata.insert(
        "exportedAt".to_string(),
        serde_json::Value::String(iso8601_utc(timestamp)),
    );

    let bundle = service.export(ExportBundleOptions {
        scope,
        include_redacted: false,
        limit: None,
        metadata: Some(metadata),
        exported_at: timestamp,
    })?;
    let stats = bundle.stats()?;

    let output_path = args
        .output
        .clone()
        .unwrap_or_else(|| format!("kindling-export-{timestamp}.json"));
    let json = bundle.to_json(args.pretty)?;
    std::fs::write(&output_path, json)?;

    if args.common.json {
        let out = ExportMetaOutput {
            success: true,
            output_path: output_path.clone(),
            stats: StatsOutput {
                observations: stats.observations,
                capsules: stats.capsules,
                summaries: stats.summaries,
                pins: stats.pins,
                total_size: stats.total_size,
            },
        };
        println!("{}", format_json(&out, true)?);
    } else {
        println!("\nExport successful");
        println!("Output: {output_path}");
        println!("\nStatistics:");
        println!("  Observations: {}", stats.observations);
        println!("  Capsules:     {}", stats.capsules);
        println!("  Summaries:    {}", stats.summaries);
        println!("  Pins:         {}", stats.pins);
        println!(
            "  Size:         {:.2} KB\n",
            stats.total_size as f64 / 1024.0
        );
    }
    Ok(())
}

pub fn run_import(args: ImportArgs, via_daemon: bool) -> CliResult {
    if via_daemon {
        return Err(CliError::Invalid(
            "import operates on the database file directly and cannot be routed \
             through the daemon (--via-daemon is not supported for import)"
                .to_string(),
        ));
    }

    let (service, _db) = open_service(args.common.db.as_deref())?;

    let json = std::fs::read_to_string(&args.file)?;
    let bundle = kindling_service::ExportBundle::from_json(&json)?;
    let result = service.import(
        &bundle,
        ImportOptions {
            dry_run: args.dry_run,
        },
    )?;

    if args.common.json {
        println!("{}", format_json(&result, true)?);
    } else {
        let phase = if args.dry_run { "Dry run" } else { "Import" };
        let status = if result.errors.is_empty() {
            "successful"
        } else {
            "completed with errors"
        };
        println!("\n{phase} {status}");
        println!("\nImported:");
        println!("  Observations: {}", result.observations);
        println!("  Capsules:     {}", result.capsules);
        println!("  Summaries:    {}", result.summaries);
        println!("  Pins:         {}", result.pins);
        if !result.errors.is_empty() {
            println!("\nErrors ({}):", result.errors.len());
            for error in &result.errors {
                println!("  - {error}");
            }
        }
        println!();
    }
    Ok(())
}
