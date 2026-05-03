//! In-process Kindling orchestration.
//!
//! `KindlingService` exposes capsule lifecycle, observation append, retrieval,
//! and pin management. Consumed in-process by the daemon (`kindling-server`),
//! by the CLI, and directly by Rust integrators (e.g. Anvil headless flows).
//!
//! Filled in by PORT-006.
