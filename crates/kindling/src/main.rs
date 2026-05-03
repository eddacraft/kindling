//! Kindling — single-binary entry point.
//!
//! Dispatches to `serve`, `hook`, or one of the CLI verbs based on the
//! invoked subcommand. Wired up by PORT-013.

fn main() {
    println!("kindling {}", env!("CARGO_PKG_VERSION"));
}
