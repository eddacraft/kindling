/**
 * Service-level import coordination
 *
 * Provides high-level import API for restoring from export bundles.
 */

import type { ExportBundle } from './bundle.js';
import { validateBundle } from './bundle.js';
import type { ScopeIds, Observation, Capsule, Summary, Pin } from '../types/index.js';

/**
 * Store interface for import operations
 */
export interface ImportStore {
  /**
   * Import dataset into database
   */
  importDatabase(dataset: {
    version: string;
    exportedAt: number;
    scope?: Partial<ScopeIds>;
    observations: Observation[];
    capsules: Capsule[];
    summaries: Summary[];
    pins: Pin[];
  }): {
    observations: number;
    capsules: number;
    summaries: number;
    pins: number;
    errors: string[];
  };
}

/**
 * Import options
 */
export interface ImportOptions {
  /** Skip validation (default: false) */
  skipValidation?: boolean;
  /** Dry run (validate only, don't import) */
  dryRun?: boolean;
}

/**
 * Import result
 */
export interface ImportResult {
  /** Number of observations imported */
  observations: number;
  /** Number of capsules imported */
  capsules: number;
  /** Number of summaries imported */
  summaries: number;
  /** Number of pins imported */
  pins: number;
  /** Any errors encountered */
  errors: string[];
  /** Whether this was a dry run */
  dryRun: boolean;
}

/**
 * Restore from export bundle
 *
 * Coordinates import with validation and error handling.
 *
 * @param store - Import store
 * @param bundle - Export bundle to restore
 * @param options - Import options
 * @returns Import result with counts and errors
 */
export function restoreFromBundle(
  store: ImportStore,
  bundle: ExportBundle,
  options: ImportOptions = {},
): ImportResult {
  const { skipValidation = false, dryRun = false } = options;

  // Validate bundle structure
  if (!skipValidation) {
    const validation = validateBundle(bundle);
    if (!validation.valid) {
      return {
        observations: 0,
        capsules: 0,
        summaries: 0,
        pins: 0,
        errors: validation.errors,
        dryRun,
      };
    }
  }

  // Dry run: validate only, don't import
  if (dryRun) {
    return {
      observations: bundle.dataset.observations.length,
      capsules: bundle.dataset.capsules.length,
      summaries: bundle.dataset.summaries.length,
      pins: bundle.dataset.pins.length,
      errors: [],
      dryRun: true,
    };
  }

  // Import dataset
  const result = store.importDatabase(bundle.dataset);

  return {
    ...result,
    dryRun: false,
  };
}

/**
 * Merge multiple export bundles
 *
 * Combines datasets from multiple bundles into a single bundle.
 * Useful for consolidating exports from different sources.
 *
 * @param bundles - Bundles to merge
 * @param metadata - Optional metadata for merged bundle
 * @returns Merged bundle
 */
export function mergeBundles(
  bundles: ExportBundle[],
  metadata?: { description?: string; tags?: string[] },
): ExportBundle {
  if (bundles.length === 0) {
    throw new Error('At least one bundle required for merge');
  }

  // Validate all bundles
  for (const bundle of bundles) {
    const validation = validateBundle(bundle);
    if (!validation.valid) {
      throw new Error(`Invalid bundle: ${validation.errors.join(', ')}`);
    }
  }

  // Merge datasets
  const merged = {
    observations: [] as Observation[],
    capsules: [] as Capsule[],
    summaries: [] as Summary[],
    pins: [] as Pin[],
  };

  for (const bundle of bundles) {
    merged.observations.push(...bundle.dataset.observations);
    merged.capsules.push(...bundle.dataset.capsules);
    merged.summaries.push(...bundle.dataset.summaries);
    merged.pins.push(...bundle.dataset.pins);
  }

  // Deduplicate by ID
  const deduped = {
    observations: deduplicateById(merged.observations),
    capsules: deduplicateById(merged.capsules),
    summaries: deduplicateById(merged.summaries),
    pins: deduplicateById(merged.pins),
  };

  return {
    bundleVersion: '1.0',
    exportedAt: Date.now(),
    metadata: metadata || {
      description: `Merged from ${bundles.length} bundles`,
    },
    dataset: {
      version: '1.0',
      exportedAt: Date.now(),
      ...deduped,
    },
  };
}

/**
 * Deduplicate entities by ID
 *
 * Keeps first occurrence of each ID.
 *
 * @param entities - Entities to deduplicate
 * @returns Deduplicated entities
 */
function deduplicateById<T extends { id: string }>(entities: T[]): T[] {
  const seen = new Set<string>();
  const deduped: T[] = [];

  for (const entity of entities) {
    if (!entity.id) continue;

    if (!seen.has(entity.id)) {
      seen.add(entity.id);
      deduped.push(entity);
    }
  }

  return deduped;
}

/**
 * Compare two bundles for differences
 *
 * @param bundle1 - First bundle
 * @param bundle2 - Second bundle
 * @returns Difference summary
 */
export function compareBundles(
  bundle1: ExportBundle,
  bundle2: ExportBundle,
): {
  observations: { added: number; removed: number; common: number };
  capsules: { added: number; removed: number; common: number };
  summaries: { added: number; removed: number; common: number };
  pins: { added: number; removed: number; common: number };
} {
  const compareArrays = (arr1: { id: string }[], arr2: { id: string }[]) => {
    const ids1 = new Set(arr1.map((e) => e.id).filter(Boolean));
    const ids2 = new Set(arr2.map((e) => e.id).filter(Boolean));

    const common = new Set([...ids1].filter((id) => ids2.has(id)));
    const added = ids2.size - common.size;
    const removed = ids1.size - common.size;

    return { added, removed, common: common.size };
  };

  return {
    observations: compareArrays(bundle1.dataset.observations, bundle2.dataset.observations),
    capsules: compareArrays(bundle1.dataset.capsules, bundle2.dataset.capsules),
    summaries: compareArrays(bundle1.dataset.summaries, bundle2.dataset.summaries),
    pins: compareArrays(bundle1.dataset.pins, bundle2.dataset.pins),
  };
}
