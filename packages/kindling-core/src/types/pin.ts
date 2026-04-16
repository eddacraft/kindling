/**
 * Pin types and definitions
 *
 * A Pin marks an observation or summary as important
 * (user-curated for retrieval prioritization)
 */

import type { ID, Timestamp, ScopeIds } from './common.js';

/**
 * Type of entity that can be pinned
 */
export type PinTargetType = 'observation' | 'summary';

/**
 * Pin entity
 *
 * Immutable - delete to remove
 */
export interface Pin {
  /** Unique identifier (UUIDv4) */
  id: ID;

  /** Type of entity being pinned */
  targetType: PinTargetType;

  /** ID of the pinned entity (observation or summary) */
  targetId: ID;

  /**
   * Optional user-provided explanation for why this is pinned
   * e.g., "Critical context for auth flow"
   */
  reason?: string;

  /** Timestamp when pin was created (epoch milliseconds) */
  createdAt: Timestamp;

  /**
   * Optional expiration timestamp (epoch milliseconds)
   * If set, pin is only active while expiresAt > now
   * Supports time-bound pins (e.g., session-only)
   */
  expiresAt?: Timestamp;

  /** Isolation dimensions for scoped queries */
  scopeIds: ScopeIds;
}

/**
 * Input for creating a new pin
 * Makes id and createdAt optional (will be auto-generated)
 */
export interface PinInput {
  id?: ID;
  targetType: PinTargetType;
  targetId: ID;
  reason?: string;
  createdAt?: Timestamp;
  expiresAt?: Timestamp;
  scopeIds: ScopeIds;
}

/**
 * All valid pin target types
 */
export const PIN_TARGET_TYPES: readonly PinTargetType[] = ['observation', 'summary'] as const;

/**
 * Type guard to check if a string is a valid PinTargetType
 */
export function isPinTargetType(value: unknown): value is PinTargetType {
  return typeof value === 'string' && PIN_TARGET_TYPES.includes(value as PinTargetType);
}

/**
 * Check if a pin is active (not expired) at a given timestamp
 */
export function isPinActive(pin: Pin, now: Timestamp): boolean {
  return pin.expiresAt === undefined || pin.expiresAt > now;
}
