//! `list <entity>` — list capsules, pins, or observations. In-process only.
//!
//! Ports `packages/kindling-cli/src/commands/list.ts`. Parity note: the TS
//! command returns **raw SQLite rows** for `capsules` and `observations` (so
//! the `--json` keys are snake_case DB column names, `scope_ids` is the raw
//! JSON *string*, and `redacted` is the integer `0`/`1`), but returns
//! `service.listPins()` typed `Pin[]` (camelCase) for `pins`. We reproduce all
//! three shapes exactly.

use rusqlite::types::Value as SqlValue;
use serde_json::{Map, Value};

use crate::cli::ListArgs;
use crate::commands::scope_from;
use crate::output::{format_json, format_timestamp, truncate};
use crate::{open_service, CliError, CliResult};

pub fn run(args: ListArgs) -> CliResult {
    let (service, _db) = open_service(args.common.db.as_deref())?;
    let entity = args.entity.to_lowercase();

    let rows: Vec<Value> = match entity.as_str() {
        "capsules" => list_capsules(&service, &args)?,
        "observations" => list_observations(&service, &args)?,
        "pins" => list_pins(&service, &args)?,
        other => {
            return Err(CliError::Invalid(format!(
                "Unknown entity type: {other}. Valid types: capsules, pins, observations"
            )));
        }
    };

    if args.common.json {
        println!("{}", format_json(&rows, true)?);
    } else {
        let title = capitalize(&args.entity);
        println!("\n{title} ({}):", rows.len());
        println!("{}\n", "=".repeat(50));

        if rows.is_empty() {
            println!("No results found.\n");
        } else {
            for (i, row) in rows.iter().enumerate() {
                println!("{}. {}", i + 1, string_field(row, "id"));
                match entity.as_str() {
                    "capsules" => {
                        println!("   Type: {}", string_field(row, "type"));
                        println!("   Intent: {}", string_field(row, "intent"));
                        println!("   Status: {}", string_field(row, "status"));
                        if let Some(opened) = num_field(row, "opened_at") {
                            println!("   Opened: {}", format_timestamp(opened));
                        }
                        if let Some(closed) = num_field(row, "closed_at") {
                            println!("   Closed: {}", format_timestamp(closed));
                        }
                    }
                    "pins" => {
                        println!(
                            "   Target: {} {}",
                            string_field(row, "targetType"),
                            string_field(row, "targetId")
                        );
                        if let Some(note) = opt_string_field(row, "reason") {
                            println!("   Note: {note}");
                        }
                        if let Some(created) = num_field(row, "createdAt") {
                            println!("   Created: {}", format_timestamp(created));
                        }
                        if let Some(expires) = num_field(row, "expiresAt") {
                            println!("   Expires: {}", format_timestamp(expires));
                        }
                    }
                    "observations" => {
                        println!("   Kind: {}", string_field(row, "kind"));
                        println!(
                            "   Content: {}",
                            truncate(&string_field(row, "content"), 100)
                        );
                        if let Some(ts) = num_field(row, "ts") {
                            println!("   Time: {}", format_timestamp(ts));
                        }
                        let redacted = num_field(row, "redacted").unwrap_or(0) != 0;
                        println!("   Redacted: {}", if redacted { "yes" } else { "no" });
                    }
                    _ => {}
                }
                println!();
            }
        }
    }
    Ok(())
}

/// Raw-row read of `capsules`, matching the TS query (snake_case keys,
/// `scope_ids` as the raw JSON string, `closed_at` null when absent).
fn list_capsules(
    service: &kindling_service::KindlingService,
    args: &ListArgs,
) -> Result<Vec<Value>, CliError> {
    let conn = service.store().connection();
    let mut sql = String::from(
        "SELECT id, type, intent, status, opened_at, closed_at, scope_ids FROM capsules",
    );
    let mut params: Vec<SqlValue> = Vec::new();
    push_scope_where(
        &mut sql,
        &mut params,
        args.session.as_deref(),
        args.repo.as_deref(),
    );
    sql.push_str(" ORDER BY opened_at DESC LIMIT ?");
    params.push(SqlValue::Integer(i64::from(args.limit)));

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(params), |row| {
            let mut obj = Map::new();
            obj.insert("id".into(), Value::String(row.get::<_, String>(0)?));
            obj.insert("type".into(), Value::String(row.get::<_, String>(1)?));
            obj.insert("intent".into(), Value::String(row.get::<_, String>(2)?));
            obj.insert("status".into(), Value::String(row.get::<_, String>(3)?));
            obj.insert("opened_at".into(), Value::from(row.get::<_, i64>(4)?));
            obj.insert(
                "closed_at".into(),
                opt_i64_value(row.get::<_, Option<i64>>(5)?),
            );
            obj.insert("scope_ids".into(), Value::String(row.get::<_, String>(6)?));
            Ok(Value::Object(obj))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Raw-row read of `observations`, matching the TS query (snake_case keys,
/// `redacted` as the integer `0`/`1`, `scope_ids` as the raw JSON string).
fn list_observations(
    service: &kindling_service::KindlingService,
    args: &ListArgs,
) -> Result<Vec<Value>, CliError> {
    let conn = service.store().connection();
    let mut sql =
        String::from("SELECT id, kind, content, ts, scope_ids, redacted FROM observations");
    let mut params: Vec<SqlValue> = Vec::new();
    push_scope_where(
        &mut sql,
        &mut params,
        args.session.as_deref(),
        args.repo.as_deref(),
    );
    sql.push_str(" ORDER BY ts DESC LIMIT ?");
    params.push(SqlValue::Integer(i64::from(args.limit)));

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(params), |row| {
            let mut obj = Map::new();
            obj.insert("id".into(), Value::String(row.get::<_, String>(0)?));
            obj.insert("kind".into(), Value::String(row.get::<_, String>(1)?));
            obj.insert("content".into(), Value::String(row.get::<_, String>(2)?));
            obj.insert("ts".into(), Value::from(row.get::<_, i64>(3)?));
            obj.insert("scope_ids".into(), Value::String(row.get::<_, String>(4)?));
            obj.insert("redacted".into(), Value::from(row.get::<_, i64>(5)?));
            Ok(Value::Object(obj))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Typed `Pin[]` from the service (camelCase), matching `service.listPins`.
fn list_pins(
    service: &kindling_service::KindlingService,
    args: &ListArgs,
) -> Result<Vec<Value>, CliError> {
    let scope = scope_from(args.session.as_deref(), args.repo.as_deref());
    let pins = service.list_pins(Some(&scope))?;
    pins.iter()
        .map(|pin| serde_json::to_value(pin).map_err(CliError::from))
        .collect()
}

/// Append `WHERE`/`AND` filters on the denormalized `session_id`/`repo_id`
/// columns, matching the TS `json_extract`-based filter (equivalent results).
fn push_scope_where(
    sql: &mut String,
    params: &mut Vec<SqlValue>,
    session: Option<&str>,
    repo: Option<&str>,
) {
    let mut conditions: Vec<&str> = Vec::new();
    if session.is_some() {
        conditions.push("session_id = ?");
    }
    if repo.is_some() {
        conditions.push("repo_id = ?");
    }
    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
        if let Some(s) = session {
            params.push(SqlValue::Text(s.to_string()));
        }
        if let Some(r) = repo {
            params.push(SqlValue::Text(r.to_string()));
        }
    }
}

fn opt_i64_value(v: Option<i64>) -> Value {
    match v {
        Some(n) => Value::from(n),
        None => Value::Null,
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

fn string_field(row: &Value, key: &str) -> String {
    row.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_default()
}

fn opt_string_field(row: &Value, key: &str) -> Option<String> {
    match row.get(key) {
        Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
        _ => None,
    }
}

fn num_field(row: &Value, key: &str) -> Option<i64> {
    row.get(key).and_then(Value::as_i64)
}
