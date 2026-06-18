/**
 * kindling Core Types
 *
 * Re-exports all domain types for convenient importing
 */

// Common types
export type { ID, Timestamp, ScopeIds, Result, ValidationError } from './common.js';

export { ok, err } from './common.js';

// Observation types
export type { ObservationKind, Observation, ObservationInput } from './observation.js';

export { OBSERVATION_KINDS, isObservationKind } from './observation.js';

// Capsule types
export type { CapsuleType, CapsuleStatus, Capsule, CapsuleInput } from './capsule.js';

export { CAPSULE_TYPES, CAPSULE_STATUSES, isCapsuleType, isCapsuleStatus } from './capsule.js';

// Summary types
export type { Summary, SummaryInput } from './summary.js';

export { isValidConfidence } from './summary.js';

// Pin types
export type { PinTargetType, Pin, PinInput } from './pin.js';

export { PIN_TARGET_TYPES, isPinTargetType, isPinActive } from './pin.js';

// Intent event types
export type {
  IntentEventSchemaVersion,
  IntentEventType,
  IntentActorKind,
  IntentActor,
  IntentContext,
  IntentPayload,
  IntentProvenance,
  IntentRedaction,
  IntentEvent,
  IntentEventInput,
} from './intent-event.js';

export {
  INTENT_EVENT_SCHEMA_VERSION,
  INTENT_EVENT_TYPES,
  INTENT_ACTOR_KINDS,
  isIntentEventType,
  isIntentActorKind,
} from './intent-event.js';

// Retrieval types
export type {
  RetrieveOptions,
  PinResult,
  CandidateResult,
  RetrieveProvenance,
  RetrieveResult,
  ProviderSearchOptions,
  ProviderSearchResult,
  RetrievalProvider,
} from './retrieval.js';
