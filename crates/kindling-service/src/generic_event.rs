//! Server-side digest framing for `generic-event@1` (KINTEG-015a).
//!
//! Only the server computes digests (never exposed as a public client helper).
//! The framed message `M` is:
//!
//! 1. Domain tag: `kindling.generic-event.v1\0` (26 ASCII bytes)
//! 2. `schemaName`: `u32be(len) || utf8`
//! 3. `schemaVersion`: `u32be`
//! 4–9. `projectId`, `sessionId`, `repoId`, `agentId`, `userId`, `taskId`:
//!    `u32be(len) || utf8` (length 0 means absent)
//! 10. `payload`: `u32be(len) || raw bytes`

use kindling_types::GenericEventScope;
use sha2::{Digest, Sha256};

/// 26-byte domain tag: ASCII `kindling.generic-event.v1` followed by `0x00`.
pub const GENERIC_EVENT_DIGEST_DOMAIN_TAG: &[u8] = b"kindling.generic-event.v1\0";

fn push_len_prefixed(buf: &mut Vec<u8>, bytes: &[u8]) {
    let len = u32::try_from(bytes.len()).expect("generic-event field fits in u32");
    buf.extend_from_slice(&len.to_be_bytes());
    buf.extend_from_slice(bytes);
}

fn push_opt_str(buf: &mut Vec<u8>, value: Option<&str>) {
    match value {
        Some(s) => push_len_prefixed(buf, s.as_bytes()),
        None => buf.extend_from_slice(&0u32.to_be_bytes()),
    }
}

/// Build the framed message `M` for `generic-event@1` digest computation.
#[doc(hidden)]
pub fn frame_generic_event_digest_input(
    schema_name: &str,
    schema_version: u32,
    scope: &GenericEventScope,
    payload: &[u8],
) -> Vec<u8> {
    let mut m = Vec::with_capacity(
        GENERIC_EVENT_DIGEST_DOMAIN_TAG.len()
            + 4
            + schema_name.len()
            + 4
            + 4
            + scope.project_id.len()
            + 20
            + 4
            + payload.len(),
    );
    m.extend_from_slice(GENERIC_EVENT_DIGEST_DOMAIN_TAG);
    push_len_prefixed(&mut m, schema_name.as_bytes());
    m.extend_from_slice(&schema_version.to_be_bytes());
    push_len_prefixed(&mut m, scope.project_id.as_bytes());
    push_opt_str(&mut m, scope.session_id.as_deref());
    push_opt_str(&mut m, scope.repo_id.as_deref());
    push_opt_str(&mut m, scope.agent_id.as_deref());
    push_opt_str(&mut m, scope.user_id.as_deref());
    push_opt_str(&mut m, scope.task_id.as_deref());
    push_len_prefixed(&mut m, payload);
    m
}

/// Compute `sha256:<64 lowercase hex>` over the framed `generic-event@1` message.
#[doc(hidden)]
pub fn compute_generic_event_digest(
    schema_name: &str,
    schema_version: u32,
    scope: &GenericEventScope,
    payload: &[u8],
) -> String {
    let m = frame_generic_event_digest_input(schema_name, schema_version, scope, payload);
    let hash = Sha256::digest(&m);
    let mut out = String::with_capacity(7 + 64);
    out.push_str("sha256:");
    for byte in hash {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}
