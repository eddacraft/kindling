/**
 * High-signal intent emitters (KINTENT-002)
 *
 * Emitters capture intent at the points with the highest signal-to-noise in an
 * AI-assisted workflow: session start, prompt submission, constraint/task
 * revisions, and commit checkpoints. Each emitter method shapes the correct
 * `event_type`, merges the configured base context/actor with any per-call
 * overrides, then appends the event through an {@link IntentStore} (which owns
 * sequencing and integrity hashing).
 *
 * Emitters are mechanism, not policy: they do not fabricate objective text or
 * gather git/session context themselves. Callers (hooks, adapters) supply the
 * intent payload and context; the emitter only stamps the moment.
 */

import type {
  IntentActor,
  IntentContext,
  IntentPayload,
  IntentRedaction,
  IntentEvent,
  IntentEventType,
  Result,
  ValidationError,
} from '../types/index.js';
import type { IntentStore, IntentEventDraft } from './store.js';

/**
 * Configuration shared across all events an emitter produces.
 */
export interface IntentEmitterConfig {
  store: IntentStore;
  /** Base execution context applied to every event. */
  context: IntentContext;
  /** Default actor applied to every event. */
  actor: IntentActor;
  /** Default redaction metadata recorded on every event. */
  redaction?: IntentRedaction;
}

/**
 * Per-call options common to every emitter method.
 */
export interface EmitOptions {
  /**
   * Fields merged (shallow) onto the configured base actor for this event.
   * Unspecified fields carry over from the base — e.g. overriding only
   * `{ kind: 'human', id: 'josh' }` keeps the base `tool`/`model`. Pass those
   * explicitly (or as `undefined`) to clear them when switching actor kind.
   */
  actor?: Partial<IntentActor>;
  /** Fields merged (shallow) onto the base context for this event. */
  context?: Partial<IntentContext>;
  /** Semantic parent event (e.g., the task this one reframes). */
  parent_event_id?: string;
  /** External references backing this intent. */
  source_refs?: string[];
  /** Per-call redaction metadata (replaces the configured default). */
  redaction?: IntentRedaction;
}

/**
 * Options for {@link IntentEmitter.checkpointCreated}, which additionally
 * stamps the commit that the checkpoint corresponds to.
 */
export interface CheckpointOptions extends EmitOptions {
  commit?: string;
}

export class IntentEmitter {
  private readonly store: IntentStore;
  private readonly context: IntentContext;
  private readonly actor: IntentActor;
  private readonly redaction?: IntentRedaction;

  constructor(config: IntentEmitterConfig) {
    this.store = config.store;
    this.context = config.context;
    this.actor = config.actor;
    this.redaction = config.redaction;
  }

  /** A new session began. */
  sessionStarted(
    intent: IntentPayload,
    options: EmitOptions = {},
  ): Result<IntentEvent, ValidationError[]> {
    return this.emit('intent.session_started', intent, options);
  }

  /** A developer/agent submitted a prompt. */
  promptSubmitted(
    intent: IntentPayload,
    options: EmitOptions = {},
  ): Result<IntentEvent, ValidationError[]> {
    return this.emit('intent.prompt_submitted', intent, options);
  }

  /** Constraints were added or changed. */
  constraintsUpdated(
    intent: IntentPayload,
    options: EmitOptions = {},
  ): Result<IntentEvent, ValidationError[]> {
    return this.emit('intent.constraints_updated', intent, options);
  }

  /** The task objective was revised. */
  taskReframed(
    intent: IntentPayload,
    options: EmitOptions = {},
  ): Result<IntentEvent, ValidationError[]> {
    return this.emit('intent.task_reframed', intent, options);
  }

  /** A commit checkpoint was created. */
  checkpointCreated(
    intent: IntentPayload,
    options: CheckpointOptions = {},
  ): Result<IntentEvent, ValidationError[]> {
    const { commit, ...rest } = options;
    const context = commit ? { ...rest.context, commit } : rest.context;
    return this.emit('intent.checkpoint_created', intent, { ...rest, context });
  }

  private emit(
    eventType: IntentEventType,
    intent: IntentPayload,
    options: EmitOptions,
  ): Result<IntentEvent, ValidationError[]> {
    const draft: IntentEventDraft = {
      event_type: eventType,
      actor: { ...this.actor, ...options.actor },
      context: { ...this.context, ...options.context },
      intent,
      provenance:
        options.parent_event_id || options.source_refs
          ? { parent_event_id: options.parent_event_id, source_refs: options.source_refs }
          : undefined,
      redaction: options.redaction ?? this.redaction,
    };
    return this.store.append(draft);
  }
}
