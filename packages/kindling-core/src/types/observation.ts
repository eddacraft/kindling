/**
 * Observation types and definitions
 *
 * An Observation is an atomic, immutable record of an event that occurred
 * during development (tool call, command, diff, error, message, etc.)
 */

import type { ID, Timestamp, ScopeIds } from './common.js';

/**
 * Types of observations that can be captured
 */
export type ObservationKind =
  | 'tool_call' // Tool invocation (e.g., grep, read file)
  | 'command' // Shell command execution
  | 'file_diff' // File change
  | 'error' // Error or exception
  | 'message' // User or agent message
  | 'node_start' // Workflow node started
  | 'node_end' // Workflow node ended
  | 'node_output' // Workflow node output
  | 'node_error'; // Workflow node error

/**
 * Observation entity
 *
 * Immutable except for the `redacted` flag (via explicit redaction API)
 */
export interface Observation {
  /** Unique identifier (UUIDv4) */
  id: ID;

  /** Type of observation */
  kind: ObservationKind;

  /** The actual content (text, JSON, etc.) */
  content: string;

  /**
   * Source-specific metadata (e.g., toolName, exitCode, nodeId)
   * Stored as JSON blob in persistence layer
   */
  provenance: Record<string, unknown>;

  /** Timestamp when observation was created (epoch milliseconds) */
  ts: Timestamp;

  /** Isolation dimensions for scoped queries */
  scopeIds: ScopeIds;

  /**
   * Privacy flag
   * If true, content is '[redacted]' and observation is excluded from FTS
   */
  redacted: boolean;
}

/**
 * Input for creating a new observation
 * Makes id, ts, and redacted optional (will be auto-generated)
 */
export interface ObservationInput {
  id?: ID;
  kind: ObservationKind;
  content: string;
  provenance?: Record<string, unknown>;
  ts?: Timestamp;
  scopeIds: ScopeIds;
  redacted?: boolean;
}

/**
 * All valid observation kinds
 */
export const OBSERVATION_KINDS: readonly ObservationKind[] = [
  'tool_call',
  'command',
  'file_diff',
  'error',
  'message',
  'node_start',
  'node_end',
  'node_output',
  'node_error',
] as const;

/**
 * Type guard to check if a string is a valid ObservationKind
 */
export function isObservationKind(value: unknown): value is ObservationKind {
  return typeof value === 'string' && OBSERVATION_KINDS.includes(value as ObservationKind);
}
