//! `kindling-cli` binary entry point.
//!
//! Thin wrapper over [`kindling_cli::main`] so the dispatch + command handlers
//! live in the library (and stay integration-testable). The umbrella
//! `kindling <verb>` dispatch is PORT-013.

fn main() {
    std::process::exit(kindling_cli::main());
}
