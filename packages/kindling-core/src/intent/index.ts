/**
 * Intent capture: append-only store (KINTENT-003), high-signal emitters
 * (KINTENT-002), redaction boundary (KINTENT-004), and signed export
 * (KINTENT-005), and the capture health report (KINTENT-006).
 */

export { canonicalize } from './canonical.js';

export { IntentStore, GENESIS_HASH } from './store.js';
export type {
  IntentEventDraft,
  IntentStoreOptions,
  IntentIntegrityError,
  IntentIntegrityErrorKind,
} from './store.js';

export { IntentEmitter } from './emitter.js';
export type { IntentEmitterConfig, EmitOptions, CheckpointOptions } from './emitter.js';

export {
  IntentRedactor,
  DEFAULT_REDACTION_POLICY,
  DEFAULT_REDACTION_PATTERNS,
  DEFAULT_REDACTION_PLACEHOLDER,
} from './redaction.js';
export type { RedactionPolicy, RedactionPattern } from './redaction.js';

export {
  createIntentExport,
  verifyIntentExport,
  serializeIntentExport,
  parseIntentExport,
  INTENT_EXPORT_BUNDLE_VERSION,
  INTENT_EXPORT_SIGNATURE_ALG,
} from './export.js';
export type {
  IntentExportBundle,
  IntentExportManifest,
  IntentExportSignature,
  IntentExportSequenceRange,
  IntentExportBundleVersion,
  IntentExportSignatureAlg,
  CreateIntentExportOptions,
  IntentExportError,
  IntentExportErrorKind,
} from './export.js';

export { computeIntentStatus, formatIntentStatus } from './status.js';
export type {
  IntentStatusReport,
  IntentStatusLastEvent,
  IntentStatusIntegrity,
  IntentStatusOptions,
  IntentStatusSource,
} from './status.js';
