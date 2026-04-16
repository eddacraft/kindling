/**
 * Capsule manager implementation
 *
 * Manages capsule lifecycle with in-memory caching for fast lookup.
 */

import type { ID, ScopeIds } from '../types/common.js';
import type { Capsule } from '../types/capsule.js';
import type {
  CapsuleManager as ICapsuleManager,
  OpenCapsuleOptions,
  CloseCapsuleSignals,
} from './types.js';
import type { CapsuleStore } from './lifecycle.js';
import { openCapsule, closeCapsule, getCapsule, getOpenCapsule } from './lifecycle.js';

/**
 * Default capsule manager implementation
 *
 * Features:
 * - In-memory cache for active capsules
 * - Thread-safe operations (via synchronous store calls)
 * - Automatic cache invalidation on close
 */
export class CapsuleManager implements ICapsuleManager {
  private store: CapsuleStore;
  private activeCache: Map<ID, Capsule>;

  constructor(store: CapsuleStore) {
    this.store = store;
    this.activeCache = new Map();
  }

  /**
   * Open a new capsule
   *
   * @param options - Capsule creation options
   * @returns The created capsule
   * @throws Error if validation fails or duplicate open capsule exists
   */
  open(options: OpenCapsuleOptions): Capsule {
    const capsule = openCapsule(this.store, options);

    // Cache for fast lookup
    this.activeCache.set(capsule.id, capsule);

    return capsule;
  }

  /**
   * Close an open capsule
   *
   * @param capsuleId - ID of capsule to close
   * @param signals - Closure signals/metadata
   * @returns The closed capsule
   * @throws Error if capsule not found or already closed
   */
  close(capsuleId: ID, signals?: CloseCapsuleSignals): Capsule {
    const capsule = closeCapsule(this.store, capsuleId, signals);

    // Remove from active cache
    this.activeCache.delete(capsuleId);

    return capsule;
  }

  /**
   * Get a capsule by ID
   *
   * Checks cache first, falls back to store.
   *
   * @param capsuleId - Capsule ID to lookup
   * @returns Capsule or undefined if not found
   */
  get(capsuleId: ID): Capsule | undefined {
    // Check cache first
    const cached = this.activeCache.get(capsuleId);
    if (cached) {
      return cached;
    }

    // Fall back to store
    return getCapsule(this.store, capsuleId);
  }

  /**
   * Get the open capsule for a scope (if any)
   *
   * Currently only supports session-scoped lookup.
   *
   * @param scopeIds - Partial scope to match
   * @returns Open capsule or undefined
   */
  getOpen(scopeIds: Partial<ScopeIds>): Capsule | undefined {
    // Only session-scoped lookup is supported for now
    if (!scopeIds.sessionId) {
      throw new Error('getOpen currently only supports sessionId lookup');
    }

    return getOpenCapsule(this.store, scopeIds.sessionId);
  }

  /**
   * Notify that an observation was attached to a capsule
   *
   * Updates the cached capsule's observationIds if the capsule is in the cache.
   *
   * @param capsuleId - Capsule that received the observation
   * @param observationId - Observation that was attached
   */
  notifyObservationAttached(capsuleId: ID, observationId: ID): void {
    const cached = this.activeCache.get(capsuleId);
    if (cached) {
      // Update the cached capsule's observationIds
      cached.observationIds.push(observationId);
    }
  }

  /**
   * Clear the active capsule cache
   *
   * Useful for testing or manual cache invalidation.
   */
  clearCache(): void {
    this.activeCache.clear();
  }

  /**
   * Get count of cached active capsules
   *
   * Useful for debugging and monitoring.
   */
  getCacheSize(): number {
    return this.activeCache.size;
  }
}
