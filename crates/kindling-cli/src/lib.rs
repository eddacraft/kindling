//! Kindling CLI commands.
//!
//! Defines the `clap` command tree ([`Cli`]) and handlers for the 12 CLI verbs.
//! The default execution mode is **in-process** via [`kindling_service`]; the
//! global `--via-daemon` flag switches the daemon-backed verbs (log, capsule
//! open/close, search, pin, unpin, forget) to route through [`kindling_client`]
//! for safe concurrent use alongside other Kindling tools.
//!
//! `export`/`import`/`status`/`list`/`init` are always in-process (they operate
//! on the DB file directly or have no daemon endpoint). `serve` starts the UDS
//! daemon via [`kindling_server::serve`].
//!
//! Wired into the umbrella `kindling` binary by PORT-013; here the crate ships
//! its own `kindling-cli` bin plus this library so the dispatch is testable.

mod cli;
mod commands;
mod output;

pub use cli::{
    CapsuleCloseArgs, CapsuleCommand, CapsuleOpenArgs, Cli, Command, CommonOpts, ExportArgs,
    ForgetArgs, ImportArgs, InitArgs, ListArgs, LogArgs, PinArgs, SearchArgs, ServeArgs,
    StatusArgs, UnpinArgs,
};

use std::path::PathBuf;

use kindling_service::KindlingService;

/// CLI-level errors. Most variants wrap a lower-layer error; the CLI surface
/// converts these to a printed message + non-zero exit code.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error(transparent)]
    Service(#[from] kindling_service::ServiceError),

    #[error(transparent)]
    Store(#[from] kindling_store::StoreError),

    #[error(transparent)]
    Client(#[from] kindling_client::ClientError),

    #[error(transparent)]
    Server(#[from] kindling_server::ServerError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    /// A raw SQLite failure (the `list` verb's raw-row reads use rusqlite
    /// directly to reproduce the TS CLI's raw-column JSON shape).
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// A bad argument value (e.g. unknown observation kind / entity / pin type),
    /// matching the messages the TS commands throw.
    #[error("{0}")]
    Invalid(String),
}

/// A CLI result. The `Ok` payload is the process exit code (0 = success).
pub type CliResult = Result<(), CliError>;

/// Resolve the database path for an in-process command.
///
/// Resolution order (documented in the PORT-012 brief):
/// 1. an explicit `--db <path>`;
/// 2. the `KINDLING_DB_PATH` environment override;
/// 3. the per-project default `project_db_path(default_kindling_home(), cwd)`
///    via [`kindling_store::resolve_db_path`].
///
/// This intentionally prefers the per-project store layout over the TS
/// single-DB default (`~/.kindling/kindling.db`): the Rust daemon model is
/// per-project, and matching `resolve_db_path` keeps the CLI and daemon pointed
/// at the same database for a given project root.
pub fn resolve_db_path(explicit: Option<&str>) -> Result<PathBuf, CliError> {
    if let Some(path) = explicit {
        return Ok(PathBuf::from(path));
    }
    let cwd = std::env::current_dir()?;
    let project_root = cwd.to_string_lossy();
    kindling_store::resolve_db_path(&project_root).ok_or_else(|| {
        CliError::Invalid(
            "could not resolve a database path: no --db, no KINDLING_DB_PATH, \
             and no home directory (HOME/USERPROFILE) to derive a per-project path"
                .to_string(),
        )
    })
}

/// Open an in-process service at the resolved DB path, creating the parent
/// directory if needed.
pub fn open_service(explicit_db: Option<&str>) -> Result<(KindlingService, PathBuf), CliError> {
    let path = resolve_db_path(explicit_db)?;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let service = KindlingService::open(&path)?;
    Ok((service, path))
}

/// Parse args from the process and execute, returning the desired exit code.
///
/// The bin (`main.rs`) calls this and `std::process::exit`s with the result.
pub fn main() -> i32 {
    let cli = <Cli as clap::Parser>::parse();
    run(cli)
}

/// Execute a parsed [`Cli`], printing output and returning an exit code.
///
/// Each command handler does its own printing (text or `--json`); on error we
/// print in the same `Error:` / `{"error":…}` shape the TS CLI used and return
/// exit code 1, matching `handleError`'s `process.exit(1)`.
pub fn run(cli: Cli) -> i32 {
    let as_json = json_flag(&cli.command);
    match dispatch(cli) {
        Ok(()) => 0,
        Err(err) => {
            output::print_error(&err.to_string(), as_json);
            1
        }
    }
}

/// Whether the selected command was invoked with `--json` (so errors print in
/// JSON too, matching the TS `handleError(error, options.json)`).
fn json_flag(command: &Command) -> bool {
    match command {
        Command::Init(a) => a.json,
        Command::Log(a) => a.common.json,
        Command::Capsule(CapsuleCommand::Open(a)) => a.common.json,
        Command::Capsule(CapsuleCommand::Close(a)) => a.common.json,
        Command::Status(a) => a.common.json,
        Command::Search(a) => a.common.json,
        Command::List(a) => a.common.json,
        Command::Pin(a) => a.common.json,
        Command::Unpin(a) => a.common.json,
        Command::Forget(a) => a.common.json,
        Command::Export(a) => a.common.json,
        Command::Import(a) => a.common.json,
        Command::Serve(_) => false,
    }
}

fn dispatch(cli: Cli) -> CliResult {
    let via_daemon = cli.via_daemon;
    match cli.command {
        Command::Init(args) => commands::init::run(args),
        Command::Log(args) => commands::log::run(args, via_daemon),
        Command::Capsule(CapsuleCommand::Open(args)) => {
            commands::capsule::run_open(args, via_daemon)
        }
        Command::Capsule(CapsuleCommand::Close(args)) => {
            commands::capsule::run_close(args, via_daemon)
        }
        Command::Status(args) => commands::status::run(args),
        Command::Search(args) => commands::search::run(args, via_daemon),
        Command::List(args) => commands::list::run(args),
        Command::Pin(args) => commands::pin::run_pin(args, via_daemon),
        Command::Unpin(args) => commands::pin::run_unpin(args, via_daemon),
        Command::Forget(args) => commands::forget::run(args, via_daemon),
        Command::Export(args) => commands::export::run_export(args, via_daemon),
        Command::Import(args) => commands::export::run_import(args, via_daemon),
        Command::Serve(args) => commands::serve::run(args),
    }
}

/// Build a daemon client honouring `--db` as a socket-routing project hint.
///
/// The client routes by project root (hashed into a per-project DB by the
/// daemon), not by an explicit DB file, so `--via-daemon` + `--db` cannot point
/// the daemon at an arbitrary file. We use the default socket + the current
/// working directory as the project root (mirroring `ClientConfig::defaults`).
pub(crate) fn build_client() -> Result<kindling_client::Client, CliError> {
    Ok(kindling_client::Client::new()?)
}

/// A small single-threaded Tokio runtime for the async client/serve paths.
pub(crate) fn runtime() -> std::io::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
}
