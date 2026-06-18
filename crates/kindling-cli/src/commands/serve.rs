//! `serve` — start the Kindling daemon.
//!
//! **Deliberate deviation from the TS CLI.** The TS `serve` ran a Fastify HTTP
//! server bound to `--host:--port` with optional CORS. The Rust port (D-005)
//! replaced that transport with a long-running per-user daemon listening on a
//! Unix domain socket (TCP fallback on Windows). So this verb maps to
//! [`kindling_server::serve`] with `--socket`/`--idle-timeout`/`--kindling-home`
//! instead of `--port`/`--host`/`--no-cors`. The umbrella `kindling serve`
//! dispatch is PORT-013; here we just wrap the server crate.

use std::path::PathBuf;
use std::time::Duration;

use kindling_server::{serve, ServerConfig};

use crate::cli::ServeArgs;
use crate::{runtime, CliError, CliResult};

pub fn run(args: ServeArgs) -> CliResult {
    let config = build_config(&args)?;

    println!("Starting Kindling daemon...");
    println!("Socket: {}", config.socket_path.display());
    println!("Kindling home: {}", config.kindling_home.display());
    println!("Idle timeout: {}s", config.idle_timeout.as_secs());
    println!();

    runtime()?.block_on(async { serve(config).await })?;
    Ok(())
}

/// Resolve the daemon config from the verb's flags.
///
/// Resolution: `--kindling-home` if given; else the parent of `--socket` when a
/// socket is given; else the default `~/.kindling`. The socket defaults to
/// `<home>/kindling.sock` (via `ServerConfig::new`) unless `--socket` overrides
/// it.
fn build_config(args: &ServeArgs) -> Result<ServerConfig, CliError> {
    let home = if let Some(home) = &args.kindling_home {
        PathBuf::from(home)
    } else if let Some(socket) = &args.socket {
        PathBuf::from(socket)
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    } else {
        kindling_store::default_kindling_home().ok_or_else(|| {
            CliError::Invalid(
                "could not determine kindling home (no HOME/USERPROFILE); pass --kindling-home"
                    .to_string(),
            )
        })?
    };

    let mut config = ServerConfig::new(home);
    if let Some(socket) = &args.socket {
        config.socket_path = PathBuf::from(socket);
    }
    config.idle_timeout = Duration::from_secs(args.idle_timeout);
    Ok(config)
}

use std::path::Path;
