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
