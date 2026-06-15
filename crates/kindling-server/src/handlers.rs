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

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use kindling_service::AppendObservationOptions;
use kindling_types::{Capsule, Observation, Pin, RetrieveOptions, RetrieveResult};
use serde_json::{json, Value};

use crate::dto::{
    AppendObservationRequest, CloseCapsuleRequest, CreatePinRequest, OpenCapsuleRequest,
};
use crate::error::ApiError;
use crate::state::AppState;

/// Header carrying the project root string for per-project DB routing.
pub const PROJECT_HEADER: &str = "x-kindling-project";

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

/// `GET /v1/health` — version, schema version, and touched project ids.
/// Requires no project header.
pub async fn health(State(state): State<AppState>) -> Json<Value> {
    let schema = kindling_store::schema_version();
    Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "schemaVersion": schema.version,
        "projects": state.known_project_ids(),
    }))
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

/// `POST /v1/observations` — append an observation.
pub async fn append_observation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<AppendObservationRequest>,
) -> Result<(StatusCode, Json<Observation>), ApiError> {
    let root = project_root(&headers)?;
    let options = AppendObservationOptions {
        capsule_id: req.capsule_id,
        validate: req.validate.unwrap_or(true),
    };
    let svc = state.service_for(&root)?;
    let observation = {
        let guard = svc.lock().expect("service mutex poisoned");
        guard.append_observation(req.input, options)?
    };
    Ok((StatusCode::CREATED, Json(observation)))
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
