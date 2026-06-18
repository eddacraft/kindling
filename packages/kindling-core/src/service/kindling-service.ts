/**
 * KindlingService - Main orchestration service
 *
 * Provides a unified API that combines:
 * - Capsule lifecycle management
 * - Observation ingestion
 * - Retrieval orchestration
 * - Pin management
 * - Export/import coordination
 */

import type { ID, ScopeIds } from '../types/common.js';
import type { Observation } from '../types/observation.js';
import type { Capsule } from '../types/capsule.js';
import type { Summary } from '../types/summary.js';
import type { Pin } from '../types/pin.js';
import type { RetrieveOptions, RetrieveResult, RetrievalProvider } from '../types/retrieval.js';
import type { OpenCapsuleOptions, CloseCapsuleSignals } from '../capsule/types.js';
import { CapsuleManager } from '../capsule/manager.js';
import { retrieve as retrieveOrchestrator } from '../retrieval/orchestrator.js';
import type { RetrievalStore } from '../retrieval/orchestrator.js';
import { validateObservation } from '../validation/observation.js';
import { validatePin } from '../validation/pin.js';
import { validateSummary } from '../validation/summary.js';
import type { ExportStore } from '../export/bundle.js';
import type { ImportStore } from '../export/restore.js';
import {
  createExportBundle,
  getBundleStats,
  serializeBundle,
  deserializeBundle,
  type ExportBundle,
  type ExportBundleOptions,
  type ExportStats,
} from '../export/bundle.js';
import {
  restoreFromBundle,
  mergeBundles,
  type ImportOptions,
  type ImportResult,
} from '../export/restore.js';

/**
 * Store interface required by KindlingService
 */
export interface KindlingStore extends RetrievalStore, ExportStore, ImportStore {
  // Write operations
  insertObservation(observation: Observation): void;
  createCapsule(capsule: Capsule): void;
  closeCapsule(capsuleId: ID, closedAt?: number): void;
  attachObservationToCapsule(capsuleId: ID, observationId: ID): void;
  createSummary(summary: Summary): void;
  createPin(pin: Pin): void;
  removePin(pinId: ID): void;
  redactObservation(observationId: ID): void;

  // Read operations
  getCapsule(capsuleId: ID): Capsule | undefined;
  getOpenCapsuleForSession(sessionId: string): Capsule | undefined;
  getObservationById(observationId: ID): Observation | undefined;
  getSummaryById(summaryId: ID): Summary | undefined;
  getLatestSummaryForCapsule(capsuleId: ID): Summary | undefined;
  listActivePins(scopeIds?: Partial<Record<string, string>>, now?: number): Pin[];
}

/**
 * Options for appendObservation
 */
export interface AppendObservationOptions {
  capsuleId?: ID;
  validate?: boolean;
}

/**
 * Options for creating a pin
 */
export interface CreatePinOptions {
  targetType: 'observation' | 'summary';
  targetId: ID;
  note?: string;
  ttlMs?: number;
  scopeIds?: Partial<ScopeIds>;
}

/**
 * Options for closing a capsule
 */
export interface CloseCapsuleOptions {
  generateSummary?: boolean;
  summaryContent?: string;
  confidence?: number;
}

/**
 * KindlingService configuration
 */
export interface KindlingServiceConfig {
  store: KindlingStore;
  provider: RetrievalProvider;
}

/**
 * Main kindling service
 *
 * Provides a unified API for all kindling operations.
 */
export class KindlingService {
  private store: KindlingStore;
  private provider: RetrievalProvider;
  private capsuleManager: CapsuleManager;

  constructor(config: KindlingServiceConfig) {
    this.store = config.store;
    this.provider = config.provider;
    this.capsuleManager = new CapsuleManager({
      createCapsule: (capsule: Capsule) => this.store.createCapsule(capsule),
      closeCapsule: (capsuleId: ID, closedAt: number) =>
        this.store.closeCapsule(capsuleId, closedAt),
      getCapsuleById: (capsuleId: ID) => this.store.getCapsule(capsuleId),
      getOpenCapsuleForSession: (sessionId: string) =>
        this.store.getOpenCapsuleForSession(sessionId),
      insertSummary: (summary: Summary) => this.store.createSummary(summary),
    });
  }

  /**
   * Open a new capsule
   *
   * @param options - Capsule creation options
   * @returns The created capsule
   */
  openCapsule(options: OpenCapsuleOptions): Capsule {
    return this.capsuleManager.open(options);
  }

  /**
   * Close a capsule
   *
   * @param capsuleId - ID of capsule to close
   * @param options - Closure options
   * @returns The closed capsule
   */
  closeCapsule(capsuleId: ID, options?: CloseCapsuleOptions): Capsule {
    const signals: CloseCapsuleSignals = {};

    // Generate summary if requested
    if (options?.generateSummary && options.summaryContent) {
      const summary: Summary = {
        id: `sum_${crypto.randomUUID()}`,
        capsuleId,
        content: options.summaryContent,
        confidence: options.confidence ?? 1.0,
        createdAt: Date.now(),
        evidenceRefs: [],
      };

      // Validate and store summary
      const validation = validateSummary(summary);
      if (!validation.ok) {
        const errorMessages = validation.error.map((e) => e.message).join(', ');
        throw new Error(`Invalid summary: ${errorMessages}`);
      }

      this.store.createSummary(validation.value);
    }

    return this.capsuleManager.close(capsuleId, signals);
  }

  /**
   * Append an observation
   *
   * @param observation - Observation to append
   * @param options - Append options
   */
  appendObservation(observation: Observation, options?: AppendObservationOptions): void {
    // Validate observation if requested (default: true)
    let obsToStore = observation;
    if (options?.validate !== false) {
      const validation = validateObservation(observation);
      if (!validation.ok) {
        const errorMessages = validation.error.map((e) => e.message).join(', ');
        throw new Error(`Invalid observation: ${errorMessages}`);
      }
      obsToStore = validation.value;
    }

    // Store observation
    this.store.insertObservation(obsToStore);

    // Attach to capsule if specified
    if (options?.capsuleId) {
      this.store.attachObservationToCapsule(options.capsuleId, obsToStore.id);
      // Update the capsule manager's cache
      this.capsuleManager.notifyObservationAttached(options.capsuleId, obsToStore.id);
    }
  }

  /**
   * Retrieve relevant context
   *
   * @param options - Retrieval options
   * @returns Retrieval result with pins, summary, and candidates
   */
  async retrieve(options: RetrieveOptions): Promise<RetrieveResult> {
    return retrieveOrchestrator(this.store, this.provider, options);
  }

  /**
   * Create a pin
   *
   * @param options - Pin creation options
   * @returns The created pin
   */
  pin(options: CreatePinOptions): Pin {
    const pin: Pin = {
      id: `pin_${crypto.randomUUID()}`,
      targetType: options.targetType,
      targetId: options.targetId,
      reason: options.note,
      createdAt: Date.now(),
      expiresAt: options.ttlMs ? Date.now() + options.ttlMs : undefined,
      scopeIds: options.scopeIds ?? {},
    };

    // Validate pin
    const validation = validatePin(pin);
    if (!validation.ok) {
      const errorMessages = validation.error.map((e) => e.message).join(', ');
      throw new Error(`Invalid pin: ${errorMessages}`);
    }

    this.store.createPin(validation.value);
    return validation.value;
  }

  /**
   * Remove a pin
   *
   * @param pinId - ID of pin to remove
   */
  unpin(pinId: ID): void {
    this.store.removePin(pinId);
  }

  /**
   * Redact an observation
   *
   * Removes content but preserves structure for provenance.
   *
   * @param observationId - ID of observation to redact
   */
  forget(observationId: ID): void {
    this.store.redactObservation(observationId);
  }

  /**
   * Get a capsule by ID
   *
   * @param capsuleId - Capsule ID
   * @returns Capsule or undefined if not found
   */
  getCapsule(capsuleId: ID): Capsule | undefined {
    return this.capsuleManager.get(capsuleId);
  }

  /**
   * Get open capsule for a session
   *
   * @param sessionId - Session ID
   * @returns Open capsule or undefined
   */
  getOpenCapsule(sessionId: string): Capsule | undefined {
    return this.capsuleManager.getOpen({ sessionId });
  }

  /**
   * Get observation by ID
   *
   * @param observationId - Observation ID
   * @returns Observation or undefined if not found
   */
  getObservation(observationId: ID): Observation | undefined {
    return this.store.getObservationById(observationId);
  }

  /**
   * Get summary by ID
   *
   * @param summaryId - Summary ID
   * @returns Summary or undefined if not found
   */
  getSummary(summaryId: ID): Summary | undefined {
    return this.store.getSummaryById(summaryId);
  }

  /**
   * List active pins for a scope
   *
   * @param scopeIds - Scope to filter by
   * @returns Array of active pins
   */
  listPins(scopeIds?: Partial<ScopeIds>): Pin[] {
    return this.store.listActivePins(scopeIds as Partial<Record<string, string>>, Date.now());
  }

  /**
   * Export database to a portable bundle
   *
   * @param options - Export options
   * @returns Export bundle with dataset and metadata
   */
  export(options?: ExportBundleOptions): ExportBundle {
    return createExportBundle(this.store, options);
  }

  /**
   * Export database to JSON string
   *
   * @param options - Export options
   * @param pretty - Use pretty formatting (default: false)
   * @returns JSON string
   */
  exportToJson(options?: ExportBundleOptions, pretty = false): string {
    const bundle = this.export(options);
    return serializeBundle(bundle, pretty);
  }

  /**
   * Import from export bundle
   *
   * @param bundle - Export bundle to import
   * @param options - Import options
   * @returns Import result with counts and errors
   */
  import(bundle: ExportBundle, options?: ImportOptions): ImportResult {
    return restoreFromBundle(this.store, bundle, options);
  }

  /**
   * Import from JSON string
   *
   * @param json - JSON string containing export bundle
   * @param options - Import options
   * @returns Import result with counts and errors
   */
  importFromJson(json: string, options?: ImportOptions): ImportResult {
    const bundle = deserializeBundle(json);
    return this.import(bundle, options);
  }

  /**
   * Get statistics for an export bundle
   *
   * @param bundle - Export bundle
   * @returns Statistics about bundle contents
   */
  getBundleStats(bundle: ExportBundle): ExportStats {
    return getBundleStats(bundle);
  }

  /**
   * Merge multiple export bundles
   *
   * @param bundles - Bundles to merge
   * @param metadata - Optional metadata for merged bundle
   * @returns Merged bundle
   */
  mergeBundles(
    bundles: ExportBundle[],
    metadata?: { description?: string; tags?: string[] },
  ): ExportBundle {
    return mergeBundles(bundles, metadata);
  }
}
