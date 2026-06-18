//! `log <content>` — append an observation.
//!
//! Ports `packages/kindling-cli/src/commands/log.ts`. The observation carries
//! `provenance = { source: "cli" }` exactly as the TS command did. In
//! `--via-daemon` mode the append is routed through the daemon client.

use kindling_service::AppendObservationOptions;
use kindling_types::{ObservationInput, ObservationKind};

use crate::cli::LogArgs;
use crate::commands::scope_from;
use crate::output::{format_json, format_timestamp};
use crate::{build_client, open_service, runtime, CliError, CliResult};

/// Parse an observation-kind string the way the TS `isObservationKind` guard
/// gates it, producing the same error message on failure.
pub(crate) fn parse_kind(kind: &str) -> Result<ObservationKind, CliError> {
    serde_json::from_value::<ObservationKind>(serde_json::Value::String(kind.to_string())).map_err(
        |_| {
            let all = ObservationKind::ALL
                .iter()
                .map(kind_to_str)
                .collect::<Vec<_>>()
                .join(", ");
            CliError::Invalid(format!("Invalid kind: '{kind}'. Must be one of: {all}"))
        },
    )
}

fn kind_to_str(kind: &ObservationKind) -> &'static str {
    match kind {
        ObservationKind::ToolCall => "tool_call",
        ObservationKind::Command => "command",
        ObservationKind::FileDiff => "file_diff",
        ObservationKind::Error => "error",
        ObservationKind::Message => "message",
        ObservationKind::NodeStart => "node_start",
        ObservationKind::NodeEnd => "node_end",
        ObservationKind::NodeOutput => "node_output",
        ObservationKind::NodeError => "node_error",
    }
}

/// CLI provenance object: `{ "source": "cli" }` (matches the TS command).
fn cli_provenance() -> serde_json::Map<String, serde_json::Value> {
    let mut map = serde_json::Map::new();
    map.insert(
        "source".to_string(),
        serde_json::Value::String("cli".to_string()),
    );
    map
}

pub fn run(args: LogArgs, via_daemon: bool) -> CliResult {
    let kind = parse_kind(&args.kind)?;
    let scope = scope_from(args.session.as_deref(), args.repo.as_deref());

    let input = ObservationInput {
        id: None,
        kind,
        content: args.content.clone(),
        provenance: Some(cli_provenance()),
        ts: None,
        scope_ids: scope,
        redacted: Some(false),
    };

    let observation = if via_daemon {
        let client = build_client()?;
        runtime()?.block_on(async {
            client
                .append_observation(input, args.capsule.clone(), None)
                .await
        })?
    } else {
        let (service, _db) = open_service(args.common.db.as_deref())?;
        service.append_observation(
            input,
            AppendObservationOptions {
                capsule_id: args.capsule.clone(),
                validate: true,
            },
        )?
    };

    if args.common.json {
        println!("{}", format_json(&observation, true)?);
    } else {
        let truncated = if observation.content.chars().count() > 80 {
            let prefix: String = observation.content.chars().take(77).collect();
            format!("{prefix}...")
        } else {
            observation.content.clone()
        };
        println!("\nObservation logged");
        println!("ID:        {}", observation.id);
        println!("Kind:      {}", kind_to_str(&observation.kind));
        println!("Timestamp: {}", format_timestamp(observation.ts));
        println!("Content:   {truncated}");
        println!();
    }

    Ok(())
}
