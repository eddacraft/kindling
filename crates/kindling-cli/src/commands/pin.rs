//! `pin <type> <id>` / `unpin <id>` — manage pins.
//!
//! Ports `packages/kindling-cli/src/commands/pin.ts`. `--via-daemon` routes
//! through the daemon client.

use kindling_client::CreatePinBody;
use kindling_service::CreatePinOptions;
use kindling_types::PinTargetType;

use crate::cli::{PinArgs, UnpinArgs};
use crate::output::{format_json, iso8601_utc};
use crate::{build_client, open_service, runtime, CliError, CliResult};

fn parse_target_type(value: &str) -> Result<PinTargetType, CliError> {
    match value {
        "observation" => Ok(PinTargetType::Observation),
        "summary" => Ok(PinTargetType::Summary),
        other => Err(CliError::Invalid(format!(
            "Invalid type: {other}. Must be 'observation' or 'summary'"
        ))),
    }
}

fn target_type_str(t: PinTargetType) -> &'static str {
    match t {
        PinTargetType::Observation => "observation",
        PinTargetType::Summary => "summary",
    }
}

pub fn run_pin(args: PinArgs, via_daemon: bool) -> CliResult {
    let target_type = parse_target_type(&args.target_type)?;

    let pin = if via_daemon {
        let client = build_client()?;
        let body = CreatePinBody {
            target_type,
            target_id: args.id.clone(),
            note: args.note.clone(),
            ttl_ms: args.ttl,
            scope_ids: None,
        };
        runtime()?.block_on(async { client.pin(body).await })?
    } else {
        let (service, _db) = open_service(args.common.db.as_deref())?;
        service.pin(CreatePinOptions {
            target_type,
            target_id: args.id.clone(),
            note: args.note.clone(),
            ttl_ms: args.ttl,
            scope_ids: None,
        })?
    };

    if args.common.json {
        println!("{}", format_json(&pin, true)?);
    } else {
        println!("\nPin created successfully");
        println!("ID: {}", pin.id);
        println!(
            "Target: {} {}",
            target_type_str(pin.target_type),
            pin.target_id
        );
        if let Some(reason) = &pin.reason {
            println!("Note: {reason}");
        }
        if let Some(expires_at) = pin.expires_at {
            println!("Expires: {}", iso8601_utc(expires_at));
        }
        println!();
    }
    Ok(())
}

pub fn run_unpin(args: UnpinArgs, via_daemon: bool) -> CliResult {
    if via_daemon {
        let client = build_client()?;
        runtime()?.block_on(async { client.unpin(&args.id).await })?;
    } else {
        let (service, _db) = open_service(args.common.db.as_deref())?;
        service.unpin(&args.id)?;
    }

    if args.common.json {
        // Matches the TS `formatJson({ success: true, pinId: id })` — compact.
        let value = serde_json::json!({ "success": true, "pinId": args.id });
        println!("{}", serde_json::to_string(&value)?);
    } else {
        println!("\nPin {} removed successfully\n", args.id);
    }
    Ok(())
}
