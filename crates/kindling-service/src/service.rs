//! `KindlingService` — in-process orchestration over the store/provider/filter
//! crates. Ports `KindlingService` from
//! `packages/kindling-core/src/service/kindling-service.ts`.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::filter::mask_secrets;
use kindling_provider::{retrieve_at, LocalFtsProvider};
use kindling_store::{SqliteKindlingStore, StoreError, StoreOptions};
use kindling_types::{
    Capsule, CapsuleInput, CapsuleStatus, CapsuleType, Id, Observation, ObservationInput, Pin,
    PinInput, PinTargetType, RetrieveOptions, RetrieveResult, ScopeIds, Summary, SummaryInput,
    Timestamp,
};

use crate::context::{PreCompactContext, ResolvedPin, SessionStartContext};
use crate::error::{ServiceError, ServiceResult};
use crate::validation;

/// Options for [`KindlingService::open_capsule`].
///
/// Mirrors `OpenCapsuleOptions` in `packages/kindling-core/src/capsule/types.ts`.
#[derive(Debug, Clone)]
pub struct OpenCapsuleOptions {
    /// Capsule type (`type` in the TS option object).
    pub kind: CapsuleType,
    /// Human-readable intent/purpose.
    pub intent: String,
    /// Scope dimensions for isolation.
    pub scope_ids: ScopeIds,
    /// Optional pre-generated id; defaults to a fresh UUID.
    pub id: Option<Id>,
}

/// Options for [`KindlingService::close_capsule`].
///
/// Mirrors `CloseCapsuleOptions` in the TS service. A summary is generated and
/// persisted only when `generate_summary` is set AND `summary_content` is
/// present.
#[derive(Debug, Clone, Default)]
pub struct CloseCapsuleOptions {
    /// Whether to generate a closing summary.
    pub generate_summary: bool,
    /// Content for the generated summary.
    pub summary_content: Option<String>,
    /// Summary confidence; defaults to `1.0` (the TS service default).
    pub confidence: Option<f64>,
}

/// Options for [`KindlingService::append_observation`].
///
/// Mirrors `AppendObservationOptions` in the TS service. `validate` defaults to
/// `true`; use [`AppendObservationOptions::default`] for the common case.
#[derive(Debug, Clone)]
pub struct AppendObservationOptions {
    /// Capsule to attach the observation to, if any.
    pub capsule_id: Option<Id>,
    /// Run validation before storing (TS default: `true`).
    pub validate: bool,
}

impl Default for AppendObservationOptions {
    fn default() -> Self {
        Self {
            capsule_id: None,
            validate: true,
        }
    }
}

/// Options for [`KindlingService::pin`].
///
/// Mirrors `CreatePinOptions` in the TS service. `note` becomes the pin's
/// `reason`; `ttl_ms` sets `expires_at = now + ttl_ms`.
#[derive(Debug, Clone)]
pub struct CreatePinOptions {
    /// What kind of entity the pin targets.
    pub target_type: PinTargetType,
    /// Id of the targeted observation or summary.
    pub target_id: Id,
    /// Optional human note (stored as the pin `reason`).
    pub note: Option<String>,
    /// Time-to-live in ms; `None` means the pin never expires.
    pub ttl_ms: Option<i64>,
    /// Scope for the pin; defaults to empty.
    pub scope_ids: Option<ScopeIds>,
}

/// In-process kindling orchestration service.
///
/// Owns the [`SqliteKindlingStore`]. The retrieval provider borrows the store
/// connection, so it is constructed per-retrieve rather than held as a field.
pub struct KindlingService {
    store: SqliteKindlingStore,
}

impl KindlingService {
    /// Build a service over an already-open store.
    pub fn new(store: SqliteKindlingStore) -> Self {
        Self { store }
    }

    /// Open (and initialise if fresh) the database at `path`.
    pub fn open(path: &Path) -> ServiceResult<Self> {
        Ok(Self::new(SqliteKindlingStore::open(path)?))
    }

    /// Open the database at `path` with explicit store options.
    pub fn open_with_options(path: &Path, options: &StoreOptions) -> ServiceResult<Self> {
        Ok(Self::new(SqliteKindlingStore::open_with_options(
            path, options,
        )?))
    }

    /// Open a fresh in-memory store (test/scratch use).
    pub fn open_in_memory() -> ServiceResult<Self> {
        Ok(Self::new(SqliteKindlingStore::open_in_memory()?))
    }

    /// Borrow the underlying store (e.g. for read-only callers).
    pub fn store(&self) -> &SqliteKindlingStore {
        &self.store
    }

    // ===== capsule lifecycle =====

    /// Open a new capsule (`status = open`). For `Session` capsules with a
    /// session scope, rejects opening when one is already open for that
    /// session. Ports `openCapsule` lifecycle.
    pub fn open_capsule(&self, options: OpenCapsuleOptions) -> ServiceResult<Capsule> {
        self.open_capsule_at(options, now_ms())
    }

    /// [`Self::open_capsule`] with an explicit clock for deterministic tests.
    pub fn open_capsule_at(
        &self,
        options: OpenCapsuleOptions,
        now: Timestamp,
    ) -> ServiceResult<Capsule> {
        // Duplicate-open guard (session-scoped only), matching the TS lifecycle.
        if options.kind == CapsuleType::Session {
            if let Some(session_id) = options.scope_ids.session_id.as_deref() {
                if let Some(existing) = self.store.get_open_capsule_for_session(session_id)? {
                    return Err(ServiceError::Conflict(format!(
                        "session {session_id} already has an open capsule ({})",
                        existing.id
                    )));
                }
            }
        }

        let capsule = validation::validate_capsule(
            CapsuleInput {
                id: options.id,
                kind: options.kind,
                intent: options.intent,
                status: Some(CapsuleStatus::Open),
                opened_at: None,
                closed_at: None,
                scope_ids: options.scope_ids,
                observation_ids: None,
                summary_id: None,
            },
            now,
        )
        .map_err(ServiceError::Validation)?;

        self.store.create_capsule(&capsule)?;
        Ok(capsule)
    }

    /// Close a capsule, optionally persisting a generated summary first.
    /// Errors if the capsule is missing ([`ServiceError::NotFound`]) or already
    /// closed ([`ServiceError::AlreadyClosed`]). Ports the service `closeCapsule`.
    pub fn close_capsule(
        &self,
        capsule_id: &str,
        options: CloseCapsuleOptions,
    ) -> ServiceResult<Capsule> {
        self.close_capsule_at(capsule_id, options, now_ms())
    }

    /// [`Self::close_capsule`] with an explicit clock for deterministic tests.
    pub fn close_capsule_at(
        &self,
        capsule_id: &str,
        options: CloseCapsuleOptions,
        now: Timestamp,
    ) -> ServiceResult<Capsule> {
        // Distinguish not-found from already-closed up front (the store
        // collapses both into one error; the TS service reports them
        // separately).
        let mut capsule = match self.store.get_capsule(capsule_id)? {
            None => return Err(ServiceError::NotFound(capsule_id.to_string())),
            Some(capsule) => capsule,
        };
        if capsule.status == CapsuleStatus::Closed {
            return Err(ServiceError::AlreadyClosed(capsule_id.to_string()));
        }

        // Generate + persist the summary before closing, matching TS ordering.
        if options.generate_summary {
            if let Some(content) = options.summary_content {
                let summary = validation::validate_summary(
                    SummaryInput {
                        id: None,
                        capsule_id: capsule_id.to_string(),
                        content,
                        confidence: options.confidence.unwrap_or(1.0),
                        created_at: Some(now),
                        evidence_refs: Vec::new(),
                    },
                    now,
                )
                .map_err(ServiceError::Validation)?;
                self.store.insert_summary(&summary)?;
            }
        }

        // Close in the store. The summary (if any) is linked via
        // summaries.capsule_id, so no summary_id is threaded here.
        match self.store.close_capsule(capsule_id, Some(now), None) {
            Ok(()) => {}
            // Lost a race: someone closed it between our read and this write.
            Err(StoreError::CapsuleNotOpen(_)) => {
                return Err(ServiceError::AlreadyClosed(capsule_id.to_string()))
            }
            Err(err) => return Err(err.into()),
        }

        capsule.status = CapsuleStatus::Closed;
        capsule.closed_at = Some(now);
        Ok(capsule)
    }

    // ===== observations =====

    /// Validate/normalise, secret-mask, store, and (optionally) attach an
    /// observation. Returns the stored observation (with masked content and
    /// any defaulted fields). Ports `appendObservation`, plus the
    /// service-boundary secret masking that has no TS equivalent.
    pub fn append_observation(
        &self,
        input: ObservationInput,
        options: AppendObservationOptions,
    ) -> ServiceResult<Observation> {
        self.append_observation_at(input, options, now_ms())
    }

    /// [`Self::append_observation`] with an explicit clock for deterministic
    /// tests.
    pub fn append_observation_at(
        &self,
        input: ObservationInput,
        options: AppendObservationOptions,
        now: Timestamp,
    ) -> ServiceResult<Observation> {
        let mut observation = if options.validate {
            validation::validate_observation(input, now).map_err(ServiceError::Validation)?
        } else {
            validation::normalize_observation(input, now)
        };

        // Service-boundary secret masking: mask (do NOT truncate — truncation
        // is a hook-layer concern owned by PORT-009) so no consumer can route
        // around secret filtering. Applies on both the validated and the
        // validate:false path.
        observation.content = mask_secrets(&observation.content);

        self.store.insert_observation(&observation)?;

        if let Some(capsule_id) = options.capsule_id.as_deref() {
            self.store
                .attach_observation_to_capsule(capsule_id, &observation.id)?;
        }

        Ok(observation)
    }

    // ===== retrieval =====

    /// Retrieve relevant context, scored as of the current time.
    pub fn retrieve(&self, options: RetrieveOptions) -> ServiceResult<RetrieveResult> {
        self.retrieve_at(options, now_ms())
    }

    /// [`Self::retrieve`] with an explicit clock for deterministic tests.
    pub fn retrieve_at(
        &self,
        options: RetrieveOptions,
        now: Timestamp,
    ) -> ServiceResult<RetrieveResult> {
        let provider = LocalFtsProvider::from_store(&self.store);
        Ok(retrieve_at(&self.store, &provider, &options, now)?)
    }

    // ===== pins =====

    /// Create a pin. `note` → reason; `ttl_ms` → `expires_at = now + ttl_ms`.
    /// Ports `pin`.
    pub fn pin(&self, options: CreatePinOptions) -> ServiceResult<Pin> {
        self.pin_at(options, now_ms())
    }

    /// [`Self::pin`] with an explicit clock for deterministic tests.
    pub fn pin_at(&self, options: CreatePinOptions, now: Timestamp) -> ServiceResult<Pin> {
        let expires_at = options.ttl_ms.map(|ttl| now + ttl);
        let pin = validation::validate_pin(
            PinInput {
                id: None,
                target_type: options.target_type,
                target_id: options.target_id,
                reason: options.note,
                created_at: Some(now),
                expires_at,
                scope_ids: options.scope_ids.unwrap_or_default(),
            },
            now,
        )
        .map_err(ServiceError::Validation)?;

        self.store.insert_pin(&pin)?;
        Ok(pin)
    }

    /// Remove a pin. Errors with [`ServiceError::Store`] if it does not exist.
    /// Ports `unpin`.
    pub fn unpin(&self, pin_id: &str) -> ServiceResult<()> {
        self.store.delete_pin(pin_id)?;
        Ok(())
    }

    // ===== read accessors =====

    /// Redact an observation (content replaced, `redacted` set). Ports `forget`.
    pub fn forget(&self, observation_id: &str) -> ServiceResult<()> {
        self.store.redact_observation(observation_id)?;
        Ok(())
    }

    /// Capsule by id. Ports `getCapsule`.
    pub fn get_capsule(&self, capsule_id: &str) -> ServiceResult<Option<Capsule>> {
        Ok(self.store.get_capsule(capsule_id)?)
    }

    /// Open capsule for a session, if any. Ports `getOpenCapsule`.
    pub fn get_open_capsule(&self, session_id: &str) -> ServiceResult<Option<Capsule>> {
        Ok(self.store.get_open_capsule_for_session(session_id)?)
    }

    /// Observation by id. Ports `getObservation`.
    pub fn get_observation(&self, observation_id: &str) -> ServiceResult<Option<Observation>> {
        Ok(self.store.get_observation_by_id(observation_id)?)
    }

    /// Summary by id. Ports `getSummary`.
    pub fn get_summary(&self, summary_id: &str) -> ServiceResult<Option<Summary>> {
        Ok(self.store.get_summary_by_id(summary_id)?)
    }

    /// Latest summary for a capsule, if any. (Read helper used by callers and
    /// tests; the TS service exposes the same via the store.)
    pub fn get_latest_summary(&self, capsule_id: &str) -> ServiceResult<Option<Summary>> {
        Ok(self.store.get_latest_summary_for_capsule(capsule_id)?)
    }

    /// Active pins for a scope, as of the current time. Ports `listPins`.
    pub fn list_pins(&self, scope: Option<&ScopeIds>) -> ServiceResult<Vec<Pin>> {
        self.list_pins_at(scope, now_ms())
    }

    /// [`Self::list_pins`] with an explicit clock for deterministic tests.
    pub fn list_pins_at(
        &self,
        scope: Option<&ScopeIds>,
        now: Timestamp,
    ) -> ServiceResult<Vec<Pin>> {
        Ok(self.store.list_active_pins(scope, Some(now))?)
    }

    // ===== injection context (hook support) =====

    /// Assemble the structured data for the SessionStart injection, scored as
    /// of the current time.
    pub fn session_start_context(
        &self,
        scope: &ScopeIds,
        max_results: u32,
    ) -> ServiceResult<SessionStartContext> {
        self.session_start_context_at(scope, max_results, now_ms())
    }

    /// [`Self::session_start_context`] with an explicit clock for deterministic
    /// tests (controls active-pin expiry).
    ///
    /// Ports the inline queries in
    /// `plugins/kindling-claude-code/hooks/session-start.js`:
    /// active pins for the scope (resolved to target content) plus the most
    /// recent non-redacted observations for the scope, capped at `max_results`.
    pub fn session_start_context_at(
        &self,
        scope: &ScopeIds,
        max_results: u32,
        now: Timestamp,
    ) -> ServiceResult<SessionStartContext> {
        let pins = self.resolved_active_pins(scope, now)?;
        // The Node hook orders by `ts DESC LIMIT maxResults`, excluding
        // redacted rows — exactly `query_observations` with no time bounds.
        let recent = self
            .store
            .query_observations(Some(scope), None, None, max_results)?;
        Ok(SessionStartContext { pins, recent })
    }

    /// Assemble the structured data for the PreCompact injection, scored as of
    /// the current time.
    pub fn pre_compact_context(&self, scope: &ScopeIds) -> ServiceResult<PreCompactContext> {
        self.pre_compact_context_at(scope, now_ms())
    }

    /// [`Self::pre_compact_context`] with an explicit clock for deterministic
    /// tests (controls active-pin expiry).
    ///
    /// Ports the inline queries in
    /// `plugins/kindling-claude-code/hooks/pre-compact.js`: active pins for the
    /// scope (resolved to target content) plus the single latest summary across
    /// the scope's capsules. An empty-content summary is normalised to `None`
    /// here, matching the Node hook's `latestSummary.content` truthiness gate,
    /// so the server never has to second-guess it.
    pub fn pre_compact_context_at(
        &self,
        scope: &ScopeIds,
        now: Timestamp,
    ) -> ServiceResult<PreCompactContext> {
        let pins = self.resolved_active_pins(scope, now)?;
        let latest_summary = self
            .store
            .latest_summary_for_scope(Some(scope))?
            .filter(|s| !s.content.is_empty());
        Ok(PreCompactContext {
            pins,
            latest_summary,
        })
    }

    /// Active pins for `scope` at `now`, each resolved to its target's content.
    ///
    /// Mirrors the TS `listActivePins` join: `note` is the pin reason, `content`
    /// is the target observation/summary content (redacted observations carry
    /// their `[redacted]` placeholder; missing targets resolve to `None`).
    fn resolved_active_pins(
        &self,
        scope: &ScopeIds,
        now: Timestamp,
    ) -> ServiceResult<Vec<ResolvedPin>> {
        let pins = self.store.list_active_pins(Some(scope), Some(now))?;
        pins.into_iter()
            .map(|pin| {
                let content = match pin.target_type {
                    PinTargetType::Observation => self
                        .store
                        .get_observation_by_id(&pin.target_id)?
                        .map(|o| o.content),
                    PinTargetType::Summary => self
                        .store
                        .get_summary_by_id(&pin.target_id)?
                        .map(|s| s.content),
                };
                Ok(ResolvedPin {
                    note: pin.reason,
                    content,
                })
            })
            .collect()
    }
}

/// Current time in epoch milliseconds.
fn now_ms() -> Timestamp {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_millis() as Timestamp
}
