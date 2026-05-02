//! Rust client for the Kindling daemon.
//!
//! Speaks HTTP/1 over the Unix domain socket at `~/.kindling/kindling.sock`
//! (TCP fallback on Windows). On `ECONNREFUSED` or missing socket, spawns
//! `kindling serve --daemonize` and polls until the socket appears. Method
//! shape mirrors `kindling-service` for ergonomic in-process / via-daemon
//! interchangeability.
//!
//! Filled in by PORT-008.
