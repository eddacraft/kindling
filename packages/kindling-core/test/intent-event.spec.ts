/**
 * Tests for the canonical IntentEvent schema (v1) and validation
 *
 * Contract reference: plans/modules/04-intent-capture-events.aps.md
 */

import { describe, it, expect } from 'vitest';
import {
  validateIntentEvent,
  isIntentEventType,
  isIntentActorKind,
  INTENT_EVENT_TYPES,
  INTENT_ACTOR_KINDS,
  INTENT_EVENT_SCHEMA_VERSION,
  type IntentEventInput,
} from '../src/index.js';

/** A minimal valid input covering all required fields */
function validInput(): IntentEventInput {
  return {
    event_type: 'intent.prompt_submitted',
    sequence: 0,
    actor: { kind: 'agent', tool: 'claude-code', model: 'claude-sonnet-4-6' },
    context: { workspace_id: 'ws-1', repo: 'eddacraft/kindling' },
    intent: { objective: 'Finalize the IntentEvent schema' },
    provenance: { integrity_hash: 'a'.repeat(64) },
  };
}

describe('IntentEvent schema', () => {
  describe('constants and guards', () => {
    it('should pin schema version at 1.0', () => {
      expect(INTENT_EVENT_SCHEMA_VERSION).toBe('1.0');
    });

    it('should enumerate all v1 event types', () => {
      expect(INTENT_EVENT_TYPES).toEqual([
        'intent.session_started',
        'intent.prompt_submitted',
        'intent.constraints_updated',
        'intent.task_reframed',
        'intent.checkpoint_created',
      ]);
    });

    it('should enumerate actor kinds', () => {
      expect(INTENT_ACTOR_KINDS).toEqual(['human', 'agent']);
    });

    it('should guard event types', () => {
      expect(isIntentEventType('intent.session_started')).toBe(true);
      expect(isIntentEventType('intent.unknown')).toBe(false);
      expect(isIntentEventType(42)).toBe(false);
    });

    it('should guard actor kinds', () => {
      expect(isIntentActorKind('human')).toBe(true);
      expect(isIntentActorKind('agent')).toBe(true);
      expect(isIntentActorKind('robot')).toBe(false);
      expect(isIntentActorKind(undefined)).toBe(false);
    });
  });

  describe('validation', () => {
    it('should validate a minimal valid event and apply defaults', () => {
      const result = validateIntentEvent(validInput());
      expect(result.ok).toBe(true);
      if (result.ok) {
        expect(result.value.schema_version).toBe('1.0');
        expect(result.value.event_id).toMatch(
          /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/,
        );
        expect(Date.parse(result.value.occurred_at)).not.toBeNaN();
        expect(result.value.redaction).toEqual({});
      }
    });

    it('should preserve provided event_id and occurred_at', () => {
      const input = {
        ...validInput(),
        event_id: '01234567-89ab-4cde-8f01-23456789abcd',
        occurred_at: '2026-06-11T10:00:00.000Z',
      };

      const result = validateIntentEvent(input);
      expect(result.ok).toBe(true);
      if (result.ok) {
        expect(result.value.event_id).toBe('01234567-89ab-4cde-8f01-23456789abcd');
        expect(result.value.occurred_at).toBe('2026-06-11T10:00:00.000Z');
      }
    });

    it('should preserve full intent, actor, and redaction detail', () => {
      const input: IntentEventInput = {
        ...validInput(),
        event_type: 'intent.constraints_updated',
        actor: { kind: 'human', id: 'aneki' },
        context: {
          workspace_id: 'ws-1',
          repo: 'eddacraft/kindling',
          branch: 'feat/intent-event-schema',
          commit: '2c94612',
          session_id: 'sess-1',
          thread_id: 'thread-1',
        },
        intent: {
          objective: 'Ship intent capture',
          constraints: ['no breaking changes'],
          success_criteria: ['tests pass'],
          scope_in: ['kindling-core'],
          scope_out: ['anvil policy'],
        },
        provenance: {
          parent_event_id: '01234567-89ab-4cde-8f01-23456789abcd',
          source_refs: ['plans/modules/04-intent-capture-events.aps.md'],
          integrity_hash: 'b'.repeat(64),
        },
        redaction: { redacted_fields: ['intent.constraints[0]'], policy_version: 'r1' },
      };

      const result = validateIntentEvent(input);
      expect(result.ok).toBe(true);
      if (result.ok) {
        expect(result.value.context.branch).toBe('feat/intent-event-schema');
        expect(result.value.intent.constraints).toEqual(['no breaking changes']);
        expect(result.value.provenance.parent_event_id).toBe(
          '01234567-89ab-4cde-8f01-23456789abcd',
        );
        expect(result.value.redaction.redacted_fields).toEqual(['intent.constraints[0]']);
      }
    });

    it('should accept an explicit schema_version matching the current contract', () => {
      const result = validateIntentEvent({ ...validInput(), schema_version: '1.0' });
      expect(result.ok).toBe(true);
    });

    it('should accept sequence 0 as the first valid sequence number', () => {
      const result = validateIntentEvent({ ...validInput(), sequence: 0 });
      expect(result.ok).toBe(true);
      if (result.ok) {
        expect(result.value.sequence).toBe(0);
      }
    });

    it('should reject missing actor', () => {
      const { actor: _dropped, ...input } = validInput();
      const result = validateIntentEvent(input);
      expect(result.ok).toBe(false);
      if (!result.ok) {
        expect(result.error).toEqual(
          expect.arrayContaining([
            expect.objectContaining({
              field: 'actor',
              message: 'actor is required and must be an object',
            }),
          ]),
        );
      }
    });

    it('should reject array values for envelope sub-objects', () => {
      const result = validateIntentEvent({ ...validInput(), actor: [], context: [] });
      expect(result.ok).toBe(false);
      if (!result.ok) {
        expect(result.error).toEqual(
          expect.arrayContaining([
            expect.objectContaining({ field: 'actor' }),
            expect.objectContaining({ field: 'context' }),
          ]),
        );
      }
    });

    it('should reject a bare-year occurred_at even though Date.parse accepts it', () => {
      const result = validateIntentEvent({ ...validInput(), occurred_at: '2025' });
      expect(result.ok).toBe(false);
      if (!result.ok) {
        expect(result.error).toEqual(
          expect.arrayContaining([expect.objectContaining({ field: 'occurred_at' })]),
        );
      }
    });

    it('should reject an unknown schema_version', () => {
      const result = validateIntentEvent({ ...validInput(), schema_version: '2.0' });
      expect(result.ok).toBe(false);
      if (!result.ok) {
        expect(result.error).toEqual(
          expect.arrayContaining([expect.objectContaining({ field: 'schema_version' })]),
        );
      }
    });

    it('should reject missing event_type', () => {
      const { event_type: _dropped, ...input } = validInput();
      const result = validateIntentEvent(input);
      expect(result.ok).toBe(false);
      if (!result.ok) {
        expect(result.error).toEqual(
          expect.arrayContaining([
            expect.objectContaining({ field: 'event_type', message: 'event_type is required' }),
          ]),
        );
      }
    });

    it('should reject invalid event_type', () => {
      const result = validateIntentEvent({ ...validInput(), event_type: 'intent.unknown' });
      expect(result.ok).toBe(false);
      if (!result.ok) {
        expect(result.error).toEqual(
          expect.arrayContaining([
            expect.objectContaining({
              field: 'event_type',
              message: expect.stringContaining('Invalid intent event type'),
            }),
          ]),
        );
      }
    });

    it('should reject invalid actor kind', () => {
      const result = validateIntentEvent({
        ...validInput(),
        actor: { kind: 'robot' },
      });
      expect(result.ok).toBe(false);
      if (!result.ok) {
        expect(result.error).toEqual(
          expect.arrayContaining([expect.objectContaining({ field: 'actor.kind' })]),
        );
      }
    });

    it('should reject missing context.workspace_id and context.repo', () => {
      const result = validateIntentEvent({ ...validInput(), context: {} });
      expect(result.ok).toBe(false);
      if (!result.ok) {
        expect(result.error).toEqual(
          expect.arrayContaining([
            expect.objectContaining({ field: 'context.workspace_id' }),
            expect.objectContaining({ field: 'context.repo' }),
          ]),
        );
      }
    });

    it('should reject empty intent.objective', () => {
      const result = validateIntentEvent({
        ...validInput(),
        intent: { objective: '   ' },
      });
      expect(result.ok).toBe(false);
      if (!result.ok) {
        expect(result.error).toEqual(
          expect.arrayContaining([expect.objectContaining({ field: 'intent.objective' })]),
        );
      }
    });

    it('should reject non-string entries in intent list fields', () => {
      const result = validateIntentEvent({
        ...validInput(),
        intent: { objective: 'x', constraints: ['ok', 42] },
      });
      expect(result.ok).toBe(false);
      if (!result.ok) {
        expect(result.error).toEqual(
          expect.arrayContaining([expect.objectContaining({ field: 'intent.constraints' })]),
        );
      }
    });

    it('should reject negative or non-integer sequence', () => {
      const negative = validateIntentEvent({ ...validInput(), sequence: -1 });
      expect(negative.ok).toBe(false);

      const fractional = validateIntentEvent({ ...validInput(), sequence: 1.5 });
      expect(fractional.ok).toBe(false);
      if (!fractional.ok) {
        expect(fractional.error).toEqual(
          expect.arrayContaining([expect.objectContaining({ field: 'sequence' })]),
        );
      }
    });

    it('should reject missing provenance.integrity_hash', () => {
      const result = validateIntentEvent({ ...validInput(), provenance: {} });
      expect(result.ok).toBe(false);
      if (!result.ok) {
        expect(result.error).toEqual(
          expect.arrayContaining([expect.objectContaining({ field: 'provenance.integrity_hash' })]),
        );
      }
    });

    it('should reject an occurred_at that does not parse as a date', () => {
      const result = validateIntentEvent({ ...validInput(), occurred_at: 'not-a-date' });
      expect(result.ok).toBe(false);
      if (!result.ok) {
        expect(result.error).toEqual(
          expect.arrayContaining([expect.objectContaining({ field: 'occurred_at' })]),
        );
      }
    });

    it('should reject non-object input', () => {
      const result = validateIntentEvent('nope');
      expect(result.ok).toBe(false);
      if (!result.ok) {
        expect(result.error).toEqual([
          expect.objectContaining({ field: 'input', message: 'Input must be an object' }),
        ]);
      }
    });

    it('should aggregate errors across fields', () => {
      const result = validateIntentEvent({
        event_type: 'bogus',
        sequence: -2,
        actor: {},
        context: {},
        intent: {},
        provenance: {},
      });
      expect(result.ok).toBe(false);
      if (!result.ok) {
        const fields = result.error.map((e) => e.field);
        expect(fields).toEqual(
          expect.arrayContaining([
            'event_type',
            'sequence',
            'actor.kind',
            'context.workspace_id',
            'context.repo',
            'intent.objective',
            'provenance.integrity_hash',
          ]),
        );
      }
    });
  });
});
