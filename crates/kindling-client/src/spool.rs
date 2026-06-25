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
//!
//! # Retention
//!
//! By default the spool is **unbounded**. A caller that wants a bounded buffer
//! (e.g. one replacing a rolling size/age-capped sidecar) opts into a cap via
//! [`SpoolConfig::with_max_bytes`] / [`SpoolConfig::with_max_age_ms`]. When a cap
//! is set, [`SpooledClient::flush`] trims the **oldest** entries from the
//! retained remainder — age first, then bytes — under the same file lock as the
//! rewrite, so trimming never races a drain. Because only a contiguous oldest
//! prefix is dropped, drain ordering is preserved and an un-drained entry is
//! never dropped while a strictly newer one is kept ahead of it.
//!
//! Trimming is **intentional, bounded retention loss** and is a *different*
//! contract from the delivery semantics above: under an outage that outlasts the
//! cap, the oldest un-drained entries are discarded (exactly what the capped
//! sidecar it replaces did). "Respect at-least-once" here means *do not reorder
//! and do not drop a newer entry while keeping an older one* — not infinite
//! retention. Shed entries are counted in
//! [`SpoolStatus::dropped_count`](crate::SpoolStatus). The append-under-outage
//! path stays near the cap because `append_observation` opportunistically
//! `flush`es (and therefore trims) whenever a backlog already exists, so the
//! spool exceeds the cap by at most the entry appended since the last flush.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{AppendResult, Client, ClientError};
use kindling_types::{Id, ObservationInput};

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
    /// Cumulative entries dropped by retention trimming (size/age caps). Always
    /// `0` when no cap is configured.
    #[serde(default)]
    pub dropped_count: u64,
}

/// Flush/error/replay counters for a spool file (in-memory + sidecar).
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SpoolRuntime {
    last_flush_time_ms: Option<i64>,
    last_error: Option<String>,
    replay_attempts: u64,
    #[serde(default)]
    dropped_count: u64,
}

/// Configuration for a [`SpooledClient`].
///
/// `#[non_exhaustive]` so future knobs can be added without breaking callers;
/// construct via [`SpoolConfig::new`] plus the `with_*` builders rather than a
/// struct literal.
///
/// # Retention
///
/// By default the spool is **unbounded** (`max_bytes` and `max_age_ms` are
/// `None`) — existing behaviour, no surprise eviction on upgrade. Opt into a
/// rolling cap with [`with_max_bytes`](Self::with_max_bytes) /
/// [`with_max_age_ms`](Self::with_max_age_ms); see [`SpooledClient::flush`] for
/// the trim semantics. The cap *values* are the caller's policy (e.g. a
/// downstream replacing a 7-day / 64 MiB sidecar wires those numbers here);
/// kindling only provides the mechanism.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct SpoolConfig {
    /// Path to the append-only NDJSON spool file. Created on first spool.
    pub spool_path: PathBuf,
    /// Rolling byte cap; `None` = unbounded. When set, [`SpooledClient::flush`]
    /// trims the oldest entries until the retained spool fits (a lone entry
    /// larger than the cap is kept — the cap is a high-water target).
    pub max_bytes: Option<u64>,
    /// Rolling age cap in milliseconds; `None` = unbounded. Entries whose
    /// `spooled_at` is older than `now - max_age_ms` are trimmed from the front.
    pub max_age_ms: Option<i64>,
}

impl SpoolConfig {
    /// Build an unbounded config from a spool path.
    pub fn new(spool_path: impl Into<PathBuf>) -> Self {
        Self {
            spool_path: spool_path.into(),
            max_bytes: None,
            max_age_ms: None,
        }
    }

    /// Set the rolling byte cap (high-water target; oldest entries trimmed first).
    pub fn with_max_bytes(mut self, max_bytes: u64) -> Self {
        self.max_bytes = Some(max_bytes);
        self
    }

    /// Set the rolling age cap in milliseconds (oldest entries trimmed first).
    pub fn with_max_age_ms(mut self, max_age_ms: i64) -> Self {
        self.max_age_ms = Some(max_age_ms);
        self
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
    /// Epoch milliseconds when this entry was written to the spool. Used as the
    /// basis for the age retention cap — distinct from `input.ts` (the
    /// *observation* time, which can be far in the past for historical replays).
    /// Legacy entries written before this field existed deserialize to `None`
    /// and are byte-trimmable but never age-trimmed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spooled_at: Option<i64>,
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
    /// Rolling byte cap; `None` = unbounded. See [`SpoolConfig::max_bytes`].
    max_bytes: Option<u64>,
    /// Rolling age cap (ms); `None` = unbounded. See [`SpoolConfig::max_age_ms`].
    max_age_ms: Option<i64>,
    /// Serializes read/append/rewrite of the spool file. The mutex guards the
    /// *file*, not the client (the client is internally `Send + Sync`).
    file_lock: Mutex<()>,
    /// Live flush/error/replay counters for this client instance.
    runtime: Mutex<SpoolRuntime>,
}

impl SpooledClient {
    /// Build an **unbounded** durable-emit client over `client`, buffering to
    /// `spool_path`. Use [`with_config`](Self::with_config) to opt into a
    /// retention cap.
    pub fn new(client: Client, spool_path: PathBuf) -> Self {
        let runtime = load_runtime_sidecar(&spool_path);
        Self {
            client,
            spool_path,
            max_bytes: None,
            max_age_ms: None,
            file_lock: Mutex::new(()),
            runtime: Mutex::new(runtime),
        }
    }

    /// Build from a [`SpoolConfig`], carrying its retention caps.
    pub fn with_config(client: Client, config: SpoolConfig) -> Self {
        let runtime = load_runtime_sidecar(&config.spool_path);
        Self {
            client,
            spool_path: config.spool_path,
            max_bytes: config.max_bytes,
            max_age_ms: config.max_age_ms,
            file_lock: Mutex::new(()),
            runtime: Mutex::new(runtime),
        }
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
                self.record_connectivity_error(&err).await;
                let entry = SpoolEntry {
                    input,
                    capsule_id,
                    validate,
                    spooled_at: Some(now_ms()),
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
    ///
    /// If a retention cap is configured (see [`SpoolConfig`]), the retained
    /// remainder is **trimmed** (oldest first) before the rewrite — see the
    /// crate-level *Retention* docs. This is the only place trimming happens, so
    /// it can never race a concurrent drain.
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

        // Apply the retention cap to the *retained* remainder (the un-replayed
        // tail), then rewrite. Trimming only the oldest leading prefix keeps the
        // survivors a contiguous, in-order tail — drain always replays
        // front-to-back, so an entry is never dropped while a strictly newer one
        // is kept ahead of it. This is intentional, bounded retention loss under
        // a sustained outage (the oldest un-drained entries are shed once they
        // exceed the cap) — distinct from the "never silently drop on a
        // reachable daemon" guarantee above.
        let mut remainder: Vec<SpoolEntry> = entries[replayed..].to_vec();
        let dropped = if self.max_bytes.is_some() || self.max_age_ms.is_some() {
            let before = remainder.len();
            remainder = trim_entries(remainder, self.max_bytes, self.max_age_ms, now_ms());
            before - remainder.len()
        } else {
            0
        };
        rewrite_spool(&self.spool_path, &remainder)?;

        if dropped > 0 {
            self.bump_dropped(dropped as u64).await;
        }

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

    async fn bump_dropped(&self, n: u64) {
        let mut rt = self.runtime.lock().await;
        rt.dropped_count = rt.dropped_count.saturating_add(n);
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
            dropped_count: runtime.dropped_count,
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
            dropped_count: runtime.dropped_count,
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

/// Serialized on-disk size of one entry: its NDJSON line plus the newline.
/// Matches exactly what [`rewrite_spool`] writes, so the byte cap is measured
/// against real file bytes. A non-serializable entry counts as 0 (it cannot
/// occur for a valid entry that round-tripped through `read_spool`).
fn entry_size(entry: &SpoolEntry) -> u64 {
    serde_json::to_string(entry)
        .map(|s| s.len() as u64 + 1)
        .unwrap_or(0)
}

/// Apply the retention caps by dropping the **oldest leading prefix** only.
///
/// Age first, then bytes. Both drop strictly from the front, so the result is
/// always a contiguous, in-order *suffix* of the input — which is exactly what
/// preserves drain ordering and the "never drop an un-drained entry ahead of a
/// kept newer one" invariant.
///
/// - **Age:** drop a leading run whose `spooled_at` is `< now - max_age_ms`.
///   Stop at the first entry that is not age-expired *or* has no `spooled_at`
///   stamp (legacy entries are never age-trimmed).
/// - **Bytes:** if the serialized remainder still exceeds `max_bytes`, keep
///   dropping from the front until it fits — but always keep at least one entry.
///   A lone entry larger than `max_bytes` is therefore retained: the byte cap is
///   a high-water target, not a hard ceiling, and a single un-delivered record
///   is never dropped purely for size (it will still age out).
fn trim_entries(
    mut entries: Vec<SpoolEntry>,
    max_bytes: Option<u64>,
    max_age_ms: Option<i64>,
    now: i64,
) -> Vec<SpoolEntry> {
    // Age: count the leading expired prefix, then drain it in one shot.
    if let Some(max_age) = max_age_ms {
        let cutoff = now.saturating_sub(max_age);
        let mut drop_n = 0;
        for entry in &entries {
            match entry.spooled_at {
                Some(t) if t < cutoff => drop_n += 1,
                _ => break,
            }
        }
        entries.drain(0..drop_n);
    }

    // Bytes: drop oldest until the remainder fits, keeping at least one entry.
    if let Some(max_bytes) = max_bytes {
        let sizes: Vec<u64> = entries.iter().map(entry_size).collect();
        let mut total: u64 = sizes.iter().sum();
        let mut drop_n = 0;
        while total > max_bytes && entries.len() - drop_n > 1 {
            total -= sizes[drop_n];
            drop_n += 1;
        }
        entries.drain(0..drop_n);
    }

    entries
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

#[cfg(test)]
mod trim_tests {
    use super::*;
    use kindling_types::{ObservationKind, ScopeIds};

    /// A spool entry tagged with `content` (an identity marker) and an optional
    /// `spooled_at` stamp.
    fn entry(content: &str, spooled_at: Option<i64>) -> SpoolEntry {
        SpoolEntry {
            input: ObservationInput {
                id: Some(format!("id-{content}")),
                kind: ObservationKind::Message,
                content: content.to_string(),
                provenance: None,
                ts: None,
                scope_ids: ScopeIds::default(),
                redacted: None,
            },
            capsule_id: None,
            validate: None,
            spooled_at,
        }
    }

    fn contents(entries: &[SpoolEntry]) -> Vec<String> {
        entries.iter().map(|e| e.input.content.clone()).collect()
    }

    /// Survivors must always be a contiguous *suffix* of the input order.
    fn assert_is_suffix(original: &[SpoolEntry], kept: &[SpoolEntry]) {
        let orig = contents(original);
        let kept = contents(kept);
        assert!(
            orig.ends_with(&kept),
            "kept {kept:?} is not a suffix of {orig:?}"
        );
    }

    #[test]
    fn no_caps_is_a_noop() {
        let input = vec![entry("a", Some(1)), entry("b", Some(2))];
        let kept = trim_entries(input.clone(), None, None, 1_000);
        assert_eq!(contents(&kept), contents(&input));
    }

    #[test]
    fn empty_spool_is_a_noop() {
        let kept = trim_entries(Vec::new(), Some(10), Some(10), 1_000);
        assert!(kept.is_empty());
    }

    #[test]
    fn age_drops_oldest_prefix_only() {
        // now = 1000, max_age = 100 → cutoff 900; entries older than 900 expire.
        let input = vec![
            entry("a", Some(500)), // expired
            entry("b", Some(800)), // expired
            entry("c", Some(950)), // kept
            entry("d", Some(990)), // kept
        ];
        let kept = trim_entries(input.clone(), None, Some(100), 1_000);
        assert_eq!(contents(&kept), vec!["c", "d"]);
        assert_is_suffix(&input, &kept);
    }

    #[test]
    fn age_stops_at_first_unexpired_even_if_later_ones_are_old() {
        // A newer entry ahead of an older one must NOT be dropped: age trim only
        // removes a contiguous leading expired run.
        let input = vec![
            entry("a", Some(500)), // expired
            entry("b", Some(950)), // not expired → stop here
            entry("c", Some(400)), // older, but behind a kept entry → retained
        ];
        let kept = trim_entries(input.clone(), None, Some(100), 1_000);
        assert_eq!(contents(&kept), vec!["b", "c"]);
        assert_is_suffix(&input, &kept);
    }

    #[test]
    fn legacy_entry_without_stamp_is_never_age_trimmed() {
        let input = vec![
            entry("a", None),      // legacy: blocks age trim at the front
            entry("b", Some(100)), // would be expired, but is behind "a"
        ];
        let kept = trim_entries(input.clone(), None, Some(100), 1_000_000);
        assert_eq!(contents(&kept), vec!["a", "b"]);
    }

    #[test]
    fn bytes_drops_oldest_until_under_cap() {
        let input = vec![entry("a", None), entry("b", None), entry("c", None)];
        let each = entry_size(&input[0]);
        // Cap that fits exactly two entries.
        let kept = trim_entries(input.clone(), Some(each * 2), None, 0);
        assert_eq!(contents(&kept), vec!["b", "c"]);
        assert_is_suffix(&input, &kept);
        let total: u64 = kept.iter().map(entry_size).sum();
        assert!(total <= each * 2);
    }

    #[test]
    fn lone_entry_larger_than_cap_is_retained() {
        // One entry whose size exceeds the cap is kept — the cap is a high-water
        // target, never a reason to drop the only un-delivered record.
        let big = entry(&"x".repeat(4096), None);
        let cap = entry_size(&big) / 2;
        let kept = trim_entries(vec![big.clone()], Some(cap), None, 0);
        assert_eq!(contents(&kept), vec![big.input.content]);
    }

    #[test]
    fn bytes_keeps_at_least_one_even_when_all_oversize() {
        let input = vec![
            entry(&"x".repeat(1000), None),
            entry(&"y".repeat(1000), None),
        ];
        // Cap smaller than a single entry: drop down to the newest single entry.
        let kept = trim_entries(input.clone(), Some(10), None, 0);
        assert_eq!(contents(&kept), vec!["y".repeat(1000)]);
    }

    #[test]
    fn age_then_bytes_compose_into_a_suffix() {
        let input = vec![
            entry("a", Some(100)), // age-expired
            entry("b", Some(200)), // age-expired
            entry("c", Some(950)), // survives age
            entry("d", Some(960)), // survives age
            entry("e", Some(970)), // survives age
        ];
        let each = entry_size(&input[0]);
        // now=1000, max_age=100 → drop a,b by age; then a byte cap of two entries
        // drops c, leaving d,e.
        let kept = trim_entries(input.clone(), Some(each * 2), Some(100), 1_000);
        assert_eq!(contents(&kept), vec!["d", "e"]);
        assert_is_suffix(&input, &kept);
    }
}
