//! Content filtering for Kindling observations.
//!
//! Secret masking, length truncation, excluded-path filtering. Owned by the
//! server side (daemon) so non-Rust consumers cannot accidentally bypass the
//! redactions.
//!
//! Filled in by PORT-004.
