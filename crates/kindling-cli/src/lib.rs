//! Kindling CLI commands.
//!
//! Defines the `clap` command tree and handlers for the 12 CLI verbs. Default
//! is in-process via `kindling-service`; `--via-daemon` switches to
//! `kindling-client` for safe concurrent use alongside other Kindling tools.
//! Wired into the umbrella binary by PORT-013.
//!
//! Filled in by PORT-012.
