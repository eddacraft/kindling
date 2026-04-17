/**
 * Pin validation
 */

import { randomUUID } from 'crypto';
import type { Pin, ValidationError, Result } from '../types/index.js';
import { ok, err, isPinTargetType } from '../types/index.js';

/**
 * Validate and normalize a pin input
 *
 * Auto-generates:
 * - id (if not provided)
 * - createdAt (if not provided)
 *
 * @param input - Pin input to validate
 * @returns Result containing validated Pin or validation errors
 */
export function validatePin(input: unknown): Result<Pin, ValidationError[]> {
  const errors: ValidationError[] = [];

  // Type check
  if (typeof input !== 'object' || input === null) {
    return err([{ field: 'input', message: 'Input must be an object' }]);
  }

  const data = input as Record<string, unknown>;

  // Validate targetType (required)
  if (!data.targetType) {
    errors.push({ field: 'targetType', message: 'targetType is required' });
  } else if (!isPinTargetType(data.targetType)) {
    errors.push({
      field: 'targetType',
      message: `Invalid pin target type: ${data.targetType}`,
      value: data.targetType,
    });
  }

  // Validate targetId (required, non-empty string)
  if (!data.targetId) {
    errors.push({ field: 'targetId', message: 'targetId is required' });
  } else if (typeof data.targetId !== 'string') {
    errors.push({
      field: 'targetId',
      message: 'targetId must be a string',
      value: typeof data.targetId,
    });
  } else if (data.targetId.trim().length === 0) {
    errors.push({ field: 'targetId', message: 'targetId cannot be empty' });
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

  // Validate reason (optional, must be string)
  if (data.reason !== undefined && typeof data.reason !== 'string') {
    errors.push({
      field: 'reason',
      message: 'reason must be a string',
      value: typeof data.reason,
    });
  }

  // Validate createdAt (optional, must be positive number)
  if (data.createdAt !== undefined) {
    if (typeof data.createdAt !== 'number') {
      errors.push({
        field: 'createdAt',
        message: 'createdAt must be a number',
        value: typeof data.createdAt,
      });
    } else if (data.createdAt < 0) {
      errors.push({
        field: 'createdAt',
        message: 'createdAt must be non-negative',
        value: data.createdAt,
      });
    }
  }

  // Validate expiresAt (optional, must be positive number)
  if (data.expiresAt !== undefined) {
    if (typeof data.expiresAt !== 'number') {
      errors.push({
        field: 'expiresAt',
        message: 'expiresAt must be a number',
        value: typeof data.expiresAt,
      });
    } else if (data.expiresAt < 0) {
      errors.push({
        field: 'expiresAt',
        message: 'expiresAt must be non-negative',
        value: data.expiresAt,
      });
    }
  }

  // Return errors if any
  if (errors.length > 0) {
    return err(errors);
  }

  // Construct validated pin with defaults
  const pin: Pin = {
    id: (data.id as string) || randomUUID(),
    targetType: data.targetType as Pin['targetType'],
    targetId: data.targetId as string,
    reason: data.reason as string | undefined,
    createdAt: (data.createdAt as number) || Date.now(),
    expiresAt: data.expiresAt as number | undefined,
    scopeIds: data.scopeIds as Pin['scopeIds'],
  };

  return ok(pin);
}
