//! Rust client for the kindling daemon.
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
//! GET    /v1/health                  → 200 { version, schemaVersion, supportedKinds, storagePath, kindRegistry, projects: [...] }
//! POST   /v1/capsules                → 201 Capsule
//! GET    /v1/capsules/open?sessionId → 200 Capsule | null
//! PATCH  /v1/capsules/:id/close      → 200 Capsule
//! POST   /v1/observations            → 201 Observation
//! POST   /v1/observations/:id/forget  → 204 (redact an observation)
//! POST   /v1/retrieve                → 200 RetrieveResult
//! POST   /v1/pins                    → 201 Pin
//! DELETE /v1/pins/:id                → 204
//! POST   /v1/context/session-start   → 200 { additionalContext: string | null }
//! POST   /v1/context/pre-compact     → 200 { additionalContext: string | null }
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
//! // Everything you need is re-exported here — no need to depend on
//! // `kindling-types` directly.
//! use kindling_client::{Client, CapsuleType, ScopeIds};
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
#[cfg(feature = "spool")]
pub mod spool;
mod transport;

pub use body::{CloseCapsuleBody, CreatePinBody};
pub use config::{
    default_port_path, default_socket_path, ClientConfig, Spawner, Transport,
    EXPECTED_SCHEMA_VERSION,
};
pub use error::ClientError;

use hyper::StatusCode;
use serde::de::DeserializeOwned;
use serde::Deserialize;

/// Domain types re-exported from [`kindling_types`] so the daemon client is a
/// self-contained SDK: depend on `kindling-client` alone and reach every type
/// the API sends or returns as `kindling_client::<Type>`. `kindling-types`
/// stays an internal transitive dependency you never have to name.
pub use kindling_types::{
    build_capability, kind_registry, supported_kind_names, CandidateResult, Capability, Capsule,
    CapsuleStatus, CapsuleType, Id, KindRegistryEntry, Observation, ObservationInput,
    ObservationKind, Pin, PinResult, PinTargetType, ProviderSearchOptions, ProviderSearchResult,
    RetrieveOptions, RetrieveProvenance, RetrieveResult, RetrievedEntity, ScopeIds, Summary,
    Timestamp,
};

use body::{
    AppendObservationBody, AppendObservationResponseBody, OpenCapsuleBody, PreCompactContextBody,
    SessionStartContextBody,
};

/// Outcome of [`Client::append_observation`].
///
/// Carries the stored observation plus whether the daemon deduplicated the
/// write. `deduplicated` is `true` when an observation with the same id already
/// existed: `observation` is then the pre-existing stored row (the daemon did
/// not overwrite it or re-run masking), making spool replay exactly-once-ish on
/// id. When `false`, a new row was written and `observation` is it.
#[derive(Debug, Clone, PartialEq)]
pub struct AppendResult {
    /// The stored observation. On a dedup hit this is the pre-existing row.
    pub observation: Observation,
    /// `true` when the id already existed and the daemon ignored the write.
    pub deduplicated: bool,
}

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
    /// Snake-case observation kinds the daemon supports.
    pub supported_kinds: Vec<String>,
    /// Daemon kindling home root (global storage path).
    pub storage_path: String,
    /// Machine-readable kind registry (kinds + required fields).
    pub kind_registry: Vec<KindRegistryEntry>,
    /// Project ids the daemon has touched this session.
    pub projects: Vec<String>,
}

/// Raw `/v1/health` JSON shape.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HealthBody {
    version: String,
    schema_version: u32,
    supported_kinds: Vec<String>,
    storage_path: String,
    kind_registry: Vec<KindRegistryEntry>,
    #[serde(default)]
    projects: Vec<String>,
}

/// Daemon error body shape: `{ "error": "<msg>" }`.
#[derive(Debug, Deserialize)]
struct ErrorBody {
    error: String,
}

/// `/v1/context/*` response shape: `{ "additionalContext": string | null }`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContextBody {
    #[serde(default)]
    additional_context: Option<String>,
}

/// A thin async client for the kindling daemon.
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
            supported_kinds: body.supported_kinds,
            storage_path: body.storage_path,
            kind_registry: body.kind_registry,
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

    /// `GET /v1/capsules/open?sessionId=…` — the open session capsule for
    /// `session_id`, or `None` when none is open.
    ///
    /// Each Claude Code hook is a fresh process holding only the session id, so
    /// the Stop hook resolves the capsule it must close through this endpoint
    /// rather than tracking it in-process.
    pub async fn get_open_capsule(&self, session_id: &str) -> Result<Option<Capsule>, ClientError> {
        let path = format!(
            "/v1/capsules/open?sessionId={}",
            percent_encode_query(session_id)
        );
        self.call("GET", &path, true, None::<&()>, &[StatusCode::OK])
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
    ///
    /// Returns an [`AppendResult`] carrying the stored observation and the
    /// daemon's `deduplicated` flag. A duplicate id is **not** an error: the
    /// daemon ignores the write and returns the pre-existing row with
    /// `deduplicated: true`, so replaying an already-delivered observation is a
    /// no-op.
    pub async fn append_observation(
        &self,
        input: ObservationInput,
        capsule_id: Option<Id>,
        validate: Option<bool>,
    ) -> Result<AppendResult, ClientError> {
        let body = AppendObservationBody {
            input,
            capsule_id,
            validate,
        };
        let resp: AppendObservationResponseBody = self
            .call(
                "POST",
                "/v1/observations",
                true,
                Some(&body),
                &[StatusCode::CREATED],
            )
            .await?;
        Ok(AppendResult {
            observation: resp.observation,
            deduplicated: resp.deduplicated,
        })
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

    /// `POST /v1/observations/:id/forget` — redact an observation (content
    /// replaced with `[redacted]`, `redacted` flag set). Succeeds on `204`.
    ///
    /// A missing id surfaces as [`ClientError::Api`] with status `404` (the
    /// daemon maps the store's `ObservationNotFound`). The `observation_id` must
    /// be exact — prefix resolution is a higher-layer concern.
    pub async fn forget(&self, observation_id: &str) -> Result<(), ClientError> {
        let path = format!("/v1/observations/{}/forget", observation_id);
        self.call_no_content("POST", &path, true, &[StatusCode::NO_CONTENT])
            .await
    }

    /// `POST /v1/context/session-start` — the assembled + formatted SessionStart
    /// injection markdown, or `None` when there is nothing to inject.
    ///
    /// The daemon owns the formatting (recency-ordered observations, pin
    /// previews, and the `toLocaleString`-parity dates). `max_results` caps the
    /// recent-observation count (default 10 when `None`). The project scope is
    /// derived from this client's project root, reproducing the Node hook's
    /// `{ repoId: <project root> }` filter within the project database.
    pub async fn session_start_context(
        &self,
        max_results: Option<u32>,
    ) -> Result<Option<String>, ClientError> {
        let body = SessionStartContextBody {
            max_results,
            scope_ids: self.project_scope(),
        };
        let resp: ContextBody = self
            .call(
                "POST",
                "/v1/context/session-start",
                true,
                Some(&body),
                &[StatusCode::OK],
            )
            .await?;
        Ok(resp.additional_context)
    }

    /// `POST /v1/context/pre-compact` — the assembled + formatted PreCompact
    /// injection markdown (pinned items + latest session summary), or `None`
    /// when there is nothing to inject.
    ///
    /// As with [`Self::session_start_context`], the project scope is derived
    /// from this client's project root.
    pub async fn pre_compact_context(&self) -> Result<Option<String>, ClientError> {
        let body = PreCompactContextBody {
            scope_ids: self.project_scope(),
        };
        let resp: ContextBody = self
            .call(
                "POST",
                "/v1/context/pre-compact",
                true,
                Some(&body),
                &[StatusCode::OK],
            )
            .await?;
        Ok(resp.additional_context)
    }

    /// A repo scope built from this client's project root, mirroring the Node
    /// hook's `{ repoId: getProjectRoot(cwd) }`.
    fn project_scope(&self) -> ScopeIds {
        ScopeIds {
            repo_id: Some(self.config.project_root.clone()),
            ..Default::default()
        }
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

/// Percent-encode a query-parameter value, escaping everything outside the
/// RFC 3986 unreserved set (`A-Z a-z 0-9 - . _ ~`). Keeps the
/// [`get_open_capsule`](Client::get_open_capsule) URL well-formed for arbitrary
/// session ids; UUIDs pass through unchanged.
fn percent_encode_query(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char);
            }
            other => {
                out.push('%');
                out.push(
                    char::from_digit((other >> 4) as u32, 16)
                        .unwrap()
                        .to_ascii_uppercase(),
                );
                out.push(
                    char::from_digit((other & 0xf) as u32, 16)
                        .unwrap()
                        .to_ascii_uppercase(),
                );
            }
        }
    }
    out
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
