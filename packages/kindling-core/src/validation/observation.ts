/**
 * Observation validation
 */

import { randomUUID } from 'crypto';
import type { Observation, ValidationError, Result } from '../types/index.js';
import { ok, err, isObservationKind } from '../types/index.js';

/**
 * Validate and normalize an observation input
 *
 * Auto-generates:
 * - id (if not provided)
 * - ts (if not provided)
 * - redacted (defaults to false)
 * - provenance (defaults to empty object)
 *
 * @param input - Observation input to validate
 * @returns Result containing validated Observation or validation errors
 */
export function validateObservation(input: unknown): Result<Observation, ValidationError[]> {
  const errors: ValidationError[] = [];

  // Type check
  if (typeof input !== 'object' || input === null) {
    return err([{ field: 'input', message: 'Input must be an object' }]);
  }

  const data = input as Record<string, unknown>;

  // Validate kind (required)
  if (!data.kind) {
    errors.push({ field: 'kind', message: 'kind is required' });
  } else if (!isObservationKind(data.kind)) {
    errors.push({
      field: 'kind',
      message: `Invalid observation kind: ${data.kind}`,
      value: data.kind,
    });
  }

  // Validate content (required, non-empty string)
  if (!data.content) {
    errors.push({ field: 'content', message: 'content is required' });
  } else if (typeof data.content !== 'string') {
    errors.push({
      field: 'content',
      message: 'content must be a string',
      value: typeof data.content,
    });
  } else if (data.content.trim().length === 0) {
    errors.push({ field: 'content', message: 'content cannot be empty' });
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

  // Validate provenance (optional, must be object)
  if (data.provenance !== undefined) {
    if (
      typeof data.provenance !== 'object' ||
      data.provenance === null ||
      Array.isArray(data.provenance)
    ) {
      errors.push({
        field: 'provenance',
        message: 'provenance must be an object',
      });
    }
  }

  // Validate ts (optional, must be positive number)
  if (data.ts !== undefined) {
    if (typeof data.ts !== 'number') {
      errors.push({
        field: 'ts',
        message: 'ts must be a number',
        value: typeof data.ts,
      });
    } else if (data.ts < 0) {
      errors.push({
        field: 'ts',
        message: 'ts must be non-negative',
        value: data.ts,
      });
    }
  }

  // Validate redacted (optional, must be boolean)
  if (data.redacted !== undefined && typeof data.redacted !== 'boolean') {
    errors.push({
      field: 'redacted',
      message: 'redacted must be a boolean',
      value: typeof data.redacted,
    });
  }

  // Return errors if any
  if (errors.length > 0) {
    return err(errors);
  }

  // Construct validated observation with defaults
  const observation: Observation = {
    id: (data.id as string) || randomUUID(),
    kind: data.kind as Observation['kind'],
    content: data.content as string,
    provenance: (data.provenance as Record<string, unknown>) || {},
    ts: (data.ts as number) || Date.now(),
    scopeIds: data.scopeIds as Observation['scopeIds'],
    redacted: (data.redacted as boolean) || false,
  };

  return ok(observation);
}
