/**
 * IntentEvent validation
 */

import { randomUUID } from 'crypto';
import type {
  IntentEvent,
  IntentActor,
  IntentContext,
  IntentPayload,
  IntentProvenance,
  IntentRedaction,
  ValidationError,
  Result,
} from '../types/index.js';
import {
  ok,
  err,
  isIntentEventType,
  isIntentActorKind,
  INTENT_EVENT_SCHEMA_VERSION,
} from '../types/index.js';

/**
 * Validate that an optional field is an array of strings
 */
function validateStringList(value: unknown, field: string, errors: ValidationError[]): void {
  if (value === undefined) {
    return;
  }
  if (!Array.isArray(value) || value.some((entry) => typeof entry !== 'string')) {
    errors.push({ field, message: `${field} must be an array of strings`, value });
  }
}

/**
 * Validate that an optional field is a string
 */
function validateOptionalString(value: unknown, field: string, errors: ValidationError[]): void {
  if (value !== undefined && typeof value !== 'string') {
    errors.push({ field, message: `${field} must be a string`, value: typeof value });
  }
}

/**
 * Validate and normalize an intent event input against contract v1
 *
 * Auto-generates:
 * - schema_version (defaults to current contract version)
 * - event_id (if not provided)
 * - occurred_at (if not provided)
 * - redaction (defaults to empty object)
 *
 * `sequence` and `provenance.integrity_hash` are required: emitters hand
 * a draft to the append-only store, which assigns both and then validates
 * the completed envelope with this function before appending. An event
 * without them is not yet a valid IntentEvent.
 *
 * @param input - Intent event input to validate
 * @returns Result containing validated IntentEvent or validation errors
 */
export function validateIntentEvent(input: unknown): Result<IntentEvent, ValidationError[]> {
  const errors: ValidationError[] = [];

  // Type check
  if (typeof input !== 'object' || input === null) {
    return err([{ field: 'input', message: 'Input must be an object' }]);
  }

  const data = input as Record<string, unknown>;

  // Validate schema_version (optional, must match current contract)
  if (data.schema_version !== undefined && data.schema_version !== INTENT_EVENT_SCHEMA_VERSION) {
    errors.push({
      field: 'schema_version',
      message: `schema_version must be '${INTENT_EVENT_SCHEMA_VERSION}'`,
      value: data.schema_version,
    });
  }

  // Validate event_id (optional, must be string)
  validateOptionalString(data.event_id, 'event_id', errors);

  // Validate occurred_at (optional, must be a parseable ISO8601 string)
  if (data.occurred_at !== undefined) {
    if (
      typeof data.occurred_at !== 'string' ||
      !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/.test(data.occurred_at) ||
      Number.isNaN(Date.parse(data.occurred_at))
    ) {
      errors.push({
        field: 'occurred_at',
        message: 'occurred_at must be an ISO8601 timestamp string',
        value: data.occurred_at,
      });
    }
  }

  // Validate sequence (required, non-negative integer)
  if (data.sequence === undefined) {
    errors.push({ field: 'sequence', message: 'sequence is required' });
  } else if (
    typeof data.sequence !== 'number' ||
    !Number.isInteger(data.sequence) ||
    data.sequence < 0
  ) {
    errors.push({
      field: 'sequence',
      message: 'sequence must be a non-negative integer',
      value: data.sequence,
    });
  }

  // Validate event_type (required)
  if (data.event_type === undefined || data.event_type === null) {
    errors.push({ field: 'event_type', message: 'event_type is required' });
  } else if (!isIntentEventType(data.event_type)) {
    errors.push({
      field: 'event_type',
      message: `Invalid intent event type: ${data.event_type}`,
      value: data.event_type,
    });
  }

  // Validate actor (required, kind must be valid)
  if (typeof data.actor !== 'object' || data.actor === null || Array.isArray(data.actor)) {
    errors.push({ field: 'actor', message: 'actor is required and must be an object' });
  } else {
    const actor = data.actor as Record<string, unknown>;
    if (!isIntentActorKind(actor.kind)) {
      errors.push({
        field: 'actor.kind',
        message: `actor.kind must be one of: human, agent`,
        value: actor.kind,
      });
    }
    validateOptionalString(actor.id, 'actor.id', errors);
    validateOptionalString(actor.tool, 'actor.tool', errors);
    validateOptionalString(actor.model, 'actor.model', errors);
  }

  // Validate context (required, workspace_id and repo required)
  if (typeof data.context !== 'object' || data.context === null || Array.isArray(data.context)) {
    errors.push({ field: 'context', message: 'context is required and must be an object' });
  } else {
    const context = data.context as Record<string, unknown>;
    if (typeof context.workspace_id !== 'string' || context.workspace_id.length === 0) {
      errors.push({
        field: 'context.workspace_id',
        message: 'context.workspace_id is required and must be a non-empty string',
      });
    }
    if (typeof context.repo !== 'string' || context.repo.length === 0) {
      errors.push({
        field: 'context.repo',
        message: 'context.repo is required and must be a non-empty string',
      });
    }
    validateOptionalString(context.branch, 'context.branch', errors);
    validateOptionalString(context.commit, 'context.commit', errors);
    validateOptionalString(context.session_id, 'context.session_id', errors);
    validateOptionalString(context.thread_id, 'context.thread_id', errors);
  }

  // Validate intent (required, objective must be non-empty)
  if (typeof data.intent !== 'object' || data.intent === null || Array.isArray(data.intent)) {
    errors.push({ field: 'intent', message: 'intent is required and must be an object' });
  } else {
    const intent = data.intent as Record<string, unknown>;
    if (typeof intent.objective !== 'string' || intent.objective.trim().length === 0) {
      errors.push({
        field: 'intent.objective',
        message: 'intent.objective is required and must be a non-empty string',
      });
    }
    validateStringList(intent.constraints, 'intent.constraints', errors);
    validateStringList(intent.success_criteria, 'intent.success_criteria', errors);
    validateStringList(intent.scope_in, 'intent.scope_in', errors);
    validateStringList(intent.scope_out, 'intent.scope_out', errors);
  }

  // Validate provenance (required, integrity_hash required)
  if (
    typeof data.provenance !== 'object' ||
    data.provenance === null ||
    Array.isArray(data.provenance)
  ) {
    errors.push({ field: 'provenance', message: 'provenance is required and must be an object' });
  } else {
    const provenance = data.provenance as Record<string, unknown>;
    if (typeof provenance.integrity_hash !== 'string' || provenance.integrity_hash.length === 0) {
      errors.push({
        field: 'provenance.integrity_hash',
        message: 'provenance.integrity_hash is required and must be a non-empty string',
      });
    }
    validateOptionalString(provenance.parent_event_id, 'provenance.parent_event_id', errors);
    validateStringList(provenance.source_refs, 'provenance.source_refs', errors);
  }

  // Validate redaction (optional, must be object)
  if (data.redaction !== undefined) {
    if (
      typeof data.redaction !== 'object' ||
      data.redaction === null ||
      Array.isArray(data.redaction)
    ) {
      errors.push({ field: 'redaction', message: 'redaction must be an object' });
    } else {
      const redaction = data.redaction as Record<string, unknown>;
      validateStringList(redaction.redacted_fields, 'redaction.redacted_fields', errors);
      validateOptionalString(redaction.policy_version, 'redaction.policy_version', errors);
    }
  }

  // Return errors if any
  if (errors.length > 0) {
    return err(errors);
  }

  // Construct validated event with defaults
  const event: IntentEvent = {
    schema_version: INTENT_EVENT_SCHEMA_VERSION,
    event_id: (data.event_id as string) ?? randomUUID(),
    occurred_at: (data.occurred_at as string) ?? new Date().toISOString(),
    sequence: data.sequence as number,
    event_type: data.event_type as IntentEvent['event_type'],
    actor: data.actor as IntentActor,
    context: data.context as IntentContext,
    intent: data.intent as IntentPayload,
    provenance: data.provenance as IntentProvenance,
    redaction: (data.redaction as IntentRedaction) ?? {},
  };

  return ok(event);
}
