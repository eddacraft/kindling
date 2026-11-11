//! Shared application state: the per-project service registry and the
//! activity tracker that drives idle shutdown.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use kindling_service::KindlingService;
use kindling_store::{project_db_path, project_id};

use crate::error::ApiError;

/// One [`KindlingService`] per project, keyed by the project-id hash.
///
/// `rusqlite::Connection` is `Send + !Sync`, so each service lives behind its
/// own `Mutex`. The per-project mutex serialises writes — the intended WAL
/// single-writer model. The outer mutex only guards the map, never DB work.
type Registry = Arc<Mutex<HashMap<String, Arc<Mutex<KindlingService>>>>>;

/// Shared, cloneable application state handed to every handler.
#[derive(Clone)]
pub struct AppState {
    kindling_home: PathBuf,
    services: Registry,
    activity: Arc<Activity>,
}

impl AppState {
    /// Build state rooted at `kindling_home`. Project DBs live under
    /// `kindling_home/projects/<hash>/kindling.db`.
    pub fn new(kindling_home: PathBuf) -> Self {
        Self {
            kindling_home,
            services: Arc::new(Mutex::new(HashMap::new())),
            activity: Arc::new(Activity::new()),
        }
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

    /// Resolve (and lazily open) the service for `project_root`. The DB path is
    /// derived from the store's hashing — the single source of truth — so the
    /// daemon and any other consumer of the same project share one database.
    ///
    /// Returns a clone of the per-project `Arc<Mutex<KindlingService>>`; the
    /// caller locks it, does synchronous DB work, and drops the lock. Never
    /// hold this lock across an `.await`.
    pub fn service_for(&self, project_root: &str) -> Result<Arc<Mutex<KindlingService>>, ApiError> {
        let key = project_id(project_root);

        // Fast path: already open.
        {
            let map = self.services.lock().expect("registry mutex poisoned");
            if let Some(svc) = map.get(&key) {
                return Ok(Arc::clone(svc));
            }
        }

        // Open outside the registry lock would risk a double-open race; instead
        // open under the lock. DB open is fast and rare (once per project).
        let mut map = self.services.lock().expect("registry mutex poisoned");
        if let Some(svc) = map.get(&key) {
            return Ok(Arc::clone(svc));
        }
        let db_path = project_db_path(&self.kindling_home, project_root);
        let service = KindlingService::open(&db_path)
            .map_err(|e| ApiError::Internal(format!("opening project db: {e}")))?;
        let entry = Arc::new(Mutex::new(service));
        map.insert(key, Arc::clone(&entry));
        Ok(entry)
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
