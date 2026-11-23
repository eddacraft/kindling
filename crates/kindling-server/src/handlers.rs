//! Axum route handlers for the v1 HTTP API.
//!
//! Per-project routing: every data endpoint requires the `X-Kindling-Project`
//! request header. Its value is the **project root string**; the server derives
//! the database via [`kindling_store::project_db_path`] and caches one
//! [`KindlingService`](kindling_service::KindlingService) per project. A
//! missing/empty header on a data endpoint yields `400`.
//!
//! All DB work is synchronous and runs on tokio's blocking pool via
//! [`AppState::write`] (the project's single writer, serialised) or
//! [`AppState::read`] (a pooled read-only connection, inside one read
//! snapshot). Only endpoints that never write use `read`. No lock is ever held
//! across an `.await`.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use kindling_service::AppendObservationOptions;
use kindling_types::{
    build_capability, Capsule, ListObservationsRequest, ListObservationsResult, Pin,
    RetrieveOptions, RetrieveResult,
};
use serde_json::{json, Value};

use crate::dto::{
    AppendObservationRequest, AppendObservationResponse, CloseCapsuleRequest, CreatePinRequest,
    OpenCapsuleQuery, OpenCapsuleRequest, PreCompactContextRequest, SessionStartContextRequest,
    DEFAULT_MAX_RESULTS,
};
use crate::error::ApiError;
use crate::inject::{format_pre_compact, format_session_start_with, local_offset_seconds};
use crate::state::AppState;

/// Header carrying the project root string for per-project DB routing.
pub const PROJECT_HEADER: &str = "x-kindling-project";

/// Header carrying the session id for `GET /v1/capsules/open`. Accepted as an
/// alternative to the `?sessionId=` query param so a hook can resolve a
/// session's open capsule without a request body (each hook is a fresh
/// process).
pub const SESSION_HEADER: &str = "x-kindling-session";

/// Pull the project root from `X-Kindling-Project`, erroring 400 if missing.
fn project_root(headers: &HeaderMap) -> Result<String, ApiError> {
    let value = headers
        .get(PROJECT_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty());
    match value {
        Some(root) => Ok(root.to_string()),
        None => Err(ApiError::BadRequest(format!(
            "missing or empty {PROJECT_HEADER} header"
        ))),
    }
}

/// `GET /v1/health` — capability handshake plus touched project ids.
/// Requires no project header.
pub async fn health(State(state): State<AppState>) -> Json<Value> {
    let capability = build_capability(
        env!("CARGO_PKG_VERSION"),
        kindling_store::wire_version(),
        state.kindling_home().display().to_string(),
    )
    .with_schema_revision(kindling_store::schema_revision());
    let mut body = serde_json::to_value(capability).expect("capability serializes");
    if let Some(obj) = body.as_object_mut() {
        obj.insert("projects".to_string(), json!(state.known_project_ids()));
    }
    Json(body)
}

/// `POST /v1/capsules` — open a capsule.
pub async fn open_capsule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<OpenCapsuleRequest>,
) -> Result<(StatusCode, Json<Capsule>), ApiError> {
    let root = project_root(&headers)?;
    let capsule = state
        .write(root, move |svc| svc.open_capsule(req.into()))
        .await?;
    Ok((StatusCode::CREATED, Json(capsule)))
}

/// `PATCH /v1/capsules/:id/close` — close a capsule.
pub async fn close_capsule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Option<Json<CloseCapsuleRequest>>,
) -> Result<Json<Capsule>, ApiError> {
    let root = project_root(&headers)?;
    let req = body.map(|Json(r)| r).unwrap_or_default();
    let capsule = state
        .write(root, move |svc| svc.close_capsule(&id, req.into()))
        .await?;
    Ok(Json(capsule))
}

/// `GET /v1/capsules/open` — the open session capsule for a session id, or
/// JSON `null` when none is open.
///
/// The session id comes from the `?sessionId=` query param or, equivalently,
/// the `X-Kindling-Session` header (the query param wins if both are present).
/// A missing/empty session id yields `400`. The Stop hook uses this to resolve
/// the capsule it must close, since each hook is a fresh process holding only
/// the session id.
pub async fn get_open_capsule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<OpenCapsuleQuery>,
) -> Result<Json<Option<Capsule>>, ApiError> {
    let root = project_root(&headers)?;
    let session_id = query
        .session_id
        .or_else(|| {
            headers
                .get(SESSION_HEADER)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string)
        })
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            ApiError::BadRequest(format!(
                "missing or empty sessionId (query param or {SESSION_HEADER} header)"
            ))
        })?;
    let capsule = state
        .read(root, move |svc| svc.get_open_capsule(&session_id))
        .await?;
    Ok(Json(capsule))
}

/// `POST /v1/observations` — append an observation.
///
/// The response flattens the stored observation and adds a top-level
/// `deduplicated` flag (see [`AppendObservationResponse`]). A duplicate id is
/// not an error: the daemon ignores the incoming write and returns the
/// pre-existing stored row with `deduplicated: true`, still as `201 Created`.
pub async fn append_observation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<AppendObservationRequest>,
) -> Result<(StatusCode, Json<AppendObservationResponse>), ApiError> {
    let root = project_root(&headers)?;
    let options = AppendObservationOptions {
        capsule_id: req.capsule_id,
        validate: req.validate.unwrap_or(true),
    };
    let outcome = state
        .write(root, move |svc| svc.append_observation(req.input, options))
        .await?;
    Ok((StatusCode::CREATED, Json(outcome.into())))
}

/// `POST /v1/observations/list` — exhaustive, paginated observation list
/// (kind / scope / half-open time-range filters; keyset cursor). FTS-independent.
pub async fn list_observations(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ListObservationsRequest>,
) -> Result<Json<ListObservationsResult>, ApiError> {
    let root = project_root(&headers)?;
    let result = state
        .read(root, move |svc| svc.list_observations(request))
        .await?;
    Ok(Json(result))
}

/// `POST /v1/retrieve` — ranked retrieval.
pub async fn retrieve(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(options): Json<RetrieveOptions>,
) -> Result<Json<RetrieveResult>, ApiError> {
    let root = project_root(&headers)?;
    let result = state.read(root, move |svc| svc.retrieve(options)).await?;
    Ok(Json(result))
}

/// `POST /v1/pins` — create a pin.
pub async fn create_pin(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreatePinRequest>,
) -> Result<(StatusCode, Json<Pin>), ApiError> {
    let root = project_root(&headers)?;
    let pin = state.write(root, move |svc| svc.pin(req.into())).await?;
    Ok((StatusCode::CREATED, Json(pin)))
}

/// `DELETE /v1/pins/:id` — remove a pin.
pub async fn unpin(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let root = project_root(&headers)?;
    state.write(root, move |svc| svc.unpin(&id)).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /v1/observations/:id/forget` — redact an observation (content replaced
/// with `[redacted]`, `redacted` flag set), returning `204 No Content`.
///
/// `service.forget` delegates to the store's `redact_observation`, which errors
/// [`StoreError::ObservationNotFound`](kindling_store::StoreError::ObservationNotFound)
/// when no row matches the id. We map that single case to `404`; any other store
/// failure stays a `500`.
///
/// Note: redaction is NOT idempotent at the store layer. The `observations_fts`
/// update trigger issues an FTS5 `'delete'` keyed on the *old* content, so a
/// second forget on an already-redacted row tries to delete the `[redacted]`
/// placeholder (which was never indexed) and surfaces an FTS error → `500`.
/// Prefix-resolution / dedup of already-redacted ids is the caller's concern;
/// this endpoint faithfully forwards whatever the store does.
pub async fn forget_observation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    use kindling_service::ServiceError;
    use kindling_store::StoreError;

    let root = project_root(&headers)?;
    // The closure returns the service result as a value (never an error) so
    // the not-found case can be mapped to 404 here.
    let forget_id = id.clone();
    let result = state
        .write(root, move |svc| Ok(svc.forget(&forget_id)))
        .await?;
    match result {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        // A missing observation is a 404, not an internal error.
        Err(ServiceError::Store(StoreError::ObservationNotFound(_))) => {
            Err(ApiError::NotFound(format!("observation {id} not found")))
        }
        Err(err) => Err(err.into()),
    }
}

/// `POST /v1/context/session-start` — assemble + format the SessionStart
/// injection. Returns `{ "additionalContext": string | null }` (null when there
/// is nothing to inject). An empty/absent body is accepted.
pub async fn session_start_context(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Option<Json<SessionStartContextRequest>>,
) -> Result<Json<Value>, ApiError> {
    let root = project_root(&headers)?;
    let req = body.map(|Json(r)| r).unwrap_or_default();
    let max_results = req.max_results.unwrap_or(DEFAULT_MAX_RESULTS);
    let now = now_ms();
    let ctx = state
        .read(root, move |svc| {
            svc.session_start_context_at(&req.scope_ids, max_results, now)
        })
        .await?;
    // Resolve the local offset at each observation's instant so every timestamp
    // renders in the DST offset that was in effect then, not at request time.
    let additional = format_session_start_with(&ctx, local_offset_seconds);
    Ok(Json(json!({ "additionalContext": additional })))
}

/// `POST /v1/context/pre-compact` — assemble + format the PreCompact injection.
/// Returns `{ "additionalContext": string | null }`. An empty/absent body is
/// accepted.
pub async fn pre_compact_context(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Option<Json<PreCompactContextRequest>>,
) -> Result<Json<Value>, ApiError> {
    let root = project_root(&headers)?;
    let req = body.map(|Json(r)| r).unwrap_or_default();
    let now = now_ms();
    let ctx = state
        .read(root, move |svc| {
            svc.pre_compact_context_at(&req.scope_ids, now)
        })
        .await?;
    let additional = format_pre_compact(&ctx);
    Ok(Json(json!({ "additionalContext": additional })))
}

/// Current time in epoch milliseconds.
fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{mpsc, Arc, Mutex};
    use std::time::Duration;

    use axum::http::HeaderValue;

    use super::*;

    const ROOT: &str = "/tmp/perf002-project";
    /// Generous bound: only a write blocked behind the held read would hit it.
    const BOUND: Duration = Duration::from_secs(10);

    fn headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(PROJECT_HEADER, HeaderValue::from_static(ROOT));
        headers
    }

    fn append_body(content: &str) -> Json<AppendObservationRequest> {
        Json(
            serde_json::from_value(json!({
                "kind": "message",
                "content": content,
                "scopeIds": { "repoId": "perf002" }
            }))
            .unwrap(),
        )
    }

    fn retrieve_body(query: &str) -> Json<RetrieveOptions> {
        Json(
            serde_json::from_value(json!({
                "query": query,
                "scopeIds": { "repoId": "perf002" }
            }))
            .unwrap(),
        )
    }

    /// A read that is holding its snapshot open must not block a write to the
    /// same project, even on a current-thread runtime (what `kindling serve`
    /// runs). The held read keeps its pre-write snapshot; the next read sees
    /// the committed write.
    #[tokio::test]
    async fn write_completes_while_a_read_holds_its_snapshot() {
        let home = tempfile::tempdir().unwrap();
        let armed = Arc::new(AtomicBool::new(false));
        let (entered_tx, mut entered_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let release_rx = Mutex::new(release_rx);
        let hook_armed = Arc::clone(&armed);
        let state = AppState::new(home.path().to_path_buf()).with_read_hook(Arc::new(move || {
            // Only the first read after arming is held.
            if hook_armed.swap(false, Ordering::SeqCst) {
                let _ = entered_tx.send(());
                let _ = release_rx.lock().unwrap().recv_timeout(BOUND * 3);
            }
        }));

        // Create the database (writer first) with one searchable row.
        let (status, _) =
            append_observation(State(state.clone()), headers(), append_body("alpha first"))
                .await
                .unwrap();
        assert_eq!(status, StatusCode::CREATED);

        // Start a read and hold it inside its snapshot.
        armed.store(true, Ordering::SeqCst);
        let held = tokio::spawn(retrieve(
            State(state.clone()),
            headers(),
            retrieve_body("beta"),
        ));
        tokio::time::timeout(BOUND, entered_rx.recv())
            .await
            .expect("read never reached its snapshot")
            .unwrap();

        // The write must complete while the read is still held.
        let (status, Json(written)) = tokio::time::timeout(
            BOUND,
            append_observation(State(state.clone()), headers(), append_body("beta second")),
        )
        .await
        .expect("write blocked behind an in-progress read")
        .unwrap();
        assert_eq!(status, StatusCode::CREATED);
        assert!(!held.is_finished(), "the read must still be held open");

        // Release: the held read answers from its pre-write snapshot.
        release_tx.send(()).unwrap();
        let Json(before) = tokio::time::timeout(BOUND, held)
            .await
            .expect("held read never finished")
            .unwrap()
            .unwrap();
        assert!(
            before.candidates.is_empty(),
            "a read keeps the snapshot it started with"
        );

        // A new read sees the committed write.
        let Json(after) = retrieve(State(state.clone()), headers(), retrieve_body("beta"))
            .await
            .unwrap();
        let ids: Vec<_> = after
            .candidates
            .iter()
            .filter_map(|c| match &c.entity {
                kindling_types::RetrievedEntity::Observation(o) => Some(o.id.clone()),
                kindling_types::RetrievedEntity::Summary(_) => None,
            })
            .collect();
        let written = serde_json::to_value(&written).unwrap();
        assert_eq!(ids, vec![written["id"].as_str().unwrap().to_string()]);
    }

    /// Reads against a project nobody has written to yet still work: the writer
    /// creates and migrates the fresh database before a read-only connection
    /// opens it.
    #[tokio::test]
    async fn first_request_to_a_fresh_project_can_be_a_read() {
        let home = tempfile::tempdir().unwrap();
        let state = AppState::new(home.path().to_path_buf());
        let Json(result) = retrieve(State(state.clone()), headers(), retrieve_body("anything"))
            .await
            .unwrap();
        assert!(result.candidates.is_empty());
        let Json(open) = get_open_capsule(
            State(state),
            headers(),
            Query(serde_json::from_value(json!({ "sessionId": "s1" })).unwrap()),
        )
        .await
        .unwrap();
        assert!(open.is_none());
    }
}
