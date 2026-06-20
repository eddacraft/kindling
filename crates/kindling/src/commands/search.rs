//! `search <query>` — deterministic retrieval.
//!
//! Ports `packages/kindling-cli/src/commands/search.ts`. `--via-daemon` routes
//! through the daemon client. The `--json` output is the full `RetrieveResult`
//! (already camelCase via its `Serialize`).

use kindling_types::{RetrieveOptions, RetrievedEntity};

use crate::cli::SearchArgs;
use crate::commands::scope_from;
use crate::output::{format_json, format_timestamp, truncate};
use crate::{build_client, open_service, runtime, CliResult};

pub fn run(args: SearchArgs, via_daemon: bool) -> CliResult {
    let scope = scope_from(args.session.as_deref(), args.repo.as_deref());
    let options = RetrieveOptions {
        query: args.query.clone(),
        scope_ids: scope,
        token_budget: None,
        max_candidates: Some(args.max),
        include_redacted: None,
    };

    let result = if via_daemon {
        let client = build_client()?;
        runtime()?.block_on(async { client.retrieve(options).await })?
    } else {
        let (service, _db) = open_service(args.common.db.as_deref())?;
        service.retrieve(options)?
    };

    if args.common.json {
        println!("{}", format_json(&result, true)?);
    } else {
        println!("\nSearch Results for: \"{}\"", args.query);
        println!("{}\n", "=".repeat(50));

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
