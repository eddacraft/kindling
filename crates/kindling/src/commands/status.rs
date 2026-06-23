//! `status` — database statistics plus capability handshake. In-process only.
//!
//! The `--json` shape augments the prior `{ database, counts, activity }` layout
//! with the capability block (`version`, `schemaVersion`, `supportedKinds`,
//! `storagePath`, `kindRegistry`).

use kindling_types::build_capability;
use serde::Serialize;

use crate::cli::StatusArgs;
use crate::output::{format_json, iso8601_utc};
use crate::{open_service, CliResult};

#[derive(Serialize)]
struct StatusOutput {
    #[serde(flatten)]
    capability: kindling_types::Capability,
    database: DatabaseSection,
    counts: CountsSection,
    activity: ActivitySection,
}

#[derive(Serialize)]
struct DatabaseSection {
    path: String,
    size: String,
    #[serde(rename = "sizeBytes")]
    size_bytes: i64,
}

#[derive(Serialize)]
struct CountsSection {
    observations: i64,
    capsules: i64,
    summaries: i64,
    pins: i64,
    redacted: i64,
    #[serde(rename = "openCapsules")]
    open_capsules: i64,
}

#[derive(Serialize)]
struct ActivitySection {
    #[serde(rename = "latestTimestamp")]
    latest_timestamp: Option<i64>,
    #[serde(rename = "latestDate")]
    latest_date: Option<String>,
}

pub fn run(args: StatusArgs) -> CliResult {
    let (service, db_path) = open_service(args.common.db.as_deref())?;
    let stats = service.store().database_stats()?;

    // `(bytes / 1MiB).toFixed(2)` — matches the TS size string.
    let size_mb = stats.size_bytes as f64 / (1024.0 * 1024.0);
    let size = format!("{size_mb:.2} MB");

    let latest_date = stats.latest_ts.map(iso8601_utc);

    let capability = build_capability(
        env!("CARGO_PKG_VERSION"),
        kindling_store::schema_version().version as u32,
        db_path.to_string_lossy(),
    );

    let output = StatusOutput {
        capability,
        database: DatabaseSection {
            path: db_path.to_string_lossy().into_owned(),
            size: size.clone(),
            size_bytes: stats.size_bytes,
        },
        counts: CountsSection {
            observations: stats.observations,
            capsules: stats.capsules,
            summaries: stats.summaries,
            pins: stats.pins,
            redacted: stats.redacted,
            open_capsules: stats.open_capsules,
        },
        activity: ActivitySection {
            latest_timestamp: stats.latest_ts,
            latest_date: latest_date.clone(),
        },
    };

    if args.common.json {
        println!("{}", format_json(&output, true)?);
    } else {
        println!("\nKindling Database Status");
        println!("========================\n");
        println!("Database: {}", output.database.path);
        println!("Size:     {size}\n");
        println!("Entity Counts:");
        println!("  Observations: {}", output.counts.observations);
        println!("  Capsules:     {}", output.counts.capsules);
        println!("  Summaries:    {}", output.counts.summaries);
        println!("  Pins:         {}", output.counts.pins);
        println!("  Redacted:     {}", output.counts.redacted);
        println!("  Open Capsules: {}\n", output.counts.open_capsules);
        println!("Latest Activity:");
        println!(
            "  {}\n",
            latest_date.unwrap_or_else(|| "No activity yet".to_string())
        );
    }
    Ok(())
}
