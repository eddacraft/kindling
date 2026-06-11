/**
 * IntentEvent types and definitions (contract v1)
 *
 * An IntentEvent is an append-only record of what a developer or agent
 * intended, under what constraints, and in what execution context. It is
 * the canonical capture contract consumed downstream (e.g., Anvil), so
 * field names use snake_case as the stable interchange format — unlike
 * the camelCase used by in-process domain types.
 *
 * Contract reference: plans/modules/04-intent-capture-events.aps.md
 */

/**
 * Current schema version for the IntentEvent contract
 */
export const INTENT_EVENT_SCHEMA_VERSION = '1.0' as const;

export type IntentEventSchemaVersion = typeof INTENT_EVENT_SCHEMA_VERSION;

/**
 * High-signal moments at which intent is captured
 */
export type IntentEventType =
  | 'intent.session_started' // New session began
  | 'intent.prompt_submitted' // Developer/agent submitted a prompt
  | 'intent.constraints_updated' // Constraints were added or changed
  | 'intent.task_reframed' // The task objective was revised
  | 'intent.checkpoint_created'; // A commit checkpoint was created

/**
 * Who produced the intent
 */
export type IntentActorKind = 'human' | 'agent';

export interface IntentActor {
  kind: IntentActorKind;
  id?: string;
  /** Originating tool (e.g., claude-code, codex) */
  tool?: string;
  model?: string;
}

/**
 * Execution context the intent was captured in
 */
export interface IntentContext {
  workspace_id: string;
  repo: string;
  branch?: string;
  commit?: string;
  session_id?: string;
  thread_id?: string;
}

/**
 * The captured intent itself
 */
export interface IntentPayload {
  objective: string;
  constraints?: string[];
  success_criteria?: string[];
  scope_in?: string[];
  scope_out?: string[];
}

/**
 * Lineage and integrity metadata
 */
export interface IntentProvenance {
  parent_event_id?: string;
  source_refs?: string[];
  /** Rolling hash linking this event into the append-only chain */
  integrity_hash: string;
}

/**
 * Record of what was redacted before persistence/export
 */
export interface IntentRedaction {
  redacted_fields?: string[];
  policy_version?: string;
}

/**
 * IntentEvent envelope (contract v1)
 *
 * Append-only and immutable once persisted.
 */
export interface IntentEvent {
  schema_version: IntentEventSchemaVersion;

  /** Unique identifier (UUIDv4) */
  event_id: string;

  /** ISO8601 timestamp of when the intent occurred */
  occurred_at: string;

  /** Monotonic per repo workspace; assigned by the append-only store */
  sequence: number;

  event_type: IntentEventType;
  actor: IntentActor;
  context: IntentContext;
  intent: IntentPayload;
  provenance: IntentProvenance;
  redaction: IntentRedaction;
}

/**
 * Input for validating a completed intent event envelope
 * Makes schema_version, event_id, occurred_at, and redaction optional
 * (auto-generated or defaulted during validation)
 *
 * `sequence` and `provenance.integrity_hash` are required here because
 * the append-only store assigns them before validating the envelope —
 * emitters hand the store a draft, not an IntentEventInput.
 */
export interface IntentEventInput {
  schema_version?: IntentEventSchemaVersion;
  event_id?: string;
  occurred_at?: string;
  sequence: number;
  event_type: IntentEventType;
  actor: IntentActor;
  context: IntentContext;
  intent: IntentPayload;
  provenance: IntentProvenance;
  redaction?: IntentRedaction;
}

/**
 * All valid intent event types
 */
export const INTENT_EVENT_TYPES: readonly IntentEventType[] = [
  'intent.session_started',
  'intent.prompt_submitted',
  'intent.constraints_updated',
  'intent.task_reframed',
  'intent.checkpoint_created',
] as const;

/**
 * All valid intent actor kinds
 */
export const INTENT_ACTOR_KINDS: readonly IntentActorKind[] = ['human', 'agent'] as const;

/**
 * Type guard to check if a string is a valid IntentEventType
 */
export function isIntentEventType(value: unknown): value is IntentEventType {
  return typeof value === 'string' && INTENT_EVENT_TYPES.includes(value as IntentEventType);
}

/**
 * Type guard to check if a string is a valid IntentActorKind
 */
export function isIntentActorKind(value: unknown): value is IntentActorKind {
  return typeof value === 'string' && INTENT_ACTOR_KINDS.includes(value as IntentActorKind);
}
