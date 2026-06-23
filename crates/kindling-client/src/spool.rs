//! A thin, opt-in **durable-emit** layer over [`crate`].
//!
//! [`crate::Client::append_observation`] returns
//! [`ClientError::Unavailable`](crate::ClientError::Unavailable) when
//! the daemon is down. A producer that wants its observations to survive a
//! daemon outage therefore has to reinvent a local fallback (this is exactly
//! why anvil grew a `usage.ndjson`). [`SpooledClient`] centralizes that once.
//!
//! # The contract
//!
//! **The daemon (SQLite) is always the authoritative store. The spool is a
//! transient, append-only write buffer — never a parallel source of truth.**
//!
//! [`SpooledClient::append_observation`] tries the socket; on a connectivity
//! failure it appends the observation to a local NDJSON spool file. The spool
//! is drained into the daemon by [`SpooledClient::flush`] (and opportunistically
//! at the start of the next *successful* `append_observation`). NDJSON exists
//! only as a fallback buffer and a debuggable / importable wire format.
//!
//! # Delivery semantics
//!
//! Delivery is **exactly-once-ish on id**. Before trying or spooling,
//! `append_observation` assigns a stable [`uuid::Uuid`] v4 to the observation
//! when the caller left `input.id` as `None`, so a spooled entry and any later
//! replay carry the *same* id. A crash after the daemon commits but before the
//! spool is rewritten can therefore replay an already-stored observation — but
//! the daemon now **deduplicates** on id: a write whose id already exists is
//! ignored (the stored row is returned untouched, never overwritten or
//! re-masked), surfaced via [`AppendResult::deduplicated`](crate::AppendResult).
//! So a replay is an observable no-op rather than a duplicate row.
//!
//! Dedup only protects the **committed-but-not-yet-drained** window: an id was
//! already assigned, the daemon stored the row, and a crash left a stale spool
//! entry to replay. It does **not** make delivery durable end-to-end. In
//! particular, an observation that is lost *before* its id reaches durable
//! state — e.g. a crash between id assignment and the spool append, or a write
//! the caller never spooled — is simply lost: there is nothing to replay, so
//! nothing to dedup. So this is "at-most-once delivery of each *attempt*, with
//! exactly-once *application* of whatever does reach the daemon", not
//! at-least-once durability. ("-ish" also because the dedup key is the
//! observation id; two genuinely distinct writes that reuse an id would
//! collapse to one.)
//!
//! # Which failures spool vs propagate
//!
//! Only *connectivity* failures buffer to the spool:
//!
//! - [`ClientError::Unavailable`](crate::ClientError::Unavailable) and
//!   [`ClientError::Http`](crate::ClientError::Http) → spool
//!   ([`AppendOutcome::Spooled`]).
//! - [`ClientError::Api`](crate::ClientError::Api),
//!   [`ClientError::SchemaMismatch`](crate::ClientError::SchemaMismatch),
//!   [`ClientError::Decode`](crate::ClientError::Decode), and
//!   [`ClientError::Io`](crate::ClientError::Io) → propagate
//!   ([`SpoolError::Client`]). The daemon *responded*; a rejected observation
//!   must never be spooled or it would loop forever on every flush.
//!
//! # Concurrency
//!
//! A spool file is **single-producer**: one [`SpooledClient`] per spool path,
//! mirroring the daemon's single-writer rule. All read/append/rewrite file ops
//! are serialized by an in-process [`tokio::sync::Mutex`]. There is no
//! cross-process lock in v1.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{AppendResult, Client, ClientError};
use kindling_types::{Id, ObservationInput};

/// Configuration for a [`SpooledClient`].
///
/// Carries just the spool file path today; kept as a struct so additional
/// knobs (rotation, size caps) can be added without breaking callers.
#[derive(Debug, Clone)]
pub struct SpoolConfig {
    /// Path to the append-only NDJSON spool file. Created on first spool.
    pub spool_path: PathBuf,
}

impl SpoolConfig {
    /// Build a config from a spool path.
    pub fn new(spool_path: impl Into<PathBuf>) -> Self {
        Self {
            spool_path: spool_path.into(),
        }
    }
}

/// A single buffered observation request, one per NDJSON line.
///
/// Field names are camelCase to match the wire shapes of the wrapped
/// `append_observation` arguments, so a spool file doubles as a debuggable /
/// importable record.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpoolEntry {
    /// The observation input (its `id` is always populated by the time it is
    /// spooled, so replay is idempotent on id).
    pub input: ObservationInput,
    /// Optional capsule to attach the observation to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capsule_id: Option<Id>,
    /// Optional service-side validation toggle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validate: Option<bool>,
}

/// Outcome of [`SpooledClient::append_observation`].
#[derive(Debug)]
pub enum AppendOutcome {
    /// Reached the daemon; carries the [`AppendResult`] (stored observation +
    /// the daemon's `deduplicated` marker). `deduplicated` is `true` when this
    /// id was already stored (e.g. a replay of an entry the daemon had already
    /// committed before a crash) — the stored row is returned unchanged.
    ///
    /// The result is boxed so the enum stays small (the `Spooled` variant
    /// carries no data).
    Delivered(Box<AppendResult>),
    /// Daemon unreachable — the request was buffered to the spool.
    Spooled,
}

/// Result of [`SpooledClient::flush`].
#[derive(Debug, PartialEq, Eq)]
pub struct FlushReport {
    /// Number of spooled entries successfully replayed into the daemon.
    pub replayed: usize,
    /// Number of entries still buffered (kept) after the flush stopped.
    pub remaining: usize,
}

/// Errors from [`SpooledClient`] operations.
///
/// Note that a daemon *outage* is **not** an error from
/// [`SpooledClient::append_observation`] — it returns [`AppendOutcome::Spooled`].
/// [`SpoolError::Client`] only carries the *propagated* client errors (the
/// daemon responded and rejected the request).
#[derive(Debug, thiserror::Error)]
pub enum SpoolError {
    /// A spool-file I/O failure (open, read, append, temp-write, rename).
    #[error("spool io error: {0}")]
    Io(#[from] std::io::Error),

    /// A spool entry could not be (de)serialized.
    #[error("spool serde error: {0}")]
    Serde(#[from] serde_json::Error),

    /// The daemon responded with a non-connectivity error that must not be
    /// spooled (`Api`, `SchemaMismatch`, `Decode`, `Io`).
    #[error("client error: {0}")]
    Client(#[from] ClientError),
}

/// A durable-emit wrapper around a [`crate::Client`].
///
/// Holds the client, the spool path, and an in-process [`Mutex`] that serializes
/// every spool-file operation (single-producer-per-spool-file).
#[derive(Debug)]
pub struct SpooledClient {
    client: Client,
    spool_path: PathBuf,
    /// Serializes read/append/rewrite of the spool file. The mutex guards the
    /// *file*, not the client (the client is internally `Send + Sync`).
    file_lock: Mutex<()>,
}

impl SpooledClient {
    /// Build a durable-emit client over `client`, buffering to `spool_path`.
    pub fn new(client: Client, spool_path: PathBuf) -> Self {
        Self {
            client,
            spool_path,
            file_lock: Mutex::new(()),
        }
    }

    /// Build from a [`SpoolConfig`].
    pub fn with_config(client: Client, config: SpoolConfig) -> Self {
        Self::new(client, config.spool_path)
    }

    /// Borrow the underlying client for reads / non-spooled ops (retrieve,
    /// health, pin, capsules, …). Only `append_observation` is durability-wrapped.
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Append an observation durably.
    ///
    /// Assigns a stable v4 id when `input.id` is `None` (so a spooled entry and
    /// any replay share one id — see the crate-level *Delivery semantics*).
    /// Then:
    ///
    /// 1. If the spool already has buffered entries, opportunistically
    ///    [`flush`](Self::flush) them **first** so the daemon observes them in
    ///    append order ahead of this new one.
    /// 2. Try the daemon. On success → [`AppendOutcome::Delivered`].
    /// 3. On a *connectivity* failure (`Unavailable` / `Http`) → buffer to the
    ///    spool and return [`AppendOutcome::Spooled`] (never an error).
    /// 4. On any other client error → propagate as [`SpoolError::Client`]; the
    ///    observation is **not** spooled.
    pub async fn append_observation(
        &self,
        mut input: ObservationInput,
        capsule_id: Option<Id>,
        validate: Option<bool>,
    ) -> Result<AppendOutcome, SpoolError> {
        // Stable id BEFORE any try/spool, so replay is idempotent on id.
        if input.id.is_none() {
            input.id = Some(Uuid::new_v4().to_string());
        }

        // Opportunistic drain: if there is a backlog, try to clear it first so
        // ordering is preserved (backlog lands before this new observation).
        // If the drain can't reach the daemon, this new entry will spool behind
        // it below, still in order.
        if self.pending_count()? > 0 {
            // Best-effort: a connectivity failure here is fine — we proceed and
            // (most likely) spool the new entry behind the backlog. A
            // *propagating* client error from a backlog entry must surface.
            self.flush().await?;
        }

        match self
            .client
            .append_observation(input.clone(), capsule_id.clone(), validate)
            .await
        {
            Ok(result) => Ok(AppendOutcome::Delivered(Box::new(result))),
            Err(err) if is_connectivity_error(&err) => {
                let entry = SpoolEntry {
                    input,
                    capsule_id,
                    validate,
                };
                self.append_to_spool(&entry).await?;
                Ok(AppendOutcome::Spooled)
            }
            Err(err) => Err(SpoolError::Client(err)),
        }
    }

    /// Drain the spool into the daemon, in order.
    ///
    /// Replays each buffered entry via the client. Stops at the first
    /// *connectivity* failure (`Unavailable` / `Http`), keeping that entry and
    /// the remainder. A non-connectivity client error also stops the drain and
    /// is propagated, but the *un-replayed remainder (including the rejected
    /// entry) is preserved* — data is never silently dropped.
    ///
    /// The spool file is rewritten with exactly the un-replayed remainder via a
    /// temp-file-then-rename, so a crash mid-flush cannot corrupt the spool.
    pub async fn flush(&self) -> Result<FlushReport, SpoolError> {
        let _guard = self.file_lock.lock().await;

        let entries = read_spool(&self.spool_path)?;
        let total = entries.len();
        if total == 0 {
            return Ok(FlushReport {
                replayed: 0,
                remaining: 0,
            });
        }

        let mut replayed = 0usize;
        let mut propagate: Option<ClientError> = None;

        for (idx, entry) in entries.iter().enumerate() {
            match self
                .client
                .append_observation(
                    entry.input.clone(),
                    entry.capsule_id.clone(),
                    entry.validate,
                )
                .await
            {
                Ok(_) => replayed += 1,
                Err(err) if is_connectivity_error(&err) => break,
                Err(err) => {
                    // Non-connectivity rejection: stop, keep this entry + the
                    // remainder, and propagate after rewriting the spool. We do
                    // NOT advance `replayed` for this entry, so it stays buffered.
                    let _ = idx;
                    propagate = Some(err);
                    break;
                }
            }
        }

        let remainder = &entries[replayed..];
        rewrite_spool(&self.spool_path, remainder)?;

        if let Some(err) = propagate {
            return Err(SpoolError::Client(err));
        }

        Ok(FlushReport {
            replayed,
            remaining: remainder.len(),
        })
    }

    /// Count of pending (un-replayed) spool entries.
    pub fn pending_count(&self) -> Result<usize, SpoolError> {
        Ok(read_spool(&self.spool_path)?.len())
    }

    /// Append one entry to the spool file (create + append), serialized by the
    /// file lock.
    async fn append_to_spool(&self, entry: &SpoolEntry) -> Result<(), SpoolError> {
        use std::io::Write;

        let _guard = self.file_lock.lock().await;
        let line = serde_json::to_string(entry)?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.spool_path)?;
        file.write_all(line.as_bytes())?;
        file.write_all(b"\n")?;
        file.flush()?;
        Ok(())
    }
}

/// Connectivity failures buffer to the spool; everything else propagates.
fn is_connectivity_error(err: &ClientError) -> bool {
    matches!(err, ClientError::Unavailable(_) | ClientError::Http(_))
}

/// Read all parseable spool entries in order.
///
/// A missing file is an empty spool. A torn trailing line (crash mid-write) is
/// tolerated: only the *last* line is allowed to fail to parse, in which case it
/// is skipped and the preceding good entries are returned. A malformed line that
/// is *not* the last is a corruption we surface as an error (it would otherwise
/// silently drop a buffered observation).
fn read_spool(path: &Path) -> Result<Vec<SpoolEntry>, SpoolError> {
    let contents = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(SpoolError::Io(e)),
    };

    // Split on '\n'. Trailing newline yields a final empty segment we ignore.
    let lines: Vec<&str> = contents.split('\n').collect();
    let mut entries = Vec::new();
    let last_idx = lines.len().saturating_sub(1);

    for (idx, raw) in lines.iter().enumerate() {
        let line = raw.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<SpoolEntry>(line) {
            Ok(entry) => entries.push(entry),
            Err(e) => {
                // Tolerate a torn trailing line only.
                if idx == last_idx {
                    break;
                }
                return Err(SpoolError::Serde(e));
            }
        }
    }
    Ok(entries)
}

/// Rewrite the spool file with exactly `entries`, atomically via temp + rename.
///
/// Writing an empty remainder truncates the spool to empty (file removed if it
/// exists, leaving a clean state).
fn rewrite_spool(path: &Path, entries: &[SpoolEntry]) -> Result<(), SpoolError> {
    use std::io::Write;

    if entries.is_empty() {
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(SpoolError::Io(e)),
        }
    } else {
        let tmp = temp_sibling(path);
        {
            let mut file = std::fs::File::create(&tmp)?;
            for entry in entries {
                let line = serde_json::to_string(entry)?;
                file.write_all(line.as_bytes())?;
                file.write_all(b"\n")?;
            }
            file.flush()?;
        }
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

/// A temp sibling path next to `path` (same directory, so `rename` is atomic on
/// the same filesystem).
fn temp_sibling(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(format!(".tmp-{}", Uuid::new_v4()));
    match path.parent() {
        Some(dir) => dir.join(name),
        None => PathBuf::from(name),
    }
}
