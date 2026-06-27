//! `capsule open` / `capsule close <id>` — capsule lifecycle.
//!
//! Ports `packages/kindling-cli/src/commands/capsule.ts`. `--via-daemon` routes
//! through the daemon client.

use kindling_client::CloseCapsuleBody;
use kindling_service::{CloseCapsuleOptions, OpenCapsuleOptions};
use kindling_types::CapsuleType;

use crate::cli::{CapsuleCloseArgs, CapsuleOpenArgs};
use crate::commands::scope_from;
use crate::output::{format_json, format_timestamp};
use crate::{build_client, open_service, runtime, CliError, CliResult};

/// Parse a capsule-type string the way the TS `isCapsuleType` guard does,
/// reproducing its error message on failure.
pub(crate) fn parse_type(kind: &str) -> Result<CapsuleType, CliError> {
    serde_json::from_value::<CapsuleType>(serde_json::Value::String(kind.to_string())).map_err(
        |_| {
            let all = CapsuleType::ALL
                .iter()
                .map(type_to_str)
                .collect::<Vec<_>>()
                .join(", ");
            CliError::Invalid(format!("Invalid capsule type: {kind}. Valid types: {all}"))
        },
    )
}

fn type_to_str(kind: &CapsuleType) -> &'static str {
    match kind {
        CapsuleType::Session => "session",
        CapsuleType::PocketflowNode => "pocketflow_node",
    }
}

fn status_to_str(status: kindling_types::CapsuleStatus) -> &'static str {
    match status {
        kindling_types::CapsuleStatus::Open => "open",
        kindling_types::CapsuleStatus::Closed => "closed",
    }
}

pub fn run_open(args: CapsuleOpenArgs, via_daemon: bool) -> CliResult {
    let kind = parse_type(&args.kind)?;
    let scope = scope_from(args.session.as_deref(), args.repo.as_deref());

    let capsule = if via_daemon {
        let client = build_client(args.common.db.as_deref())?;
        runtime()?.block_on(async {
            client
                .open_capsule(kind, args.intent.clone(), scope, None)
                .await
        })?
    } else {
        let (service, _db) = open_service(args.common.db.as_deref())?;
        service.open_capsule(OpenCapsuleOptions {
            kind,
            intent: args.intent.clone(),
            scope_ids: scope,
            id: None,
        })?
    };

    if args.common.json {
        println!("{}", format_json(&capsule, true)?);
    } else {
        println!("\nCapsule opened successfully");
        println!("ID: {}", capsule.id);
        println!("Type: {}", type_to_str(&capsule.kind));
        println!("Intent: {}", capsule.intent);
        println!("Status: {}", status_to_str(capsule.status));
        println!("Opened at: {}", format_timestamp(capsule.opened_at));
        println!();
    }
    Ok(())
}

pub fn run_close(args: CapsuleCloseArgs, via_daemon: bool) -> CliResult {
    let capsule = if via_daemon {
        let client = build_client(args.common.db.as_deref())?;
        let body = match &args.summary {
            Some(content) => CloseCapsuleBody {
                generate_summary: Some(true),
                summary_content: Some(content.clone()),
                confidence: None,
            },
            None => CloseCapsuleBody::default(),
        };
        runtime()?.block_on(async { client.close_capsule(&args.id, body).await })?
    } else {
        let (service, _db) = open_service(args.common.db.as_deref())?;
        let options = match &args.summary {
            Some(content) => CloseCapsuleOptions {
                generate_summary: true,
                summary_content: Some(content.clone()),
                confidence: None,
            },
            None => CloseCapsuleOptions::default(),
        };
        service.close_capsule(&args.id, options)?
    };

    if args.common.json {
        println!("{}", format_json(&capsule, true)?);
    } else {
        println!("\nCapsule closed successfully");
        println!("ID: {}", capsule.id);
        println!("Status: {}", status_to_str(capsule.status));
        if let Some(closed_at) = capsule.closed_at {
            println!("Closed at: {}", format_timestamp(closed_at));
        }
        if let Some(summary_id) = &capsule.summary_id {
            println!("Summary: {summary_id}");
        }
        println!();
    }
    Ok(())
}
