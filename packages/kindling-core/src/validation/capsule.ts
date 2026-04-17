/**
 * Capsule validation
 */

import { randomUUID } from 'crypto';
import type { Capsule, ValidationError, Result } from '../types/index.js';
import { ok, err, isCapsuleType, isCapsuleStatus } from '../types/index.js';

/**
 * Validate and normalize a capsule input
 *
 * Auto-generates:
 * - id (if not provided)
 * - openedAt (if not provided)
 * - status (defaults to 'open')
 * - observationIds (defaults to empty array)
 *
 * @param input - Capsule input to validate
 * @returns Result containing validated Capsule or validation errors
 */
export function validateCapsule(input: unknown): Result<Capsule, ValidationError[]> {
  const errors: ValidationError[] = [];

  // Type check
  if (typeof input !== 'object' || input === null) {
    return err([{ field: 'input', message: 'Input must be an object' }]);
  }

  const data = input as Record<string, unknown>;

  // Validate type (required)
  if (!data.type) {
    errors.push({ field: 'type', message: 'type is required' });
  } else if (!isCapsuleType(data.type)) {
    errors.push({
      field: 'type',
      message: `Invalid capsule type: ${data.type}`,
      value: data.type,
    });
  }

  // Validate intent (required, non-empty string)
  if (!data.intent) {
    errors.push({ field: 'intent', message: 'intent is required' });
  } else if (typeof data.intent !== 'string') {
    errors.push({
      field: 'intent',
      message: 'intent must be a string',
      value: typeof data.intent,
    });
  } else if (data.intent.trim().length === 0) {
    errors.push({ field: 'intent', message: 'intent cannot be empty' });
  }

  // Validate scopeIds (required)
  if (!data.scopeIds) {
    errors.push({ field: 'scopeIds', message: 'scopeIds is required' });
  } else if (typeof data.scopeIds !== 'object' || data.scopeIds === null) {
    errors.push({
      field: 'scopeIds',
      message: 'scopeIds must be an object',
    });
  }

  // Validate status (optional)
  if (data.status !== undefined && !isCapsuleStatus(data.status)) {
    errors.push({
      field: 'status',
      message: `Invalid capsule status: ${data.status}`,
      value: data.status,
    });
  }

  // Validate openedAt (optional, must be positive number)
  if (data.openedAt !== undefined) {
    if (typeof data.openedAt !== 'number') {
      errors.push({
        field: 'openedAt',
        message: 'openedAt must be a number',
        value: typeof data.openedAt,
      });
    } else if (data.openedAt < 0) {
      errors.push({
        field: 'openedAt',
        message: 'openedAt must be non-negative',
        value: data.openedAt,
      });
    }
  }

  // Validate closedAt (optional, must be positive number)
  if (data.closedAt !== undefined) {
    if (typeof data.closedAt !== 'number') {
      errors.push({
        field: 'closedAt',
        message: 'closedAt must be a number',
        value: typeof data.closedAt,
      });
    } else if (data.closedAt < 0) {
      errors.push({
        field: 'closedAt',
        message: 'closedAt must be non-negative',
        value: data.closedAt,
      });
    }
  }

  // Validate observationIds (optional, must be array of strings)
  if (data.observationIds !== undefined) {
    if (!Array.isArray(data.observationIds)) {
      errors.push({
        field: 'observationIds',
        message: 'observationIds must be an array',
      });
    } else if (!data.observationIds.every((id) => typeof id === 'string')) {
      errors.push({
        field: 'observationIds',
        message: 'observationIds must contain only strings',
      });
    }
  }

  // Validate summaryId (optional, must be string)
  if (data.summaryId !== undefined && typeof data.summaryId !== 'string') {
    errors.push({
      field: 'summaryId',
      message: 'summaryId must be a string',
      value: typeof data.summaryId,
    });
  }

  // Return errors if any
  if (errors.length > 0) {
    return err(errors);
  }

  // Construct validated capsule with defaults
  const capsule: Capsule = {
    id: (data.id as string) || randomUUID(),
    type: data.type as Capsule['type'],
    intent: data.intent as string,
    status: (data.status as Capsule['status']) || 'open',
    openedAt: (data.openedAt as number) || Date.now(),
    closedAt: data.closedAt as number | undefined,
    scopeIds: data.scopeIds as Capsule['scopeIds'],
    observationIds: (data.observationIds as string[]) || [],
    summaryId: data.summaryId as string | undefined,
  };

  return ok(capsule);
}
