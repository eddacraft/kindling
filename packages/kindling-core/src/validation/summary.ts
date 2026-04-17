/**
 * Summary validation
 */

import { randomUUID } from 'crypto';
import type { Summary, ValidationError, Result } from '../types/index.js';
import { ok, err, isValidConfidence } from '../types/index.js';

/**
 * Validate and normalize a summary input
 *
 * Auto-generates:
 * - id (if not provided)
 * - createdAt (if not provided)
 *
 * @param input - Summary input to validate
 * @returns Result containing validated Summary or validation errors
 */
export function validateSummary(input: unknown): Result<Summary, ValidationError[]> {
  const errors: ValidationError[] = [];

  // Type check
  if (typeof input !== 'object' || input === null) {
    return err([{ field: 'input', message: 'Input must be an object' }]);
  }

  const data = input as Record<string, unknown>;

  // Validate capsuleId (required, non-empty string)
  if (!data.capsuleId) {
    errors.push({ field: 'capsuleId', message: 'capsuleId is required' });
  } else if (typeof data.capsuleId !== 'string') {
    errors.push({
      field: 'capsuleId',
      message: 'capsuleId must be a string',
      value: typeof data.capsuleId,
    });
  } else if (data.capsuleId.trim().length === 0) {
    errors.push({ field: 'capsuleId', message: 'capsuleId cannot be empty' });
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

  // Validate confidence (required, must be 0.0-1.0)
  if (data.confidence === undefined || data.confidence === null) {
    errors.push({ field: 'confidence', message: 'confidence is required' });
  } else if (typeof data.confidence !== 'number') {
    errors.push({
      field: 'confidence',
      message: 'confidence must be a number',
      value: typeof data.confidence,
    });
  } else if (!isValidConfidence(data.confidence)) {
    errors.push({
      field: 'confidence',
      message: 'confidence must be between 0.0 and 1.0',
      value: data.confidence,
    });
  }

  // Validate evidenceRefs (required, must be array of strings)
  if (!data.evidenceRefs) {
    errors.push({ field: 'evidenceRefs', message: 'evidenceRefs is required' });
  } else if (!Array.isArray(data.evidenceRefs)) {
    errors.push({
      field: 'evidenceRefs',
      message: 'evidenceRefs must be an array',
    });
  } else if (!data.evidenceRefs.every((id) => typeof id === 'string')) {
    errors.push({
      field: 'evidenceRefs',
      message: 'evidenceRefs must contain only strings',
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

  // Return errors if any
  if (errors.length > 0) {
    return err(errors);
  }

  // Construct validated summary with defaults
  const summary: Summary = {
    id: (data.id as string) || randomUUID(),
    capsuleId: data.capsuleId as string,
    content: data.content as string,
    confidence: data.confidence as number,
    createdAt: (data.createdAt as number) || Date.now(),
    evidenceRefs: data.evidenceRefs as string[],
  };

  return ok(summary);
}
