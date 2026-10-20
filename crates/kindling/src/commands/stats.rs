//! `stats` — memory statistics: observation counts by kind, capsules, pins and
//! the activity range. In-process only, like `status`: there is no daemon
//! endpoint for it, so `--via-daemon` does not reroute this verb.
//!
//! The figures come from [`kindling_service::KindlingService::stats`], a
//! read-only aggregate over metadata columns, so no observation content
//! (redacted or otherwise) is ever read or printed. Breakdowns list every known
//! variant in declaration order, zero-filled, so output is deterministic for a
//! given database (pins' active/expired split is judged as of now).

use serde::Serialize;
use serde_json::{Map, Value};

use kindling_service::MemoryStats;
use kindling_types::Timestamp;

use crate::cli::StatsArgs;
use crate::output::{format_json, format_timestamp, iso8601_utc};
use crate::{open_service, CliResult};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatsOutput {
    database: DatabaseSection,
    observations: ObservationsSection,
    capsules: CapsulesSection,
    summaries: i64,
    pins: PinsSection,
}

#[derive(Serialize)]
struct DatabaseSection {
    path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ObservationsSection {
    total: i64,
    redacted: i64,
    by_kind: Map<String, Value>,
    earliest_timestamp: Option<Timestamp>,
    earliest_date: Option<String>,
    latest_timestamp: Option<Timestamp>,
    latest_date: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CapsulesSection {
    total: i64,
    open: i64,
    closed: i64,
    by_type: Map<String, Value>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PinsSection {
    total: i64,
    active: i64,
    expired: i64,
    by_target: Map<String, Value>,
}

/// Turn an ordered `(variant, count)` breakdown into a JSON object keyed by the
/// variant's wire name (`tool_call`, `pocketflow_node`, ...), keeping order.
fn breakdown<K: Serialize>(entries: &[(K, i64)]) -> Result<Map<String, Value>, serde_json::Error> {
    let mut map = Map::new();
    for (key, count) in entries {
        let name = match serde_json::to_value(key)? {
            Value::String(name) => name,
            other => other.to_string(),
        };
        map.insert(name, Value::from(*count));
    }
    Ok(map)
}

fn build_output(stats: &MemoryStats, db_path: String) -> Result<StatsOutput, serde_json::Error> {
    let obs = &stats.observations;
    Ok(StatsOutput {
        database: DatabaseSection { path: db_path },
        observations: ObservationsSection {
            total: obs.total,
            redacted: obs.redacted,
            by_kind: breakdown(&obs.by_kind)?,
            earliest_timestamp: obs.earliest_ts,
            earliest_date: obs.earliest_ts.map(iso8601_utc),
            latest_timestamp: obs.latest_ts,
            latest_date: obs.latest_ts.map(iso8601_utc),
        },
        capsules: CapsulesSection {
            total: stats.capsules.total,
            open: stats.capsules.open,
            closed: stats.capsules.closed,
            by_type: breakdown(&stats.capsules.by_type)?,
        },
        summaries: stats.summaries,
        pins: PinsSection {
            total: stats.pins.total,
            active: stats.pins.active,
            expired: stats.pins.expired,
            by_target: breakdown(&stats.pins.by_target)?,
        },
    })
}

/// Print a two-column table section with right-aligned counts.
fn print_section(title: &str, rows: &[(String, i64)]) {
    let label_width = rows.iter().map(|(l, _)| l.len()).max().unwrap_or(0);
    let count_width = rows
        .iter()
        .map(|(_, n)| n.to_string().len())
        .max()
        .unwrap_or(0);
    println!("{title}");
    for (label, count) in rows {
        println!("  {label:<label_width$}  {count:>count_width$}");
    }
    println!();
}

fn map_rows(map: &Map<String, Value>) -> Vec<(String, i64)> {
    map.iter()
        .map(|(k, v)| (format!("  {k}"), v.as_i64().unwrap_or(0)))
        .collect()
}

fn print_text(output: &StatsOutput) {
    println!("\nKindling Memory Stats");
    println!("=====================\n");
    println!("Database: {}\n", output.database.path);

    let obs = &output.observations;
    let mut rows = vec![
        ("Total".to_string(), obs.total),
        ("Redacted".to_string(), obs.redacted),
    ];
    rows.extend(map_rows(&obs.by_kind));
    print_section("Observations", &rows);

    let caps = &output.capsules;
    let mut rows = vec![
        ("Total".to_string(), caps.total),
        ("Open".to_string(), caps.open),
        ("Closed".to_string(), caps.closed),
    ];
    rows.extend(map_rows(&caps.by_type));
    print_section("Capsules", &rows);

    print_section("Summaries", &[("Total".to_string(), output.summaries)]);

    let pins = &output.pins;
    let mut rows = vec![
        ("Total".to_string(), pins.total),
        ("Active".to_string(), pins.active),
        ("Expired".to_string(), pins.expired),
    ];
    rows.extend(map_rows(&pins.by_target));
    print_section("Pins", &rows);

    println!("Activity");
    match (obs.earliest_timestamp, obs.latest_timestamp) {
        (Some(earliest), Some(latest)) => {
            println!("  Earliest  {} UTC", format_timestamp(earliest));
            println!("  Latest    {} UTC\n", format_timestamp(latest));
        }
        _ => println!("  No activity yet\n"),
    }
}

pub fn run(args: StatsArgs) -> CliResult {
    let (service, db_path) = open_service(args.common.db.as_deref())?;
    let stats = service.stats()?;
    let output = build_output(&stats, db_path.to_string_lossy().into_owned())?;

    if args.common.json {
        println!("{}", format_json(&output, true)?);
    } else {
        print_text(&output);
    }
    Ok(())
}
