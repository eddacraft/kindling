//! Shared application state: the per-project service registry and the
//! activity tracker that drives idle shutdown.
//!
//! # Concurrency model (PERF-002)
//!
//! Each project has one **writer** [`KindlingService`] behind a mutex — the
//! WAL single-writer model — and a small pool of **read-only** services opened
//! on the same database file. Read endpoints run on a pooled reader inside one
//! read transaction, so a slow search holds a WAL snapshot instead of the write
//! lock, and captures keep committing while it runs. All database work runs on
//! tokio's blocking pool ([`tokio::task::spawn_blocking`]), never on an async
//! runtime thread, so one slow query cannot stall unrelated requests either.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use kindling_service::{KindlingService, ServiceError};
use kindling_store::{project_db_path, project_id, StoreError, StoreOptions};

use crate::error::ApiError;

/// Most read-only connections kept per project. Reads beyond this wait for a
/// free connection (on the blocking pool, not an async thread). Bounded so a
/// burst of searches cannot open an unbounded number of SQLite connections,
/// each with its own page cache.
const MAX_READERS_PER_PROJECT: usize = 4;

/// Per-project handles: the single writer and the read-only pool.
struct Project {
    /// The only read-write connection. `rusqlite::Connection` is
    /// `Send + !Sync`, so it lives behind a mutex, which also serialises writes.
    writer: Arc<Mutex<KindlingService>>,
    readers: ReaderPool,
}

/// One [`Project`] per project, keyed by the project-id hash. The registry
/// mutex only guards the map (and the one-time open), never per-request DB
/// work.
type Registry = Arc<Mutex<HashMap<String, Arc<Project>>>>;

#[cfg(test)]
type ReadHook = Arc<dyn Fn() + Send + Sync>;

/// Shared, cloneable application state handed to every handler.
#[derive(Clone)]
pub struct AppState {
    kindling_home: PathBuf,
    services: Registry,
    activity: Arc<Activity>,
    /// Test hook run inside a read, after its snapshot is taken.
    #[cfg(test)]
    read_hook: Option<ReadHook>,
}

impl AppState {
    /// Build state rooted at `kindling_home`. Project DBs live under
    /// `kindling_home/projects/<hash>/kindling.db`.
    pub fn new(kindling_home: PathBuf) -> Self {
        Self {
            kindling_home,
            services: Arc::new(Mutex::new(HashMap::new())),
            activity: Arc::new(Activity::new()),
            #[cfg(test)]
            read_hook: None,
        }
    }

    /// Install a hook that every read runs while holding its read snapshot.
    #[cfg(test)]
    pub(crate) fn with_read_hook(mut self, hook: ReadHook) -> Self {
        self.read_hook = Some(hook);
        self
    }

    /// kindling home root.
    pub fn kindling_home(&self) -> &PathBuf {
        &self.kindling_home
    }

    /// Activity tracker (for the idle-shutdown layer/task).
    pub fn activity(&self) -> &Arc<Activity> {
        &self.activity
    }

    /// Project ids for every project that has been touched this session (used
    /// by `/v1/health`). Sorted for deterministic output.
    pub fn known_project_ids(&self) -> Vec<String> {
        let map = self.services.lock().expect("registry mutex poisoned");
        let mut ids: Vec<String> = map.keys().cloned().collect();
        ids.sort();
        ids
    }

    /// Resolve (and lazily open) the **writer** service for `project_root`.
    /// The DB path is derived from the store's hashing — the single source of
    /// truth — so the daemon and any other consumer of the same project share
    /// one database.
    ///
    /// Returns a clone of the per-project `Arc<Mutex<KindlingService>>`; the
    /// caller locks it, does synchronous DB work, and drops the lock. Never
    /// hold this lock across an `.await`. The HTTP handlers use [`Self::read`]
    /// and [`Self::write`] instead, which also move the work off async threads.
    pub fn service_for(&self, project_root: &str) -> Result<Arc<Mutex<KindlingService>>, ApiError> {
        Ok(Arc::clone(&self.project_for(project_root)?.writer))
    }

    /// Resolve (and lazily open) the project. The writer is opened first, so a
    /// fresh database is created and migrated before any read-only connection
    /// is attempted (a read-only open of an uninitialised file fails).
    fn project_for(&self, project_root: &str) -> Result<Arc<Project>, ApiError> {
        let key = project_id(project_root);

        // Fast path: already open.
        {
            let map = self.services.lock().expect("registry mutex poisoned");
            if let Some(project) = map.get(&key) {
                return Ok(Arc::clone(project));
            }
        }

        // Open outside the registry lock would risk a double-open race; instead
        // open under the lock. DB open is fast and rare (once per project).
        let mut map = self.services.lock().expect("registry mutex poisoned");
        if let Some(project) = map.get(&key) {
            return Ok(Arc::clone(project));
        }
        let db_path = project_db_path(&self.kindling_home, project_root);
        let writer = KindlingService::open(&db_path)
            .map_err(|e| ApiError::Internal(format!("opening project db: {e}")))?;
        let project = Arc::new(Project {
            writer: Arc::new(Mutex::new(writer)),
            readers: ReaderPool::new(db_path, MAX_READERS_PER_PROJECT),
        });
        map.insert(key, Arc::clone(&project));
        Ok(project)
    }

    /// Run `op` on the project's writer, serialised with every other write,
    /// on the blocking pool.
    pub(crate) async fn write<T, F>(&self, project_root: String, op: F) -> Result<T, ApiError>
    where
        T: Send + 'static,
        F: FnOnce(&KindlingService) -> Result<T, ServiceError> + Send + 'static,
    {
        let state = self.clone();
        run_blocking(self, move || {
            let project = state.project_for(&project_root)?;
            let writer = project.writer.lock().expect("service mutex poisoned");
            Ok(op(&writer)?)
        })
        .await
    }

    /// Run a read-only `op` on a pooled read-only connection, inside one read
    /// transaction (a single consistent snapshot), on the blocking pool.
    ///
    /// `op` must not write. It sees every write committed before the snapshot
    /// is taken. If a read-only connection cannot be opened, the read falls
    /// back to the writer, as before this pool existed.
    pub(crate) async fn read<T, F>(&self, project_root: String, op: F) -> Result<T, ApiError>
    where
        T: Send + 'static,
        F: FnOnce(&KindlingService) -> Result<T, ServiceError> + Send + 'static,
    {
        let state = self.clone();
        run_blocking(self, move || {
            let project = state.project_for(&project_root)?;
            let result = match project.readers.acquire() {
                Ok(reader) => state.read_in_snapshot(&reader, op),
                Err(_) => {
                    let writer = project.writer.lock().expect("service mutex poisoned");
                    state.read_in_snapshot(&writer, op)
                }
            };
            result
        })
        .await
    }

    fn read_in_snapshot<T, F>(&self, service: &KindlingService, op: F) -> Result<T, ApiError>
    where
        F: FnOnce(&KindlingService) -> Result<T, ServiceError>,
    {
        let conn = service.store().connection();
        // Rolled back on drop: a read transaction has nothing to commit.
        let _snapshot = conn
            .unchecked_transaction()
            .map_err(|e| ServiceError::from(StoreError::from(e)))?;
        #[cfg(test)]
        if let Some(hook) = &self.read_hook {
            // Take the WAL read snapshot now, then let the test hold it open.
            conn.query_row("SELECT count(*) FROM observations", [], |_| Ok(()))
                .map_err(|e| ServiceError::from(StoreError::from(e)))?;
            hook();
        }
        Ok(op(service)?)
    }
}

/// Run blocking DB work on tokio's blocking pool. The request stays counted
/// as in flight until the work finishes, even if the handler future is
/// dropped first (client disconnect), so idle shutdown never races a running
/// write. A panic in the work is re-raised on the handler task, as it was when
/// handlers ran the work inline.
async fn run_blocking<T, F>(state: &AppState, work: F) -> Result<T, ApiError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, ApiError> + Send + 'static,
{
    let in_flight = RequestGuard::new(Arc::clone(&state.activity));
    let task = tokio::task::spawn_blocking(move || {
        let _in_flight = in_flight;
        work()
    });
    match task.await {
        Ok(result) => result,
        Err(err) if err.is_panic() => std::panic::resume_unwind(err.into_panic()),
        Err(err) => Err(ApiError::Internal(format!("database task failed: {err}"))),
    }
}

/// A bounded pool of read-only [`KindlingService`]s over one database file.
/// Connections are opened lazily and kept for reuse.
struct ReaderPool {
    db_path: PathBuf,
    max: usize,
    state: Mutex<PoolState>,
    available: Condvar,
}

struct PoolState {
    idle: Vec<KindlingService>,
    /// Connections that exist (idle or leased).
    open: usize,
}

impl ReaderPool {
    fn new(db_path: PathBuf, max: usize) -> Self {
        Self {
            db_path,
            max,
            state: Mutex::new(PoolState {
                idle: Vec::new(),
                open: 0,
            }),
            available: Condvar::new(),
        }
    }

    /// Take an idle reader, open a new one under the cap, or wait for one to
    /// be returned. Blocks; call only from the blocking pool.
    fn acquire(&self) -> Result<ReaderLease<'_>, ServiceError> {
        let mut state = self.state.lock().expect("reader pool mutex poisoned");
        loop {
            if let Some(service) = state.idle.pop() {
                return Ok(ReaderLease {
                    pool: self,
                    service: Some(service),
                });
            }
            if state.open < self.max {
                state.open += 1;
                drop(state);
                let opened = KindlingService::open_with_options(
                    &self.db_path,
                    &StoreOptions { readonly: true },
                );
                return match opened {
                    Ok(service) => Ok(ReaderLease {
                        pool: self,
                        service: Some(service),
                    }),
                    Err(err) => {
                        let mut state = self.state.lock().expect("reader pool mutex poisoned");
                        state.open -= 1;
                        self.available.notify_one();
                        Err(err)
                    }
                };
            }
            state = self
                .available
                .wait(state)
                .expect("reader pool mutex poisoned");
        }
    }
}

/// A leased reader; returns to the pool on drop.
struct ReaderLease<'a> {
    pool: &'a ReaderPool,
    service: Option<KindlingService>,
}

impl std::ops::Deref for ReaderLease<'_> {
    type Target = KindlingService;

    fn deref(&self) -> &KindlingService {
        self.service
            .as_ref()
            .expect("lease holds a service until drop")
    }
}

impl Drop for ReaderLease<'_> {
    fn drop(&mut self) {
        if let Some(service) = self.service.take() {
            // Recover a poisoned lock: returning a connection must not panic
            // during unwinding.
            let mut state = self
                .pool
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.idle.push(service);
            self.pool.available.notify_one();
        }
    }
}

/// Tracks in-flight request count and last-activity time for idle shutdown.
#[derive(Debug)]
pub struct Activity {
    in_flight: AtomicU64,
    /// Millis since `start` of the last request boundary (start or finish).
    last_active_ms: AtomicU64,
    start: Instant,
}

/// Cancellation-safe request scope: [`Activity::enter`] on construct,
/// [`Activity::leave`] on drop — including when the handler future is
/// cancelled rather than run to completion.
#[must_use = "the guard leaves on drop; not binding it marks the request finished immediately"]
pub(crate) struct RequestGuard {
    activity: Arc<Activity>,
}

impl RequestGuard {
    pub(crate) fn new(activity: Arc<Activity>) -> Self {
        activity.enter();
        Self { activity }
    }
}

impl Drop for RequestGuard {
    fn drop(&mut self) {
        self.activity.leave();
    }
}

impl Activity {
    fn new() -> Self {
        Self {
            in_flight: AtomicU64::new(0),
            last_active_ms: AtomicU64::new(0),
            start: Instant::now(),
        }
    }

    fn now_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    /// Mark the start of a request: bump in-flight and touch last-active.
    pub fn enter(&self) {
        self.in_flight.fetch_add(1, Ordering::SeqCst);
        self.last_active_ms.store(self.now_ms(), Ordering::SeqCst);
    }

    /// Mark the end of a request: touch last-active, then drop in-flight.
    ///
    /// Last-activity is published before the count drops so a concurrent
    /// [`is_idle_for`] cannot observe zero in-flight requests against the
    /// timestamp from [`enter`]. A long request would otherwise look idle the
    /// moment it finished.
    pub fn leave(&self) {
        self.publish_leave(None);
    }

    /// Shared leave transition. `interleaved` runs after last-activity is
    /// stored and before in-flight drops, so tests can observe that window.
    fn publish_leave(&self, interleaved: Option<&dyn Fn(&Self)>) {
        self.last_active_ms.store(self.now_ms(), Ordering::SeqCst);
        if let Some(observe) = interleaved {
            observe(self);
        }
        self.in_flight.fetch_sub(1, Ordering::SeqCst);
    }

    /// Whether the daemon has been idle (no in-flight, no recent activity) for
    /// at least `timeout`.
    pub fn is_idle_for(&self, timeout: Duration) -> bool {
        if self.in_flight.load(Ordering::SeqCst) > 0 {
            return false;
        }
        let idle_ms = self
            .now_ms()
            .saturating_sub(self.last_active_ms.load(Ordering::SeqCst));
        idle_ms >= timeout.as_millis() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_after_timeout_when_no_activity() {
        let a = Activity::new();
        // Brand new: last_active = 0, but elapsed grows. With a zero timeout we
        // are immediately "idle".
        assert!(a.is_idle_for(Duration::from_millis(0)));
    }

    #[test]
    fn not_idle_while_in_flight() {
        let a = Activity::new();
        a.enter();
        assert!(!a.is_idle_for(Duration::from_millis(0)));
        a.leave();
        assert!(a.is_idle_for(Duration::from_millis(0)));
    }

    #[test]
    fn leave_does_not_report_idle_between_touch_and_decrement() {
        let a = Activity::new();
        a.enter();
        // Age the enter() timestamp past the idle threshold so a decrement
        // that ran first would look idle against a stale clock.
        std::thread::sleep(Duration::from_millis(40));
        a.publish_leave(Some(&|a| {
            assert!(
                !a.is_idle_for(Duration::from_millis(20)),
                "last-activity must be published before in-flight drops"
            );
        }));
        assert!(
            !a.is_idle_for(Duration::from_millis(20)),
            "a just-finished request must not look idle"
        );
        assert!(a.is_idle_for(Duration::from_millis(0)));
    }

    #[test]
    fn decrementing_before_touch_can_look_idle_against_a_stale_timestamp() {
        let a = Activity::new();
        a.enter();
        std::thread::sleep(Duration::from_millis(40));
        // The unsafe order: drop in-flight first, leaving last-activity at
        // enter(). This is the window `publish_leave` must not expose.
        a.in_flight.fetch_sub(1, Ordering::SeqCst);
        assert!(
            a.is_idle_for(Duration::from_millis(20)),
            "stale last-activity plus zero in-flight is the leave() race"
        );
    }

    #[tokio::test]
    async fn cancelled_request_guard_releases_activity() {
        let activity = Arc::new(Activity::new());
        let entered = Arc::new(tokio::sync::Notify::new());
        let guard_activity = Arc::clone(&activity);
        let entered_signal = Arc::clone(&entered);
        let handle = tokio::spawn(async move {
            let _request = RequestGuard::new(guard_activity);
            entered_signal.notify_one();
            std::future::pending::<()>().await
        });
        entered.notified().await;
        assert!(
            !activity.is_idle_for(Duration::from_millis(0)),
            "an in-flight request must keep the tracker busy"
        );
        handle.abort();
        let _ = handle.await;
        assert!(
            activity.is_idle_for(Duration::from_millis(0)),
            "cancelling the request future must still call leave"
        );
    }

    #[test]
    fn request_guard_leave_on_drop() {
        let activity = Arc::new(Activity::new());
        {
            let _request = RequestGuard::new(Arc::clone(&activity));
            assert!(!activity.is_idle_for(Duration::from_millis(0)));
        }
        assert!(activity.is_idle_for(Duration::from_millis(0)));
    }
}
