//! Daemon error types and HTTP error mapping.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use kindling_service::ServiceError;
use serde_json::json;

/// Top-level daemon error.
///
/// Returned by [`serve`](crate::serve) for lifecycle failures (binding the
/// socket, PID acquisition, IO). Per-request failures use
/// [`ApiError`] instead so they can map to HTTP status codes.
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    /// Another live daemon already holds the PID lock.
    #[error("a kindling daemon is already running (pid {0})")]
    AlreadyRunning(i32),

    /// Failed to read/parse/write the PID file.
    #[error("pid file error: {0}")]
    Pid(String),

    /// Socket bind / IO failure.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// A service/store failure surfaced during startup.
    #[error(transparent)]
    Service(#[from] ServiceError),
}

/// Per-request error mapped to an HTTP status + JSON body `{ "error": "…" }`.
#[derive(Debug)]
pub enum ApiError {
    /// 400 — malformed request, missing project header, or validation failure.
    BadRequest(String),
    /// 404 — referenced entity does not exist.
    NotFound(String),
    /// 409 — lifecycle conflict (duplicate open / already closed).
    Conflict(String),
    /// 500 — store or other internal failure.
    Internal(String),
}

impl ApiError {
    fn parts(&self) -> (StatusCode, &str) {
        match self {
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            ApiError::NotFound(m) => (StatusCode::NOT_FOUND, m),
            ApiError::Conflict(m) => (StatusCode::CONFLICT, m),
            ApiError::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
        }
    }
}

impl From<ServiceError> for ApiError {
    fn from(err: ServiceError) -> Self {
        match err {
            ServiceError::Validation(_) => ApiError::BadRequest(err.to_string()),
            ServiceError::NotFound(_) => ApiError::NotFound(err.to_string()),
            ServiceError::Conflict(_) | ServiceError::AlreadyClosed(_) => {
                ApiError::Conflict(err.to_string())
            }
            ServiceError::Store(_) | ServiceError::Provider(_) => {
                ApiError::Internal(err.to_string())
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = self.parts();
        (status, Json(json!({ "error": message }))).into_response()
    }
}
