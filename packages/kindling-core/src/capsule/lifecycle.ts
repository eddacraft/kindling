/**
 * Capsule lifecycle functions
 *
 * Core logic for opening, closing, and managing capsules.
 */

import type { ID } from '../types/common.js';
import type { Capsule, CapsuleStatus } from '../types/capsule.js';
import type { Summary } from '../types/summary.js';
import type { OpenCapsuleOptions, CloseCapsuleSignals } from './types.js';
import { validateCapsule } from '../validation/capsule.js';
import { validateSummary } from '../validation/summary.js';

/**
 * Store interface for capsule lifecycle operations
 *
 * Minimal interface required by lifecycle functions
 */
export interface CapsuleStore {
  createCapsule(capsule: Capsule): void;
  closeCapsule(capsuleId: ID, closedAt: number): void;
  getCapsuleById(capsuleId: ID): Capsule | undefined;
  getOpenCapsuleForSession(sessionId: string): Capsule | undefined;
  insertSummary(summary: Summary): void;
}

/**
 * Open a new capsule
 *
 * Creates and persists a new capsule with status=open.
 * For session-type capsules, validates that no other capsule is open for the same session.
 *
 * @param store - Store implementation
 * @param options - Capsule creation options
 * @returns The created capsule
 * @throws Error if validation fails or duplicate open capsule exists
 */
export function openCapsule(store: CapsuleStore, options: OpenCapsuleOptions): Capsule {
  const { type, intent, scopeIds, id } = options;

  // Check for duplicate open capsule (session-scoped)
  if (type === 'session' && scopeIds.sessionId) {
    const existingCapsule = store.getOpenCapsuleForSession(scopeIds.sessionId);
    if (existingCapsule) {
      throw new Error(
        `Cannot open capsule: session ${scopeIds.sessionId} already has an open capsule (${existingCapsule.id})`,
      );
    }
  }

  // Validate and create capsule
  const capsuleResult = validateCapsule({
    id,
    type,
    intent,
    status: 'open',
    scopeIds,
  });

  if (!capsuleResult.ok) {
    throw new Error(
      `Capsule validation failed: ${capsuleResult.error.map((e) => e.message).join(', ')}`,
    );
  }

  const capsule = capsuleResult.value;

  // Persist to store
  store.createCapsule(capsule);

  return capsule;
}

/**
 * Close an open capsule
 *
 * Updates capsule status to closed and sets closedAt timestamp.
 * Optionally creates a summary if content is provided.
 *
 * @param store - Store implementation
 * @param capsuleId - ID of capsule to close
 * @param signals - Closure signals/metadata
 * @returns The closed capsule
 * @throws Error if capsule not found or already closed
 */
export function closeCapsule(
  store: CapsuleStore,
  capsuleId: ID,
  signals: CloseCapsuleSignals = {},
): Capsule {
  // Get existing capsule
  const capsule = store.getCapsuleById(capsuleId);

  if (!capsule) {
    throw new Error(`Capsule ${capsuleId} not found`);
  }

  if (capsule.status === 'closed') {
    throw new Error(`Capsule ${capsuleId} is already closed`);
  }

  // Close capsule
  const closedAt = Date.now();
  store.closeCapsule(capsuleId, closedAt);

  // Create summary if content provided
  if (signals.summaryContent) {
    const summaryResult = validateSummary({
      capsuleId,
      content: signals.summaryContent,
      confidence: signals.summaryConfidence ?? 0.8,
      evidenceRefs: signals.evidenceRefs ?? [],
    });

    if (!summaryResult.ok) {
      throw new Error(
        `Summary validation failed: ${summaryResult.error.map((e) => e.message).join(', ')}`,
      );
    }

    store.insertSummary(summaryResult.value);
  }

  // Return updated capsule
  return {
    ...capsule,
    status: 'closed' as CapsuleStatus,
    closedAt,
  };
}

/**
 * Get a capsule by ID
 *
 * @param store - Store implementation
 * @param capsuleId - Capsule ID to lookup
 * @returns Capsule or undefined if not found
 */
export function getCapsule(store: CapsuleStore, capsuleId: ID): Capsule | undefined {
  return store.getCapsuleById(capsuleId);
}

/**
 * Get the open capsule for a session (if any)
 *
 * @param store - Store implementation
 * @param sessionId - Session ID to search for
 * @returns Open capsule or undefined
 */
export function getOpenCapsule(store: CapsuleStore, sessionId: string): Capsule | undefined {
  return store.getOpenCapsuleForSession(sessionId);
}
