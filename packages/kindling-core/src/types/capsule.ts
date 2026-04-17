/**
 * Capsule types and definitions
 *
 * A Capsule is a bounded unit of meaning that groups related observations
 * (e.g., a session, a workflow node)
 */

import type { ID, Timestamp, ScopeIds } from './common.js';

/**
 * Types of capsules
 */
export type CapsuleType =
  | 'session' // OpenCode session
  | 'pocketflow_node'; // PocketFlow workflow node

/**
 * Capsule lifecycle status
 */
export type CapsuleStatus =
  | 'open' // Accepting observations
  | 'closed'; // Finalized

/**
 * Capsule entity
 *
 * Immutable except for:
 * - status (transitions from 'open' to 'closed')
 * - closedAt (set when status changes to 'closed')
 */
export interface Capsule {
  /** Unique identifier (UUIDv4) */
  id: ID;

  /** Type of capsule */
  type: CapsuleType;

  /**
   * Human-readable description of capsule purpose
   * e.g., "Fix authentication bug"
   */
  intent: string;

  /** Lifecycle state */
  status: CapsuleStatus;

  /** Timestamp when capsule was opened (epoch milliseconds) */
  openedAt: Timestamp;

  /** Timestamp when capsule was closed (epoch milliseconds, undefined if open) */
  closedAt?: Timestamp;

  /** Isolation dimensions for scoped queries */
  scopeIds: ScopeIds;

  /**
   * Ordered list of observation IDs attached to this capsule
   * Order is deterministic (insertion order)
   */
  observationIds: ID[];

  /** Optional reference to summary for this capsule */
  summaryId?: ID;
}

/**
 * Input for creating a new capsule
 * Makes id, openedAt, status, observationIds, and summaryId optional
 */
export interface CapsuleInput {
  id?: ID;
  type: CapsuleType;
  intent: string;
  status?: CapsuleStatus;
  openedAt?: Timestamp;
  closedAt?: Timestamp;
  scopeIds: ScopeIds;
  observationIds?: ID[];
  summaryId?: ID;
}

/**
 * All valid capsule types
 */
export const CAPSULE_TYPES: readonly CapsuleType[] = ['session', 'pocketflow_node'] as const;

/**
 * All valid capsule statuses
 */
export const CAPSULE_STATUSES: readonly CapsuleStatus[] = ['open', 'closed'] as const;

/**
 * Type guard to check if a string is a valid CapsuleType
 */
export function isCapsuleType(value: unknown): value is CapsuleType {
  return typeof value === 'string' && CAPSULE_TYPES.includes(value as CapsuleType);
}

/**
 * Type guard to check if a string is a valid CapsuleStatus
 */
export function isCapsuleStatus(value: unknown): value is CapsuleStatus {
  return typeof value === 'string' && CAPSULE_STATUSES.includes(value as CapsuleStatus);
}
