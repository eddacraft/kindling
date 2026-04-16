/**
 * Common types used across Kindling packages
 */

/**
 * Unique identifier for entities
 * Implementation uses UUIDv4 format
 */
export type ID = string;

/**
 * Timestamp in epoch milliseconds
 */
export type Timestamp = number;

/**
 * Scope identifiers for multi-dimensional isolation
 *
 * All fields are optional to support partial scoping:
 * - sessionId: Isolate by session (e.g., OpenCode session)
 * - repoId: Isolate by repository path
 * - agentId: Isolate by agent (future)
 * - userId: Isolate by user (future)
 * - taskId: Link to external task system (e.g., Beads task ID)
 */
export interface ScopeIds {
  sessionId?: string;
  repoId?: string;
  agentId?: string;
  userId?: string;
  taskId?: string;
}

/**
 * Result type for operations that can fail
 */
export type Result<T, E = Error> = { ok: true; value: T } | { ok: false; error: E };

/**
 * Validation error details
 */
export interface ValidationError {
  field: string;
  message: string;
  value?: unknown;
}

/**
 * Helper to create a success result
 */
export function ok<T>(value: T): Result<T, never> {
  return { ok: true, value };
}

/**
 * Helper to create an error result
 */
export function err<E>(error: E): Result<never, E> {
  return { ok: false, error };
}
