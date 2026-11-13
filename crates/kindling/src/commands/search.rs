//! `search <query>` — deterministic retrieval.
//!
//! Ports `packages/kindling-cli/src/commands/search.ts`. `--via-daemon` routes
//! through the daemon client. The `--json` output is the full `RetrieveResult`
//! (already camelCase via its `Serialize`); its `provenance.scopeIds` records
//! the repo scope that was actually applied.
//!
//! Without `--repo`, the repo scope defaults to the enclosing git repository
//! root (see [`crate::commands::resolve_repo`]); `--all-repos` searches every
//! repository.

use kindling_types::{
    ObservationKind, RetrieveOptions, RetrieveResult, RetrievedEntity, Timestamp,
};

use crate::cli::SearchArgs;
use crate::commands::{resolve_repo, scope_from};
use crate::output::{format_json, format_timestamp, truncate};
use crate::{build_client, open_service, runtime, CliError, CliResult};

pub fn run(args: SearchArgs, via_daemon: bool) -> CliResult {
    let repo = resolve_repo(args.repo.as_deref(), args.all_repos);
    let scope = scope_from(args.session.as_deref(), repo.as_deref());

    // Parse `--kind` / `--after` / `--before` up front so the retrieve call can
    // widen its candidate limit when a filter will re-limit afterwards.
    let now = now_ms();
    let kind = match args.kind.as_deref() {
        Some(k) => Some(crate::commands::log::parse_kind(k)?),
        None => None,
    };
    let after = match args.after.as_deref() {
        Some(a) => Some(parse_time_bound(a, now)?),
        None => None,
    };
    let before = match args.before.as_deref() {
        Some(b) => Some(parse_time_bound(b, now)?),
        None => None,
    };
    let has_filters = kind.is_some() || after.is_some() || before.is_some();

    let options = RetrieveOptions {
        query: args.query.clone(),
        scope_ids: scope,
        token_budget: None,
        // When filtering, retrieve the full ranked set and let `apply_filters`
        // re-limit, so a filter never drops matches that the pre-filter `--max`
        // would have cut off.
        max_candidates: if has_filters { None } else { Some(args.max) },
        include_redacted: None,
    };

    let mut result = if via_daemon {
        let client = build_client(args.common.db.as_deref())?;
        runtime()?.block_on(async { client.retrieve(options).await })?
    } else {
        let (service, _db) = open_service(args.common.db.as_deref())?;
        service.retrieve(options)?
    };

    if has_filters {
        apply_filters(&mut result, kind, after, before, args.max);
    }

    if args.common.json {
        println!("{}", format_json(&result, true)?);
    } else {
        println!("\nSearch Results for: \"{}\"", args.query);
        println!("{}", "=".repeat(50));
        match &repo {
            Some(repo) => println!("Repo: {repo} (use --all-repos to search every repository)\n"),
            None => println!("Repo: all repositories\n"),
        }

        if !result.pins.is_empty() {
            println!("Pins ({}):", result.pins.len());
            for (i, pin) in result.pins.iter().enumerate() {
                let (id, type_label, content) = entity_fields(&pin.target);
                println!("\n{}. [PIN] {id}", i + 1);
                println!("   Type: {type_label}");
                println!("   Content: {}", truncate(content, 100));
                if let Some(reason) = &pin.pin.reason {
                    println!("   Note: {reason}");
                }
            }
            println!();
        }

        if let Some(summary) = &result.current_summary {
            println!("Current Summary:");
            println!("  {}", truncate(&summary.content, 200));
            println!("  Confidence: {}", summary.confidence);
            println!();
        }

        if !result.candidates.is_empty() {
            println!("Candidates ({}):", result.candidates.len());
            for (i, candidate) in result.candidates.iter().enumerate() {
                let (id, type_label, content) = entity_fields(&candidate.entity);
                println!("\n{}. {id} (score: {:.2})", i + 1, candidate.score);
                println!("   Type: {type_label}");
                println!("   Content: {}", truncate(content, 100));
                if let RetrievedEntity::Observation(obs) = &candidate.entity {
                    println!("   Time: {}", format_timestamp(obs.ts));
                }
            }
        } else if result.pins.is_empty() && result.current_summary.is_none() {
            println!("No results found.");
        }

        println!();
    }
    Ok(())
}

/// `(id, "type label", content)` for an observation or summary. The type label
/// mirrors the TS `'kind' in entity ? entity.kind : 'summary'`.
fn entity_fields(entity: &RetrievedEntity) -> (&str, String, &str) {
    match entity {
        RetrievedEntity::Observation(obs) => {
            (obs.id.as_str(), kind_label(obs.kind), obs.content.as_str())
        }
        RetrievedEntity::Summary(summary) => (
            summary.id.as_str(),
            "summary".to_string(),
            summary.content.as_str(),
        ),
    }
}

fn kind_label(kind: kindling_types::ObservationKind) -> String {
    use kindling_types::ObservationKind::*;
    match kind {
        ToolCall => "tool_call",
        Command => "command",
        FileDiff => "file_diff",
        Error => "error",
        Message => "message",
        NodeStart => "node_start",
        NodeEnd => "node_end",
        NodeOutput => "node_output",
        NodeError => "node_error",
    }
    .to_string()
}

// --- C7: search filters -----------------------------------------------------

/// Current time in epoch milliseconds (same clock the service uses).
fn now_ms() -> Timestamp {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_millis() as Timestamp
}

/// Parse a `--after` / `--before` bound: either epoch milliseconds (an integer)
/// or a relative "<N> <unit> ago" phrase resolved against `now`.
fn parse_time_bound(raw: &str, now: Timestamp) -> Result<Timestamp, CliError> {
    let s = raw.trim();
    if s.is_empty() {
        return Err(CliError::Invalid(
            "time bound must not be empty".to_string(),
        ));
    }
    if let Ok(ms) = s.parse::<i64>() {
        return Ok(ms);
    }
    if let Some(rest) = s.strip_suffix(" ago") {
        return parse_relative(rest.trim(), now);
    }
    Err(CliError::Invalid(format!(
        "invalid time bound '{raw}': expected epoch milliseconds or '<N> <unit> ago'"
    )))
}

fn parse_relative(rest: &str, now: Timestamp) -> Result<Timestamp, CliError> {
    let mut parts = rest.split_whitespace();
    let n = parts
        .next()
        .ok_or_else(|| CliError::Invalid(format!("invalid relative time '{rest}'")))?
        .parse::<i64>()
        .map_err(|_| CliError::Invalid(format!("invalid relative time '{rest}'")))?;
    let unit = parts.next().unwrap_or("");
    if parts.next().is_some() {
        return Err(CliError::Invalid(format!("invalid relative time '{rest}'")));
    }
    let ms_per_unit = match unit {
        "second" | "seconds" => 1_000i64,
        "minute" | "minutes" => 60 * 1_000,
        "hour" | "hours" => 60 * 60 * 1_000,
        "day" | "days" => 24 * 60 * 60 * 1_000,
        "week" | "weeks" => 7 * 24 * 60 * 60 * 1_000,
        _ => {
            return Err(CliError::Invalid(format!(
                "unknown time unit '{unit}' (expected second(s), minute(s), hour(s), day(s), or week(s))"
            )))
        }
    };
    let delta = n
        .checked_mul(ms_per_unit)
        .ok_or_else(|| CliError::Invalid(format!("relative time '{rest}' is too large")))?;
    now.checked_sub(delta)
        .ok_or_else(|| CliError::Invalid(format!("relative time '{rest}' is out of range")))
}

/// Apply `--kind` / `--after` / `--before` filters to a retrieval result in
/// place, then re-apply the user's `--max` and fix the provenance counts so they
/// reflect the filtered result. Summaries carry no kind (so `--kind` drops
/// them, including the current summary) but are time-filtered on `createdAt`.
/// Time bounds are half-open: `after` inclusive, `before` exclusive.
fn apply_filters(
    result: &mut RetrieveResult,
    kind: Option<ObservationKind>,
    after: Option<Timestamp>,
    before: Option<Timestamp>,
    max: u32,
) {
    result
        .pins
        .retain(|p| entity_matches(&p.target, kind, after, before));
    result
        .candidates
        .retain(|c| entity_matches(&c.entity, kind, after, before));

    result.current_summary = result
        .current_summary
        .take()
        .filter(|s| entity_matches(&RetrievedEntity::Summary(s.clone()), kind, after, before));

    let filtered_total = result.candidates.len();
    result.candidates.truncate(max as usize);
    result.provenance.total_candidates = filtered_total as u32;
    result.provenance.returned_candidates = result.candidates.len() as u32;
}

fn entity_matches(
    entity: &RetrievedEntity,
    kind: Option<ObservationKind>,
    after: Option<Timestamp>,
    before: Option<Timestamp>,
) -> bool {
    match entity {
        RetrievedEntity::Observation(obs) => {
            if let Some(k) = kind {
                if obs.kind != k {
                    return false;
                }
            }
            if let Some(a) = after {
                if obs.ts < a {
                    return false;
                }
            }
            if let Some(b) = before {
                if obs.ts >= b {
                    return false;
                }
            }
            true
        }
        RetrievedEntity::Summary(summary) => {
            if kind.is_some() {
                return false;
            }
            if let Some(a) = after {
                if summary.created_at < a {
                    return false;
                }
            }
            if let Some(b) = before {
                if summary.created_at >= b {
                    return false;
                }
            }
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kindling_types::{Observation, ScopeIds, Summary};

    fn obs(kind: ObservationKind, ts: Timestamp) -> RetrievedEntity {
        RetrievedEntity::Observation(Observation {
            id: "o".into(),
            kind,
            content: "c".into(),
            provenance: serde_json::Map::new(),
            ts,
            scope_ids: ScopeIds::default(),
            redacted: false,
        })
    }

    fn summary(created_at: Timestamp) -> RetrievedEntity {
        RetrievedEntity::Summary(Summary {
            id: "s".into(),
            capsule_id: "cap".into(),
            content: "summary".into(),
            confidence: 0.9,
            created_at,
            evidence_refs: vec![],
        })
    }

    #[test]
    fn parse_time_bound_accepts_epoch_ms_and_relative() {
        let now = 1_700_000_000_000i64;
        assert_eq!(parse_time_bound("123", now).unwrap(), 123);
        assert_eq!(
            parse_time_bound("1 week ago", now).unwrap(),
            now - 7 * 24 * 3600 * 1000
        );
        assert_eq!(
            parse_time_bound("2 days ago", now).unwrap(),
            now - 2 * 24 * 3600 * 1000
        );
        assert!(parse_time_bound("nonsense", now).is_err());
        assert!(parse_time_bound("5 fortnights ago", now).is_err());
        assert!(parse_time_bound("", now).is_err());
    }

    #[test]
    fn parse_time_bound_rejects_overflow_instead_of_panicking() {
        let now = 1_700_000_000_000i64;
        assert!(parse_time_bound("99999999999999999 weeks ago", now).is_err());
        assert!(parse_time_bound("99999999999999999 days ago", now).is_err());
    }

    #[test]
    fn kind_filter_excludes_other_kinds_and_summaries() {
        let command = obs(ObservationKind::Command, 1000);
        let error = obs(ObservationKind::Error, 1000);
        let sum = summary(1000);
        assert!(entity_matches(
            &command,
            Some(ObservationKind::Command),
            None,
            None
        ));
        assert!(!entity_matches(
            &error,
            Some(ObservationKind::Command),
            None,
            None
        ));
        assert!(!entity_matches(
            &sum,
            Some(ObservationKind::Command),
            None,
            None
        ));
    }

    #[test]
    fn time_bounds_are_half_open() {
        let at_100 = obs(ObservationKind::Message, 100);
        let at_200 = obs(ObservationKind::Message, 200);
        // [100, 200): 100 included, 200 excluded.
        assert!(entity_matches(&at_100, None, Some(100), Some(200)));
        assert!(!entity_matches(&at_200, None, Some(100), Some(200)));
        assert!(entity_matches(&at_200, None, Some(100), None));
        assert!(!entity_matches(&at_100, None, None, Some(100)));
    }

    #[test]
    fn summary_filtered_by_created_at() {
        let s = summary(100);
        assert!(entity_matches(&s, None, Some(100), Some(200)));
        assert!(!entity_matches(&s, None, None, Some(100)));
    }
}
