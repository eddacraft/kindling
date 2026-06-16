//! Client error type.

use std::io;

use thiserror::Error;

/// Errors returned by [`Client`](crate::Client) operations.
#[derive(Debug, Error)]
pub enum ClientError {
    /// The daemon could not be reached or spawned within the connect budget.
    ///
    /// Carries a human-readable explanation (e.g. "spawner failed" or
    /// "socket never appeared within 1s").
    #[error("kindling daemon unavailable: {0}")]
    Unavailable(String),

    /// A transport-level failure talking HTTP/1 over the socket (hyper).
    #[error("http transport error: {0}")]
    Http(String),

    /// The daemon returned a non-2xx response. The `message` is the daemon's
    /// `{ "error": "<msg>" }` body when present, else the raw body or a status
    /// phrase.
    #[error("daemon returned {status}: {message}")]
    Api {
        /// HTTP status code.
        status: u16,
        /// Error message extracted from the daemon's JSON body.
        message: String,
    },

    /// The daemon's reported `schemaVersion` does not match the version this
    /// client was built/configured to expect. Fail loud rather than risk
    /// silent contract drift.
    #[error("schema version mismatch: client expected {expected}, daemon reports {actual}")]
    SchemaMismatch {
        /// Schema version the client expects.
        expected: u32,
        /// Schema version the daemon reports.
        actual: u32,
    },

    /// A 2xx response body could not be decoded into the expected type.
    #[error("failed to decode daemon response: {0}")]
    Decode(String),

    /// A low-level I/O error (socket connect, spawn) not otherwise classified.
    #[error("io error: {0}")]
    Io(#[from] io::Error),
}
