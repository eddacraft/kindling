/**
 * Intent capture: append-only store (KINTENT-003) and high-signal emitters
 * (KINTENT-002).
 */

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
