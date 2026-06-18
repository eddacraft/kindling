//! Export / import bundle coordination.
//!
//! Ports the TS export pipeline that lived in
//! `packages/kindling-core/src/export/{bundle.ts,restore.ts}` plus the
//! store-level primitives in
//! `packages/kindling-store-sqlite/src/store/export.ts`. The JSON shape of
//! [`ExportBundle`] is byte-compatible with the TS `ExportBundle` so a bundle
//! produced here round-trips through the TS importer and vice versa.
//!
//! Deferred from PORT-006 (the service deliberately shipped without
//! export/import); owned here by PORT-012 because the CLI is the only consumer.
//!
//! # Key-ordering parity
//!
//! `serde_json` is built with `preserve_order` (enabled in this crate's
//! `Cargo.toml`), so struct fields serialize in declaration order. The field
//! order below mirrors the object-literal construction order in the TS source:
//!
//! * dataset: `version, exportedAt, scope, observations, capsules, summaries,
//!   pins` (the `exportDatabase` return literal).
//! * bundle: `bundleVersion, exportedAt, dataset` with `metadata` appended
//!   **after** `dataset` (TS sets `bundle.metadata` only when present, after the
//!   literal is built).
//!
//! `scope` and `metadata` are omitted entirely when absent — matching TS, which
//! never serializes them as `null` (an undefined property is dropped by
//! `JSON.stringify`).

use serde::{Deserialize, Serialize};

use kindling_types::{Capsule, Observation, Pin, ScopeIds, Summary, Timestamp};

use crate::error::ServiceResult;
use crate::KindlingService;

/// Bundle format version. Mirrors the TS `bundleVersion`/`version` literals.
pub const BUNDLE_VERSION: &str = "1.0";

/// Options for [`KindlingService::export`].
///
/// Mirrors the union of TS `ExportBundleOptions` (`scope`, `metadata`) and the
/// store-level `ExportOptions` (`includeRedacted`, `limit`). `exported_at` is
/// injected explicitly (rather than read from a clock) so exports are
/// deterministic and testable — the CLI passes the timestamp it stamps into the
/// default output filename.
#[derive(Debug, Clone, Default)]
pub struct ExportBundleOptions {
    /// Optional scope filter applied to every entity.
    pub scope: Option<ScopeIds>,
    /// Include redacted observations (TS default: false).
    pub include_redacted: bool,
    /// Maximum observations to export.
    pub limit: Option<u32>,
    /// Optional bundle metadata (serialized verbatim as a JSON object).
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
    /// Export timestamp stamped into both the bundle and the dataset.
    pub exported_at: Timestamp,
}

/// The entity dataset inside an [`ExportBundle`]. Mirrors the TS `ExportDataset`
/// shape returned by `exportDatabase`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportDataset {
    /// Schema version for forward compatibility (`"1.0"`).
    pub version: String,
    /// Export timestamp (epoch ms).
    pub exported_at: Timestamp,
    /// Scope filter applied, when any. Omitted from JSON when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<ScopeIds>,
    /// Observations ordered `ts ASC, id ASC`.
    pub observations: Vec<Observation>,
    /// Capsules ordered `opened_at ASC, id ASC`.
    pub capsules: Vec<Capsule>,
    /// Summaries ordered `created_at ASC, id ASC`.
    pub summaries: Vec<Summary>,
    /// Pins ordered `created_at ASC, id ASC`.
    pub pins: Vec<Pin>,
}

/// A portable export bundle. JSON-compatible with the TS `ExportBundle`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportBundle {
    /// Bundle format version (`"1.0"`).
    pub bundle_version: String,
    /// Export timestamp (epoch ms).
    pub exported_at: Timestamp,
    /// The entity dataset.
    pub dataset: ExportDataset,
    /// Optional metadata. Declared **after** `dataset` to match the TS field
    /// order; omitted from JSON when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
}

/// Summary statistics for an [`ExportBundle`]. Mirrors the TS `ExportStats`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportStats {
    pub observations: usize,
    pub capsules: usize,
    pub summaries: usize,
    pub pins: usize,
    /// Length of the compact JSON serialization (`JSON.stringify(bundle).length`
    /// in TS — UTF-16 code units there, bytes here; equal for ASCII-only data).
    pub total_size: usize,
}

/// Options for [`KindlingService::import`]. Mirrors the TS `ImportOptions`.
#[derive(Debug, Clone, Default)]
pub struct ImportOptions {
    /// Validate only; do not write. Mirrors the TS `dryRun`.
    pub dry_run: bool,
}

/// Result of an import. Mirrors the TS `ImportResult`, including the `dryRun`
/// flag echoed back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub observations: usize,
    pub capsules: usize,
    pub summaries: usize,
    pub pins: usize,
    pub errors: Vec<String>,
    pub dry_run: bool,
}

impl ExportBundle {
    /// Serialize to JSON. `pretty` switches between compact (single line, TS
    /// `JSON.stringify(bundle, null, 0)`) and 2-space pretty printing.
    pub fn to_json(&self, pretty: bool) -> ServiceResult<String> {
        let json = if pretty {
            serde_json::to_string_pretty(self)?
        } else {
            serde_json::to_string(self)?
        };
        Ok(json)
    }

    /// Parse a bundle from JSON, validating its structure (version + required
    /// arrays). Mirrors `deserializeBundle` + `validateBundle`.
    pub fn from_json(json: &str) -> ServiceResult<Self> {
        let bundle: ExportBundle = serde_json::from_str(json)?;
        bundle.validate()?;
        Ok(bundle)
    }

    /// Structural validation. Errors mirror the messages produced by the TS
    /// `validateBundle` for the version checks (the array/required-field checks
    /// are enforced statically by the typed deserialization above).
    pub fn validate(&self) -> ServiceResult<()> {
        if self.bundle_version != BUNDLE_VERSION {
            return Err(crate::ServiceError::Validation(vec![
                kindling_types::ValidationError {
                    field: "bundleVersion".to_string(),
                    message: format!("Unsupported bundle version: {}", self.bundle_version),
                    value: None,
                },
            ]));
        }
        if self.dataset.version != BUNDLE_VERSION {
            return Err(crate::ServiceError::Validation(vec![
                kindling_types::ValidationError {
                    field: "dataset.version".to_string(),
                    message: format!("Unsupported schema version: {}", self.dataset.version),
                    value: None,
                },
            ]));
        }
        Ok(())
    }

    /// Bundle statistics (entity counts + serialized size). Mirrors
    /// `getBundleStats`.
    pub fn stats(&self) -> ServiceResult<ExportStats> {
        let total_size = serde_json::to_string(self)?.len();
        Ok(ExportStats {
            observations: self.dataset.observations.len(),
            capsules: self.dataset.capsules.len(),
            summaries: self.dataset.summaries.len(),
            pins: self.dataset.pins.len(),
            total_size,
        })
    }
}

impl KindlingService {
    /// Build an export bundle from the store. Ports `createExportBundle`:
    /// reads each entity table in deterministic order, applies the optional
    /// scope/redaction/limit filters, and wraps the dataset with bundle
    /// metadata.
    pub fn export(&self, options: ExportBundleOptions) -> ServiceResult<ExportBundle> {
        let scope = options.scope.as_ref();
        let observations =
            self.store()
                .export_observations(scope, options.include_redacted, options.limit)?;
        let capsules = self.store().export_capsules(scope)?;
        let summaries = self.store().export_summaries(scope)?;
        let pins = self.store().export_pins(scope)?;

        let dataset = ExportDataset {
            version: BUNDLE_VERSION.to_string(),
            exported_at: options.exported_at,
            scope: options.scope,
            observations,
            capsules,
            summaries,
            pins,
        };

        Ok(ExportBundle {
            bundle_version: BUNDLE_VERSION.to_string(),
            exported_at: options.exported_at,
            dataset,
            metadata: options.metadata,
        })
    }

    /// Restore a bundle into the store. Ports `restoreFromBundle` +
    /// `importDatabase`: validates structure, short-circuits on `dry_run`, and
    /// otherwise imports every entity in a single transaction with
    /// `INSERT OR IGNORE` semantics (existing ids are skipped, not overwritten).
    /// Per-row failures are collected into `errors` rather than aborting.
    pub fn import(
        &self,
        bundle: &ExportBundle,
        options: ImportOptions,
    ) -> ServiceResult<ImportResult> {
        // Validate structure first (matches restoreFromBundle's pre-check). On
        // a bad version, TS returns the validation errors with zero counts
        // rather than throwing.
        if let Err(crate::ServiceError::Validation(errors)) = bundle.validate() {
            return Ok(ImportResult {
                observations: 0,
                capsules: 0,
                summaries: 0,
                pins: 0,
                errors: errors.into_iter().map(|e| e.message).collect(),
                dry_run: options.dry_run,
            });
        }

        if options.dry_run {
            return Ok(ImportResult {
                observations: bundle.dataset.observations.len(),
                capsules: bundle.dataset.capsules.len(),
                summaries: bundle.dataset.summaries.len(),
                pins: bundle.dataset.pins.len(),
                errors: Vec::new(),
                dry_run: true,
            });
        }

        let mut errors: Vec<String> = Vec::new();
        let result = self.store().transaction(|store| {
            let mut observations = 0usize;
            let mut capsules = 0usize;
            let mut summaries = 0usize;
            let mut pins = 0usize;

            for obs in &bundle.dataset.observations {
                match store.import_observation(obs) {
                    Ok(true) => observations += 1,
                    Ok(false) => {}
                    Err(err) => {
                        errors.push(format!("Failed to import observation {}: {err}", obs.id))
                    }
                }
            }
            for capsule in &bundle.dataset.capsules {
                match store.import_capsule(capsule) {
                    Ok(true) => capsules += 1,
                    Ok(false) => {}
                    Err(err) => {
                        errors.push(format!("Failed to import capsule {}: {err}", capsule.id))
                    }
                }
            }
            for summary in &bundle.dataset.summaries {
                match store.import_summary(summary) {
                    Ok(true) => summaries += 1,
                    Ok(false) => {}
                    Err(err) => {
                        errors.push(format!("Failed to import summary {}: {err}", summary.id))
                    }
                }
            }
            for pin in &bundle.dataset.pins {
                match store.import_pin(pin) {
                    Ok(true) => pins += 1,
                    Ok(false) => {}
                    Err(err) => errors.push(format!("Failed to import pin {}: {err}", pin.id)),
                }
            }

            Ok((observations, capsules, summaries, pins))
        })?;

        let (observations, capsules, summaries, pins) = result;
        Ok(ImportResult {
            observations,
            capsules,
            summaries,
            pins,
            errors,
            dry_run: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kindling_types::{
        Capsule, CapsuleStatus, CapsuleType, Observation, ObservationKind, Pin, PinTargetType,
        Summary,
    };

    fn seeded() -> KindlingService {
        let service = KindlingService::open_in_memory().unwrap();
        let store = service.store();
        store
            .insert_observation(&Observation {
                id: "o1".into(),
                kind: ObservationKind::Message,
                content: "hi".into(),
                provenance: serde_json::Map::new(),
                ts: 10,
                scope_ids: ScopeIds::default(),
                redacted: false,
            })
            .unwrap();
        store
            .create_capsule(&Capsule {
                id: "c1".into(),
                kind: CapsuleType::Session,
                intent: "i".into(),
                status: CapsuleStatus::Closed,
                opened_at: 5,
                closed_at: Some(20),
                scope_ids: ScopeIds::default(),
                observation_ids: vec![],
                summary_id: None,
            })
            .unwrap();
        store
            .insert_summary(&Summary {
                id: "s1".into(),
                capsule_id: "c1".into(),
                content: "sum".into(),
                confidence: 0.5,
                created_at: 15,
                evidence_refs: vec![],
            })
            .unwrap();
        store
            .insert_pin(&Pin {
                id: "p1".into(),
                target_type: PinTargetType::Observation,
                target_id: "o1".into(),
                reason: None,
                created_at: 12,
                expires_at: None,
                scope_ids: ScopeIds::default(),
            })
            .unwrap();
        service
    }

    fn export_opts() -> ExportBundleOptions {
        ExportBundleOptions {
            scope: None,
            include_redacted: false,
            limit: None,
            metadata: None,
            exported_at: 100,
        }
    }

    #[test]
    fn export_then_import_into_fresh_store_round_trips() {
        let bundle = seeded().export(export_opts()).unwrap();

        let dest = KindlingService::open_in_memory().unwrap();
        let result = dest.import(&bundle, ImportOptions::default()).unwrap();
        assert_eq!(result.observations, 1);
        assert_eq!(result.capsules, 1);
        assert_eq!(result.summaries, 1);
        assert_eq!(result.pins, 1);
        assert!(result.errors.is_empty());
        assert!(!result.dry_run);

        // Re-exporting the destination yields the same dataset.
        let re = dest.export(export_opts()).unwrap();
        assert_eq!(re.dataset.observations, bundle.dataset.observations);
        assert_eq!(re.dataset.capsules, bundle.dataset.capsules);
        assert_eq!(re.dataset.summaries, bundle.dataset.summaries);
        assert_eq!(re.dataset.pins, bundle.dataset.pins);
    }

    #[test]
    fn import_is_idempotent() {
        let bundle = seeded().export(export_opts()).unwrap();
        let dest = KindlingService::open_in_memory().unwrap();
        dest.import(&bundle, ImportOptions::default()).unwrap();
        let again = dest.import(&bundle, ImportOptions::default()).unwrap();
        assert_eq!(again.observations, 0);
        assert_eq!(again.capsules, 0);
        assert_eq!(again.summaries, 0);
        assert_eq!(again.pins, 0);
    }

    #[test]
    fn dry_run_counts_without_writing() {
        let bundle = seeded().export(export_opts()).unwrap();
        let dest = KindlingService::open_in_memory().unwrap();
        let result = dest
            .import(&bundle, ImportOptions { dry_run: true })
            .unwrap();
        assert!(result.dry_run);
        assert_eq!(result.observations, 1);
        // Nothing persisted.
        assert_eq!(dest.store().database_stats().unwrap().observations, 0);
    }

    #[test]
    fn bad_version_returns_errors_not_panic() {
        let mut bundle = seeded().export(export_opts()).unwrap();
        bundle.dataset.version = "9.9".into();
        let dest = KindlingService::open_in_memory().unwrap();
        let result = dest.import(&bundle, ImportOptions::default()).unwrap();
        assert_eq!(result.observations, 0);
        assert!(result.errors.iter().any(|e| e.contains("9.9")));
    }

    #[test]
    fn export_respects_scope_filter() {
        let service = KindlingService::open_in_memory().unwrap();
        let scoped = ScopeIds {
            session_id: Some("keep".into()),
            ..Default::default()
        };
        let other = ScopeIds {
            session_id: Some("drop".into()),
            ..Default::default()
        };
        for (id, scope) in [("a", &scoped), ("b", &other)] {
            service
                .store()
                .insert_observation(&Observation {
                    id: id.into(),
                    kind: ObservationKind::Message,
                    content: id.into(),
                    provenance: serde_json::Map::new(),
                    ts: 1,
                    scope_ids: scope.clone(),
                    redacted: false,
                })
                .unwrap();
        }
        let bundle = service
            .export(ExportBundleOptions {
                scope: Some(scoped),
                ..export_opts()
            })
            .unwrap();
        assert_eq!(bundle.dataset.observations.len(), 1);
        assert_eq!(bundle.dataset.observations[0].id, "a");
    }
}
