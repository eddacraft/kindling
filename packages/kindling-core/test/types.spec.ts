/**
 * Tests for domain types and validation
 */

import { describe, it, expect } from 'vitest';
import {
  validateObservation,
  validateCapsule,
  validateSummary,
  validatePin,
  isObservationKind,
  isCapsuleType,
  isCapsuleStatus,
  isPinTargetType,
  isPinActive,
  isValidConfidence,
} from '../src/index.js';

describe('Observation Validation', () => {
  it('should validate a valid observation', () => {
    const input = {
      kind: 'tool_call' as const,
      content: 'grep pattern',
      scopeIds: { sessionId: 's1' },
    };

    const result = validateObservation(input);
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.value.kind).toBe('tool_call');
      expect(result.value.content).toBe('grep pattern');
      expect(result.value.id).toBeDefined();
      expect(result.value.ts).toBeGreaterThan(0);
      expect(result.value.redacted).toBe(false);
      expect(result.value.provenance).toEqual({});
    }
  });

  it('should reject missing kind', () => {
    const input = {
      content: 'test',
      scopeIds: { sessionId: 's1' },
    };

    const result = validateObservation(input);
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toEqual(
        expect.arrayContaining([
          expect.objectContaining({ field: 'kind', message: 'kind is required' }),
        ]),
      );
    }
  });

  it('should reject invalid kind', () => {
    const input = {
      kind: 'invalid_kind',
      content: 'test',
      scopeIds: { sessionId: 's1' },
    };

    const result = validateObservation(input);
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toEqual(
        expect.arrayContaining([
          expect.objectContaining({
            field: 'kind',
            message: expect.stringContaining('Invalid observation kind'),
          }),
        ]),
      );
    }
  });

  it('should reject empty content', () => {
    const input = {
      kind: 'message' as const,
      content: '   ',
      scopeIds: { sessionId: 's1' },
    };

    const result = validateObservation(input);
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toEqual(
        expect.arrayContaining([
          expect.objectContaining({ field: 'content', message: 'content cannot be empty' }),
        ]),
      );
    }
  });

  it('should reject missing scopeIds', () => {
    const input = {
      kind: 'message' as const,
      content: 'test',
    };

    const result = validateObservation(input);
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toEqual(
        expect.arrayContaining([
          expect.objectContaining({ field: 'scopeIds', message: 'scopeIds is required' }),
        ]),
      );
    }
  });

  it('should reject negative timestamp', () => {
    const input = {
      kind: 'message' as const,
      content: 'test',
      scopeIds: { sessionId: 's1' },
      ts: -1,
    };

    const result = validateObservation(input);
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toEqual(
        expect.arrayContaining([
          expect.objectContaining({ field: 'ts', message: 'ts must be non-negative' }),
        ]),
      );
    }
  });
});

describe('Capsule Validation', () => {
  it('should validate a valid capsule', () => {
    const input = {
      type: 'session' as const,
      intent: 'Fix auth bug',
      scopeIds: { sessionId: 's1', repoId: '/repo' },
    };

    const result = validateCapsule(input);
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.value.type).toBe('session');
      expect(result.value.intent).toBe('Fix auth bug');
      expect(result.value.status).toBe('open');
      expect(result.value.id).toBeDefined();
      expect(result.value.openedAt).toBeGreaterThan(0);
      expect(result.value.observationIds).toEqual([]);
    }
  });

  it('should reject missing type', () => {
    const input = {
      intent: 'test',
      scopeIds: { sessionId: 's1' },
    };

    const result = validateCapsule(input);
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toEqual(
        expect.arrayContaining([
          expect.objectContaining({ field: 'type', message: 'type is required' }),
        ]),
      );
    }
  });

  it('should reject invalid type', () => {
    const input = {
      type: 'invalid_type',
      intent: 'test',
      scopeIds: { sessionId: 's1' },
    };

    const result = validateCapsule(input);
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toEqual(
        expect.arrayContaining([
          expect.objectContaining({
            field: 'type',
            message: expect.stringContaining('Invalid capsule type'),
          }),
        ]),
      );
    }
  });

  it('should reject empty intent', () => {
    const input = {
      type: 'session' as const,
      intent: '   ',
      scopeIds: { sessionId: 's1' },
    };

    const result = validateCapsule(input);
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toEqual(
        expect.arrayContaining([
          expect.objectContaining({ field: 'intent', message: 'intent cannot be empty' }),
        ]),
      );
    }
  });
});

describe('Summary Validation', () => {
  it('should validate a valid summary', () => {
    const input = {
      capsuleId: 'cap1',
      content: 'Fixed authentication bug',
      confidence: 0.9,
      evidenceRefs: ['obs1', 'obs2'],
    };

    const result = validateSummary(input);
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.value.capsuleId).toBe('cap1');
      expect(result.value.content).toBe('Fixed authentication bug');
      expect(result.value.confidence).toBe(0.9);
      expect(result.value.evidenceRefs).toEqual(['obs1', 'obs2']);
      expect(result.value.id).toBeDefined();
      expect(result.value.createdAt).toBeGreaterThan(0);
    }
  });

  it('should reject missing capsuleId', () => {
    const input = {
      content: 'test',
      confidence: 0.5,
      evidenceRefs: [],
    };

    const result = validateSummary(input);
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toEqual(
        expect.arrayContaining([
          expect.objectContaining({ field: 'capsuleId', message: 'capsuleId is required' }),
        ]),
      );
    }
  });

  it('should reject invalid confidence (too high)', () => {
    const input = {
      capsuleId: 'cap1',
      content: 'test',
      confidence: 1.5,
      evidenceRefs: [],
    };

    const result = validateSummary(input);
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toEqual(
        expect.arrayContaining([
          expect.objectContaining({
            field: 'confidence',
            message: 'confidence must be between 0.0 and 1.0',
          }),
        ]),
      );
    }
  });

  it('should reject invalid confidence (negative)', () => {
    const input = {
      capsuleId: 'cap1',
      content: 'test',
      confidence: -0.1,
      evidenceRefs: [],
    };

    const result = validateSummary(input);
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toEqual(
        expect.arrayContaining([
          expect.objectContaining({
            field: 'confidence',
            message: 'confidence must be between 0.0 and 1.0',
          }),
        ]),
      );
    }
  });

  it('should reject missing evidenceRefs', () => {
    const input = {
      capsuleId: 'cap1',
      content: 'test',
      confidence: 0.5,
    };

    const result = validateSummary(input);
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toEqual(
        expect.arrayContaining([
          expect.objectContaining({ field: 'evidenceRefs', message: 'evidenceRefs is required' }),
        ]),
      );
    }
  });
});

describe('Pin Validation', () => {
  it('should validate a valid pin', () => {
    const input = {
      targetType: 'observation' as const,
      targetId: 'obs1',
      reason: 'Important context',
      scopeIds: { sessionId: 's1' },
    };

    const result = validatePin(input);
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.value.targetType).toBe('observation');
      expect(result.value.targetId).toBe('obs1');
      expect(result.value.reason).toBe('Important context');
      expect(result.value.id).toBeDefined();
      expect(result.value.createdAt).toBeGreaterThan(0);
    }
  });

  it('should reject missing targetType', () => {
    const input = {
      targetId: 'obs1',
      scopeIds: { sessionId: 's1' },
    };

    const result = validatePin(input);
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toEqual(
        expect.arrayContaining([
          expect.objectContaining({ field: 'targetType', message: 'targetType is required' }),
        ]),
      );
    }
  });

  it('should reject invalid targetType', () => {
    const input = {
      targetType: 'invalid_type',
      targetId: 'obs1',
      scopeIds: { sessionId: 's1' },
    };

    const result = validatePin(input);
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toEqual(
        expect.arrayContaining([
          expect.objectContaining({
            field: 'targetType',
            message: expect.stringContaining('Invalid pin target type'),
          }),
        ]),
      );
    }
  });
});

describe('Type Guards', () => {
  it('should validate observation kinds', () => {
    expect(isObservationKind('tool_call')).toBe(true);
    expect(isObservationKind('command')).toBe(true);
    expect(isObservationKind('invalid')).toBe(false);
    expect(isObservationKind(123)).toBe(false);
  });

  it('should validate capsule types', () => {
    expect(isCapsuleType('session')).toBe(true);
    expect(isCapsuleType('pocketflow_node')).toBe(true);
    expect(isCapsuleType('invalid')).toBe(false);
  });

  it('should validate capsule statuses', () => {
    expect(isCapsuleStatus('open')).toBe(true);
    expect(isCapsuleStatus('closed')).toBe(true);
    expect(isCapsuleStatus('invalid')).toBe(false);
  });

  it('should validate pin target types', () => {
    expect(isPinTargetType('observation')).toBe(true);
    expect(isPinTargetType('summary')).toBe(true);
    expect(isPinTargetType('invalid')).toBe(false);
  });

  it('should check if pin is active', () => {
    const now = Date.now();
    const futurePin = {
      id: 'pin1',
      targetType: 'observation' as const,
      targetId: 'obs1',
      createdAt: now,
      expiresAt: now + 10000,
      scopeIds: { sessionId: 's1' },
    };
    const expiredPin = {
      id: 'pin2',
      targetType: 'observation' as const,
      targetId: 'obs2',
      createdAt: now - 20000,
      expiresAt: now - 10000,
      scopeIds: { sessionId: 's1' },
    };
    const neverExpiresPin = {
      id: 'pin3',
      targetType: 'observation' as const,
      targetId: 'obs3',
      createdAt: now,
      scopeIds: { sessionId: 's1' },
    };

    expect(isPinActive(futurePin, now)).toBe(true);
    expect(isPinActive(expiredPin, now)).toBe(false);
    expect(isPinActive(neverExpiresPin, now)).toBe(true);
  });

  it('should validate confidence scores', () => {
    expect(isValidConfidence(0.0)).toBe(true);
    expect(isValidConfidence(0.5)).toBe(true);
    expect(isValidConfidence(1.0)).toBe(true);
    expect(isValidConfidence(-0.1)).toBe(false);
    expect(isValidConfidence(1.1)).toBe(false);
    expect(isValidConfidence(NaN)).toBe(false);
  });
});
