//! `spool` — inspect durable-emit spool status from an on-disk NDJSON file.

use kindling_client::spool::SpooledClient;

use crate::cli::SpoolStatusArgs;
use crate::output::format_json;
use crate::CliResult;

pub fn run_status(args: SpoolStatusArgs) -> CliResult {
    let status = SpooledClient::spool_status_from_path(&args.spool_path)?;

    if args.json {
        println!("{}", format_json(&status, true)?);
    } else {
        println!("\nKindling Spool Status");
        println!("=====================\n");
        println!("Spool path:       {}", status.spool_path.display());
        println!("Pending count:    {}", status.pending_count);
        println!("Replay attempts:  {}", status.replay_attempts);
        match status.last_flush_time_ms {
            Some(ts) => println!("Last flush (ms):  {ts}"),
            None => println!("Last flush (ms):  —"),
        }
        match &status.last_error {
            Some(err) => println!("Last error:       {err}"),
            None => println!("Last error:       —"),
        }
        println!();
    }
    Ok(())
}
