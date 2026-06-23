//! `clap` command tree for the kindling CLI.
//!
//! Mirrors the Commander.js definitions in
//! `packages/kindling-cli/src/index.ts` (flags + defaults). Two deliberate
//! deviations are documented inline:
//!
//! * a global `--via-daemon` flag (no TS equivalent) routes the daemon-backed
//!   verbs through `kindling-client` instead of the in-process service;
//! * `serve` maps to the UDS daemon (`--socket`/`--idle-timeout`) rather than
//!   the TS HTTP server (`--port`/`--host`/`--no-cors`), because the transport
//!   changed in the Rust port (see D-005).
//!
//! The `sync` (GitHub) commands are intentionally out of PORT-012 scope.

use clap::{Args, Parser, Subcommand};

/// Local memory and continuity engine for AI-assisted development.
#[derive(Debug, Parser)]
#[command(name = "kindling", version, about, long_about = None)]
pub struct Cli {
    /// Route daemon-backed verbs (log, capsule, search, pin, unpin, forget)
    /// through the running daemon via the UDS client instead of opening the DB
    /// in-process.
    #[arg(long, global = true)]
    pub via_daemon: bool,

    #[command(subcommand)]
    pub command: Command,
}

/// Shared `--db` / `--json` flags carried by most verbs.
#[derive(Debug, Args, Clone, Default)]
pub struct CommonOpts {
    /// Database path. Overrides `KINDLING_DB_PATH` and the per-project default.
    #[arg(long, value_name = "path")]
    pub db: Option<String>,

    /// Output as JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Initialize kindling (create database and configure hooks).
    Init(InitArgs),

    /// Log an observation to memory.
    Log(LogArgs),

    /// Manage capsules (open/close).
    #[command(subcommand)]
    Capsule(CapsuleCommand),

    /// Show database status and statistics.
    Status(StatusArgs),

    /// Search for relevant context in memory.
    Search(SearchArgs),

    /// List entities (capsules, pins, observations).
    List(ListArgs),

    /// Pin an observation or summary (type: observation|summary).
    Pin(PinArgs),

    /// Remove a pin by ID.
    Unpin(UnpinArgs),

    /// Redact (forget) an observation by ID.
    Forget(ForgetArgs),

    /// Export memory to file (default: kindling-export-<timestamp>.json).
    Export(ExportArgs),

    /// Import memory from an export file.
    Import(ImportArgs),

    /// Start the kindling daemon (HTTP/1 over a Unix domain socket).
    Serve(ServeArgs),

    /// Load sample memory for trying search and browse.
    Demo(DemoArgs),

    /// Open a local HTML viewer for memory in the database.
    Browse(BrowseArgs),
}

#[derive(Debug, Args)]
pub struct InitArgs {
    /// Database path (default: per-project under ~/.kindling).
    #[arg(long, value_name = "path")]
    pub db: Option<String>,

    /// Also configure Claude Code integration.
    #[arg(long)]
    pub claude_code: bool,

    /// Skip database creation (only configure hooks).
    #[arg(long)]
    pub skip_db: bool,

    /// Output as JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct LogArgs {
    /// Content of the observation.
    pub content: String,

    /// Observation kind (default: message).
    #[arg(long, value_name = "kind", default_value = "message")]
    pub kind: String,

    /// Session scope ID.
    #[arg(long, value_name = "id")]
    pub session: Option<String>,

    /// Repository scope ID.
    #[arg(long, value_name = "id")]
    pub repo: Option<String>,

    /// Attach to existing capsule.
    #[arg(long, value_name = "id")]
    pub capsule: Option<String>,

    #[command(flatten)]
    pub common: CommonOpts,
}

#[derive(Debug, Subcommand)]
pub enum CapsuleCommand {
    /// Open a new capsule.
    Open(CapsuleOpenArgs),
    /// Close a capsule.
    Close(CapsuleCloseArgs),
}

#[derive(Debug, Args)]
pub struct CapsuleOpenArgs {
    /// Purpose of the capsule (required).
    #[arg(long, value_name = "text")]
    pub intent: String,

    /// Capsule type (default: session).
    #[arg(long = "type", value_name = "type", default_value = "session")]
    pub kind: String,

    /// Session scope ID.
    #[arg(long, value_name = "id")]
    pub session: Option<String>,

    /// Repository scope ID.
    #[arg(long, value_name = "id")]
    pub repo: Option<String>,

    #[command(flatten)]
    pub common: CommonOpts,
}

#[derive(Debug, Args)]
pub struct CapsuleCloseArgs {
    /// Capsule ID to close.
    pub id: String,

    /// Summary text for the capsule.
    #[arg(long, value_name = "text")]
    pub summary: Option<String>,

    #[command(flatten)]
    pub common: CommonOpts,
}

#[derive(Debug, Args)]
pub struct StatusArgs {
    #[command(flatten)]
    pub common: CommonOpts,
}

#[derive(Debug, Args)]
pub struct SearchArgs {
    /// Query string.
    pub query: String,

    /// Filter by session ID.
    #[arg(long, value_name = "id")]
    pub session: Option<String>,

    /// Filter by repository ID.
    #[arg(long, value_name = "id")]
    pub repo: Option<String>,

    /// Maximum results to return.
    #[arg(long, value_name = "n", default_value_t = 10)]
    pub max: u32,

    #[command(flatten)]
    pub common: CommonOpts,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    /// Entity to list: capsules | pins | observations.
    pub entity: String,

    /// Filter by session ID.
    #[arg(long, value_name = "id")]
    pub session: Option<String>,

    /// Filter by repository ID.
    #[arg(long, value_name = "id")]
    pub repo: Option<String>,

    /// Maximum results to return.
    #[arg(long, value_name = "n", default_value_t = 20)]
    pub limit: u32,

    #[command(flatten)]
    pub common: CommonOpts,
}

#[derive(Debug, Args)]
pub struct PinArgs {
    /// Target type: observation | summary.
    #[arg(value_name = "type")]
    pub target_type: String,

    /// Target id (observation or summary).
    pub id: String,

    /// Note describing why this is pinned.
    #[arg(long, value_name = "text")]
    pub note: Option<String>,

    /// Time-to-live in milliseconds.
    #[arg(long, value_name = "ms")]
    pub ttl: Option<i64>,

    #[command(flatten)]
    pub common: CommonOpts,
}

#[derive(Debug, Args)]
pub struct UnpinArgs {
    /// Pin ID to remove.
    pub id: String,

    #[command(flatten)]
    pub common: CommonOpts,
}

#[derive(Debug, Args)]
pub struct ForgetArgs {
    /// Observation ID to redact (exact id; no prefix matching).
    pub id: String,

    #[command(flatten)]
    pub common: CommonOpts,
}

#[derive(Debug, Args)]
pub struct ExportArgs {
    /// Output file (default: kindling-export-<timestamp>.json).
    pub output: Option<String>,

    /// Export only a specific session.
    #[arg(long, value_name = "id")]
    pub session: Option<String>,

    /// Export only a specific repository.
    #[arg(long, value_name = "id")]
    pub repo: Option<String>,

    /// Pretty-print JSON output.
    #[arg(long)]
    pub pretty: bool,

    /// Timestamp (epoch ms) to stamp into the bundle and the default filename.
    /// Defaults to the current time; surfaced as a flag for deterministic tests
    /// (the TS CLI used `Date.now()` directly with no override).
    #[arg(long, value_name = "ms", hide = true)]
    pub timestamp: Option<i64>,

    #[command(flatten)]
    pub common: CommonOpts,
}

#[derive(Debug, Args)]
pub struct ImportArgs {
    /// Bundle file to import.
    pub file: String,

    /// Validate without importing.
    #[arg(long)]
    pub dry_run: bool,

    #[command(flatten)]
    pub common: CommonOpts,
}

#[derive(Debug, Args)]
pub struct ServeArgs {
    /// Unix domain socket to bind (default: ~/.kindling/kindling.sock).
    #[arg(long, value_name = "path")]
    pub socket: Option<String>,

    /// Idle timeout in seconds before the daemon shuts itself down.
    #[arg(long, value_name = "secs", default_value_t = 1800)]
    pub idle_timeout: u64,

    /// kindling home (root of per-project databases). Defaults to ~/.kindling
    /// (or the parent of `--socket` when given).
    #[arg(long, value_name = "path")]
    pub kindling_home: Option<String>,

    /// Run as a background daemon: suppress the human-readable startup banner.
    ///
    /// This is the flag `kindling-client` passes when it auto-spawns the daemon
    /// on first call (`kindling serve --daemonize`). Detachment from the calling
    /// process (its stdio and process group) is owned by the spawner; this flag
    /// only silences the banner so it never corrupts a caller's stdout (e.g. a
    /// Claude Code hook writing JSON to stdout).
    #[arg(long)]
    pub daemonize: bool,
}

#[derive(Debug, Args)]
pub struct DemoArgs {
    /// Replace an existing demo database before importing sample memory.
    #[arg(long)]
    pub reset: bool,

    #[command(flatten)]
    pub common: CommonOpts,
}

#[derive(Debug, Args)]
pub struct BrowseArgs {
    /// Write HTML to this path instead of a temp file.
    #[arg(long, value_name = "path")]
    pub output: Option<String>,

    /// Print the output path only; do not open a browser.
    #[arg(long)]
    pub no_open: bool,

    #[command(flatten)]
    pub common: CommonOpts,
}
