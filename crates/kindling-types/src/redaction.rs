//! Redaction evidence for append responses.
//!
//! Proves sensitive data was handled without leaking the values: a count of
//! masked matches plus the matched secret *classes* (never the matched
//! substrings). The evidence is derived from the same masking pass that redacts
//! content at the service boundary, so it cannot be bypassed or drift from what
//! was actually masked.

use serde::{Deserialize, Serialize};

#[cfg(feature = "ts-rs")]
use ts_rs::TS;

/// Evidence that secret masking ran over an appended observation's content.
///
/// `count` is the number of masked matches; `classes` are the distinct secret
/// classes that matched, in detection order (e.g. `"bearerToken"`). It never
/// carries the matched substrings — only how many and of what kind.
///
/// When nothing matched this is the default (`count: 0`, `classes: []`), so the
/// field is always present on the wire and callers can treat its absence and an
/// empty evidence block identically.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "camelCase")]
pub struct RedactionEvidence {
    /// Total number of masked matches across all secret patterns.
    pub count: u32,
    /// Distinct secret classes that matched, in detection order. Stable,
    /// machine-readable identifiers (e.g. `"credentialAssignment"`,
    /// `"bearerToken"`), never the matched substrings.
    pub classes: Vec<String>,
}
