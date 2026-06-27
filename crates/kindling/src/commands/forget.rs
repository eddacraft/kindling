//! `forget <id>` — redact (forget) an observation.
//!
//! In-process this opens a [`KindlingService`](kindling_service::KindlingService)
//! at the resolved DB path and calls `forget`. `--via-daemon` routes through the
//! daemon client. The `id` is an exact observation id (no prefix matching at this
//! layer — the plugin wrapper resolves prefixes separately).
//!
//! Note: redaction is not idempotent at the store layer (the FTS update trigger
//! keys its delete on the old content), so forgetting an already-redacted id
//! errors. Callers that may re-forget must dedup first.

use crate::cli::ForgetArgs;
use crate::{build_client, open_service, runtime, CliResult};

pub fn run(args: ForgetArgs, via_daemon: bool) -> CliResult {
    if via_daemon {
        let client = build_client(args.common.db.as_deref())?;
        runtime()?.block_on(async { client.forget(&args.id).await })?;
    } else {
        let (service, _db) = open_service(args.common.db.as_deref())?;
        service.forget(&args.id)?;
    }

    if args.common.json {
        // Compact JSON matching the sibling commands' success shapes.
        let value = serde_json::json!({ "redacted": true, "id": args.id });
        println!("{}", serde_json::to_string(&value)?);
    } else {
        println!("\nRedacted observation {}\n", args.id);
    }
    Ok(())
}
