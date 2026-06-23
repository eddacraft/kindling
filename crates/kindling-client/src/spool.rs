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
//! v1 is **at-least-once**. Before trying or spooling, `append_observation`
//! assigns a stable [`uuid::Uuid`] v4 to the observation when the caller left
//! `input.id` as `None`, so a spooled entry and any later replay carry the
//! *same* id. A crash after the daemon commits but before the spool is rewritten
//! can therefore replay an already-stored observation. Making this
//! **exactly-once** requires the daemon to ignore (dedup) a write whose id
//! already exists — a noted follow-up, not yet implemented.
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
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{Client, ClientError};
use kindling_types::{Id, Observation, ObservationInput};

/// Live + on-disk spool observability snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpoolStatus {
    /// Count of buffered NDJSON entries not yet replayed into the daemon.
    pub pending_count: usize,
    /// Path to the NDJSON spool file this status describes.
    pub spool_path: PathBuf,
    /// Epoch milliseconds of the last successful flush.
    #[serde(default)]
    pub last_flush_time_ms: Option<i64>,
    /// Last connectivity or flush error observed for this spool file.
    #[serde(default)]
    pub last_error: Option<String>,
    /// Cumulative replay attempts (each flush try per spooled entry).
    pub replay_attempts: u64,
}

/// Flush/error/replay counters for a spool file (in-memory + sidecar).
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SpoolRuntime {
    last_flush_time_ms: Option<i64>,
    last_error: Option<String>,
    replay_attempts: u64,
}

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
    /// Written straight to the daemon; carries the stored [`Observation`].
    ///
    /// The `Observation` is boxed so the enum stays small (the `Spooled`
    /// variant carries no data).
    Delivered(Box<Observation>),
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
    /// Live flush/error/replay counters for this client instance.
    runtime: Mutex<SpoolRuntime>,
}

impl SpooledClient {
    /// Build a durable-emit client over `client`, buffering to `spool_path`.
    pub fn new(client: Client, spool_path: PathBuf) -> Self {
        let runtime = load_runtime_sidecar(&spool_path);
        Self {
            client,
            spool_path,
            file_lock: Mutex::new(()),
            runtime: Mutex::new(runtime),
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
            Ok(observation) => Ok(AppendOutcome::Delivered(Box::new(observation))),
            Err(err) if is_connectivity_error(&err) => {
                self.record_connectivity_error(&err).await;
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
            self.bump_replay_attempts().await;
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
                Err(err) if is_connectivity_error(&err) => {
                    self.record_connectivity_error(&err).await;
                    break;
                }
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

        if replayed > 0 {
            self.record_successful_flush().await;
        }

        Ok(FlushReport {
            replayed,
            remaining: remainder.len(),
        })
    }

    async fn bump_replay_attempts(&self) {
        let mut rt = self.runtime.lock().await;
        rt.replay_attempts = rt.replay_attempts.saturating_add(1);
        persist_runtime_sidecar(&self.spool_path, &rt);
    }

    async fn record_connectivity_error(&self, err: &ClientError) {
        let mut rt = self.runtime.lock().await;
        rt.last_error = Some(err.to_string());
        persist_runtime_sidecar(&self.spool_path, &rt);
    }

    async fn record_successful_flush(&self) {
        let mut rt = self.runtime.lock().await;
        rt.last_flush_time_ms = Some(now_ms());
        rt.last_error = None;
        persist_runtime_sidecar(&self.spool_path, &rt);
    }

    /// Count of pending (un-replayed) spool entries.
    pub fn pending_count(&self) -> Result<usize, SpoolError> {
        Ok(read_spool(&self.spool_path)?.len())
    }

    /// Observability snapshot for this client (pending count + live counters).
    pub async fn spool_status(&self) -> Result<SpoolStatus, SpoolError> {
        let runtime = self.runtime.lock().await;
        Ok(SpoolStatus {
            pending_count: self.pending_count()?,
            spool_path: self.spool_path.clone(),
            last_flush_time_ms: runtime.last_flush_time_ms,
            last_error: runtime.last_error.clone(),
            replay_attempts: runtime.replay_attempts,
        })
    }

    /// Passive status from an on-disk spool file and its optional `.status.json`
    /// sidecar (best-effort: a corrupt sidecar is ignored).
    pub fn spool_status_from_path(
        spool_path: impl Into<PathBuf>,
    ) -> Result<SpoolStatus, SpoolError> {
        let spool_path = spool_path.into();
        let pending_count = read_spool(&spool_path)?.len();
        let runtime = load_runtime_sidecar(&spool_path);
        Ok(SpoolStatus {
            pending_count,
            spool_path,
            last_flush_time_ms: runtime.last_flush_time_ms,
            last_error: runtime.last_error,
            replay_attempts: runtime.replay_attempts,
        })
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

/// Sidecar path for flush/error/replay metadata: `{spool_path}.status.json`.
fn status_sidecar_path(spool_path: &Path) -> PathBuf {
    let name = format!(
        "{}.status.json",
        spool_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "spool".to_string())
    );
    match spool_path.parent() {
        Some(dir) => dir.join(name),
        None => PathBuf::from(name),
    }
}

/// Load sidecar metadata. Best-effort: any I/O or parse failure yields defaults.
fn load_runtime_sidecar(spool_path: &Path) -> SpoolRuntime {
    let path = status_sidecar_path(spool_path);
    let contents = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return SpoolRuntime::default(),
    };
    serde_json::from_str(&contents).unwrap_or_default()
}

/// Persist sidecar metadata. Best-effort: errors are ignored so observability
/// never masks spool delivery or flush progress.
fn persist_runtime_sidecar(spool_path: &Path, runtime: &SpoolRuntime) {
    let _ = (|| -> Result<(), SpoolError> {
        let path = status_sidecar_path(spool_path);
        let line = serde_json::to_string(runtime)?;
        let tmp = temp_sibling(&path);
        std::fs::write(&tmp, format!("{line}\n"))?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    })();
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
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
