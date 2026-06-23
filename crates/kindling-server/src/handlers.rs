//! Axum route handlers for the v1 HTTP API.
//!
//! Per-project routing: every data endpoint requires the `X-Kindling-Project`
//! request header. Its value is the **project root string**; the server derives
//! the database via [`kindling_store::project_db_path`] and caches one
//! [`KindlingService`](kindling_service::KindlingService) per project. A
//! missing/empty header on a data endpoint yields `400`.
//!
//! All DB work is synchronous. Each handler clones the per-project
//! `Arc<Mutex<KindlingService>>`, locks it, runs the synchronous service call,
//! and drops the lock before returning. No lock is ever held across an
//! `.await`.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use kindling_service::AppendObservationOptions;
use kindling_types::{build_capability, Capsule, Pin, RetrieveOptions, RetrieveResult};
use serde_json::{json, Value};

use crate::dto::{
    AppendObservationRequest, AppendObservationResponse, CloseCapsuleRequest, CreatePinRequest,
    OpenCapsuleQuery, OpenCapsuleRequest, PreCompactContextRequest, SessionStartContextRequest,
    DEFAULT_MAX_RESULTS,
};
use crate::error::ApiError;
use crate::inject::{format_pre_compact, format_session_start, local_offset_seconds};
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
    let schema = kindling_store::schema_version();
    let capability = build_capability(
        env!("CARGO_PKG_VERSION"),
        schema.version as u32,
        state.kindling_home().display().to_string(),
    );
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
    let svc = state.service_for(&root)?;
    let capsule = {
        let guard = svc.lock().expect("service mutex poisoned");
        guard.open_capsule(req.into())?
    };
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
    let svc = state.service_for(&root)?;
    let capsule = {
        let guard = svc.lock().expect("service mutex poisoned");
        guard.close_capsule(&id, req.into())?
    };
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
    let svc = state.service_for(&root)?;
    let capsule = {
        let guard = svc.lock().expect("service mutex poisoned");
        guard.get_open_capsule(&session_id)?
    };
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
    let svc = state.service_for(&root)?;
    let outcome = {
        let guard = svc.lock().expect("service mutex poisoned");
        guard.append_observation(req.input, options)?
    };
    Ok((StatusCode::CREATED, Json(outcome.into())))
}

/// `POST /v1/retrieve` — ranked retrieval.
pub async fn retrieve(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(options): Json<RetrieveOptions>,
) -> Result<Json<RetrieveResult>, ApiError> {
    let root = project_root(&headers)?;
    let svc = state.service_for(&root)?;
    let result = {
        let guard = svc.lock().expect("service mutex poisoned");
        guard.retrieve(options)?
    };
    Ok(Json(result))
}

/// `POST /v1/pins` — create a pin.
pub async fn create_pin(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreatePinRequest>,
) -> Result<(StatusCode, Json<Pin>), ApiError> {
    let root = project_root(&headers)?;
    let svc = state.service_for(&root)?;
    let pin = {
        let guard = svc.lock().expect("service mutex poisoned");
        guard.pin(req.into())?
    };
    Ok((StatusCode::CREATED, Json(pin)))
}

/// `DELETE /v1/pins/:id` — remove a pin.
pub async fn unpin(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let root = project_root(&headers)?;
    let svc = state.service_for(&root)?;
    {
        let guard = svc.lock().expect("service mutex poisoned");
        guard.unpin(&id)?;
    }
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
    let svc = state.service_for(&root)?;
    let result = {
        let guard = svc.lock().expect("service mutex poisoned");
        guard.forget(&id)
    };
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
    let svc = state.service_for(&root)?;
    let ctx = {
        let guard = svc.lock().expect("service mutex poisoned");
        guard.session_start_context_at(&req.scope_ids, max_results, now)?
    };
    // Resolve the local offset once, at the request instant, so every timestamp
    // in this batch renders in a single consistent zone.
    let offset = local_offset_seconds(now);
    let additional = format_session_start(&ctx, offset);
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
    let svc = state.service_for(&root)?;
    let ctx = {
        let guard = svc.lock().expect("service mutex poisoned");
        guard.pre_compact_context_at(&req.scope_ids, now)?
    };
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
