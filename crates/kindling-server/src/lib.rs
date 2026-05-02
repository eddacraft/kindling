//! Kindling daemon — HTTP API over Unix domain socket.
//!
//! Long-running per-user process. Listens on `~/.kindling/kindling.sock`
//! (mode `0600`) on Unix; localhost TCP fallback on Windows. Auto-spawned
//! by clients, shuts down on idle. Routes per-project to the right SQLite DB.
//!
//! Filled in by PORT-007.
