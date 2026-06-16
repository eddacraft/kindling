//! Rust client for the Kindling daemon.
//!
//! A thin, async HTTP/1-over-Unix-domain-socket client for the
//! [`kindling-server`](../kindling_server/index.html) daemon. It speaks the v1
//! wire contract exactly, sending the `X-Kindling-Project` header on every data
//! endpoint, and auto-spawns `kindling serve --daemonize` on first call if the
//! daemon is not running.
//!
//! The method surface mirrors `kindling-service` for ergonomic
//! in-process / via-daemon interchangeability. To stay thin, the crate depends
//! only on [`kindling_types`] for domain shapes — never on `kindling-service`
//! or `kindling-store` (which pull `rusqlite`).
//!
//! # v1 wire contract
//!
//! ```text
//! GET    /v1/health             → 200 { version, schemaVersion, projects: [...] }
//! POST   /v1/capsules           → 201 Capsule
//! PATCH  /v1/capsules/:id/close → 200 Capsule
//! POST   /v1/observations       → 201 Observation
//! POST   /v1/retrieve           → 200 RetrieveResult
//! POST   /v1/pins               → 201 Pin
//! DELETE /v1/pins/:id           → 204
//! ```
//!
//! # Schema version
//!
//! [`Client::health`] checks the daemon's reported `schemaVersion` against
//! [`ClientConfig::expected_schema_version`] (default
//! [`EXPECTED_SCHEMA_VERSION`], sourced at compile time from the repo-root
//! `schema/version.json`) and returns [`ClientError::SchemaMismatch`] on
//! disagreement. See [`Spawner`] and the `config` notes for the `cargo publish`
//! copy-step caveat.
//!
//! # Example
//!
//! ```no_run
//! # async fn run() -> Result<(), kindling_client::ClientError> {
//! use kindling_client::Client;
//! use kindling_types::{CapsuleType, ScopeIds};
//!
//! let client = Client::new()?;
//! let health = client.health().await?;
//! println!("daemon schema v{}", health.schema_version);
//!
//! let capsule = client
//!     .open_capsule(CapsuleType::Session, "investigate bug", ScopeIds::default(), None)
//!     .await?;
//! # let _ = capsule;
//! # Ok(())
//! # }
//! ```

mod body;
mod config;
mod error;
mod transport;

pub use body::{CloseCapsuleBody, CreatePinBody};
pub use config::{default_socket_path, ClientConfig, Spawner, EXPECTED_SCHEMA_VERSION};
pub use error::ClientError;

use hyper::StatusCode;
use serde::de::DeserializeOwned;
use serde::Deserialize;

use kindling_types::{
    Capsule, CapsuleType, Id, Observation, ObservationInput, Pin, RetrieveOptions, RetrieveResult,
    ScopeIds,
};

use body::{AppendObservationBody, OpenCapsuleBody};

/// Header carrying the project root string for per-project DB routing. Mirrors
/// `kindling_server::PROJECT_HEADER`.
pub const PROJECT_HEADER: &str = "x-kindling-project";

/// Result of `GET /v1/health`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Health {
    /// Daemon package version (`CARGO_PKG_VERSION` of `kindling-server`).
    pub version: String,
    /// Schema version the daemon's store reports.
    pub schema_version: u32,
    /// Project ids the daemon has touched this session.
    pub projects: Vec<String>,
}

/// Raw `/v1/health` JSON shape.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HealthBody {
    version: String,
    schema_version: u32,
    #[serde(default)]
    projects: Vec<String>,
}

/// Daemon error body shape: `{ "error": "<msg>" }`.
#[derive(Debug, Deserialize)]
struct ErrorBody {
    error: String,
}

/// A thin async client for the Kindling daemon.
///
/// Cheap to clone-by-config; construct once and share a reference. Each call
/// opens a fresh connection (no pooling), so the client itself holds no live
/// socket and is `Send + Sync`.
#[derive(Debug, Clone)]
pub struct Client {
    config: ClientConfig,
}

impl Client {
    /// Build a client from the default [`ClientConfig`]: socket at
    /// `~/.kindling/kindling.sock`, project root from the current directory,
    /// the compiled schema version, a 1s connect budget, and the real binary
    /// spawner.
    pub fn new() -> Result<Self, ClientError> {
        Ok(Self {
            config: ClientConfig::defaults()?,
        })
    }

    /// Build a client from an explicit [`ClientConfig`].
    pub fn with_config(config: ClientConfig) -> Self {
        Self { config }
    }

    /// The configuration this client was built with.
    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    /// `GET /v1/health` — version, schema version, and touched project ids.
    ///
    /// Verifies the daemon's `schemaVersion` matches
    /// [`ClientConfig::expected_schema_version`]; returns
    /// [`ClientError::SchemaMismatch`] if not (fail loud).
    pub async fn health(&self) -> Result<Health, ClientError> {
        let body: HealthBody = self
            .call(
                "GET",
                "/v1/health",
                /* project */ false,
                None::<&()>,
                &[StatusCode::OK],
            )
            .await?;

        let expected = self.config.expected_schema_version;
        if body.schema_version != expected {
            return Err(ClientError::SchemaMismatch {
                expected,
                actual: body.schema_version,
            });
        }
        Ok(Health {
            version: body.version,
            schema_version: body.schema_version,
            projects: body.projects,
        })
    }

    /// `POST /v1/capsules` — open a capsule.
    pub async fn open_capsule(
        &self,
        kind: CapsuleType,
        intent: impl Into<String>,
        scope_ids: ScopeIds,
        id: Option<Id>,
    ) -> Result<Capsule, ClientError> {
        let body = OpenCapsuleBody {
            kind,
            intent: intent.into(),
            scope_ids,
            id,
        };
        self.call(
            "POST",
            "/v1/capsules",
            true,
            Some(&body),
            &[StatusCode::CREATED],
        )
        .await
    }

    /// `PATCH /v1/capsules/:id/close` — close a capsule.
    pub async fn close_capsule(
        &self,
        capsule_id: &str,
        body: CloseCapsuleBody,
    ) -> Result<Capsule, ClientError> {
        let path = format!("/v1/capsules/{}/close", capsule_id);
        self.call("PATCH", &path, true, Some(&body), &[StatusCode::OK])
            .await
    }

    /// `POST /v1/observations` — append an observation, optionally attaching it
    /// to `capsule_id` and toggling service-side `validate` (default true).
    pub async fn append_observation(
        &self,
        input: ObservationInput,
        capsule_id: Option<Id>,
        validate: Option<bool>,
    ) -> Result<Observation, ClientError> {
        let body = AppendObservationBody {
            input,
            capsule_id,
            validate,
        };
        self.call(
            "POST",
            "/v1/observations",
            true,
            Some(&body),
            &[StatusCode::CREATED],
        )
        .await
    }

    /// `POST /v1/retrieve` — deterministic ranked retrieval.
    pub async fn retrieve(&self, options: RetrieveOptions) -> Result<RetrieveResult, ClientError> {
        self.call(
            "POST",
            "/v1/retrieve",
            true,
            Some(&options),
            &[StatusCode::OK],
        )
        .await
    }

    /// `POST /v1/pins` — create a pin.
    pub async fn pin(&self, body: CreatePinBody) -> Result<Pin, ClientError> {
        self.call(
            "POST",
            "/v1/pins",
            true,
            Some(&body),
            &[StatusCode::CREATED],
        )
        .await
    }

    /// `DELETE /v1/pins/:id` — remove a pin.
    pub async fn unpin(&self, pin_id: &str) -> Result<(), ClientError> {
        let path = format!("/v1/pins/{}", pin_id);
        self.call_no_content("DELETE", &path, true, &[StatusCode::NO_CONTENT])
            .await
    }

    // ---- internal request plumbing ------------------------------------------

    /// Send a request and decode a 2xx JSON body into `T`.
    async fn call<B, T>(
        &self,
        method: &str,
        path: &str,
        project: bool,
        body: Option<&B>,
        expected: &[StatusCode],
    ) -> Result<T, ClientError>
    where
        B: serde::Serialize,
        T: DeserializeOwned,
    {
        let raw = self.send(method, path, project, body).await?;
        ensure_status(&raw, expected)?;
        serde_json::from_slice(&raw.body)
            .map_err(|e| ClientError::Decode(format!("{e}: body was {:?}", raw.body)))
    }

    /// Send a request that returns no body on success.
    async fn call_no_content(
        &self,
        method: &str,
        path: &str,
        project: bool,
        expected: &[StatusCode],
    ) -> Result<(), ClientError> {
        let raw = self.send(method, path, project, None::<&()>).await?;
        ensure_status(&raw, expected)?;
        Ok(())
    }

    /// Serialize the body and dispatch through the transport.
    async fn send<B>(
        &self,
        method: &str,
        path: &str,
        project: bool,
        body: Option<&B>,
    ) -> Result<transport::RawResponse, ClientError>
    where
        B: serde::Serialize,
    {
        let body_str = match body {
            Some(b) => serde_json::to_string(b)
                .map_err(|e| ClientError::Decode(format!("serializing request body: {e}")))?,
            None => String::new(),
        };
        let project_header = if project {
            Some(self.config.project_root.as_str())
        } else {
            None
        };
        transport::request(
            &self.config,
            transport::OutgoingRequest {
                method,
                path,
                project: project_header,
                body: body_str,
            },
        )
        .await
    }
}

/// Map a response to an error if its status is not in `expected`, extracting the
/// daemon's `{ "error": "<msg>" }` message when present.
fn ensure_status(raw: &transport::RawResponse, expected: &[StatusCode]) -> Result<(), ClientError> {
    if expected.contains(&raw.status) {
        return Ok(());
    }
    let message = serde_json::from_slice::<ErrorBody>(&raw.body)
        .map(|b| b.error)
        .unwrap_or_else(|_| {
            if raw.body.is_empty() {
                raw.status
                    .canonical_reason()
                    .unwrap_or("unknown error")
                    .to_string()
            } else {
                String::from_utf8_lossy(&raw.body).into_owned()
            }
        });
    Err(ClientError::Api {
        status: raw.status.as_u16(),
        message,
    })
}
