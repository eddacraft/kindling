/**
 * Service-level export coordination
 *
 * Provides high-level export API for backup and portability.
 * Builds on store-level export primitives from STORAGE-005.
 */

import type { ScopeIds } from '../types/common.js';
import type { Observation, Capsule, Summary, Pin } from '../types/index.js';

/**
 * Store interface for export operations
 */
export interface ExportStore {
  /**
   * Export database to a dataset
   */
  exportDatabase(options?: {
    scope?: Partial<ScopeIds>;
    includeRedacted?: boolean;
    limit?: number;
  }): {
    version: string;
    exportedAt: number;
    scope?: Partial<ScopeIds>;
    observations: Observation[];
    capsules: Capsule[];
    summaries: Summary[];
    pins: Pin[];
  };
}

/**
 * Export options
 */
export interface ExportBundleOptions {
  /** Optional scope filter */
  scope?: Partial<ScopeIds>;
  /** Include redacted observations (default: false) */
  includeRedacted?: boolean;
  /** Maximum observations to export */
  limit?: number;
  /** Bundle metadata */
  metadata?: {
    description?: string;
    tags?: string[];
    [key: string]: unknown;
  };
}

/**
 * Export bundle with metadata
 */
export interface ExportBundle {
  /** Bundle format version */
  bundleVersion: string;
  /** Export timestamp */
  exportedAt: number;
  /** Optional metadata */
  metadata?: {
    description?: string;
    tags?: string[];
    [key: string]: unknown;
  };
  /** Dataset with entities */
  dataset: {
    version: string;
    exportedAt: number;
    scope?: Partial<ScopeIds>;
    observations: Observation[];
    capsules: Capsule[];
    summaries: Summary[];
    pins: Pin[];
  };
}

/**
 * Export bundle statistics
 */
export interface ExportStats {
  observations: number;
  capsules: number;
  summaries: number;
  pins: number;
  totalSize: number;
}

/**
 * Create an export bundle
 *
 * Coordinates export from store with optional metadata and validation.
 *
 * @param store - Export store
 * @param options - Export options
 * @returns Export bundle with dataset and metadata
 */
export function createExportBundle(
  store: ExportStore,
  options: ExportBundleOptions = {},
): ExportBundle {
  const { scope, includeRedacted = false, limit, metadata } = options;

  // Export dataset from store
  const dataset = store.exportDatabase({
    scope,
    includeRedacted,
    limit,
  });

  // Create bundle with metadata
  const bundle: ExportBundle = {
    bundleVersion: '1.0',
    exportedAt: Date.now(),
    dataset,
  };

  if (metadata) {
    bundle.metadata = metadata;
  }

  return bundle;
}

/**
 * Get statistics for an export bundle
 *
 * @param bundle - Export bundle
 * @returns Statistics about bundle contents
 */
export function getBundleStats(bundle: ExportBundle): ExportStats {
  const { dataset } = bundle;

  // Estimate size (JSON string length approximation)
  const totalSize = JSON.stringify(bundle).length;

  return {
    observations: dataset.observations.length,
    capsules: dataset.capsules.length,
    summaries: dataset.summaries.length,
    pins: dataset.pins.length,
    totalSize,
  };
}

/**
 * Validate export bundle structure
 *
 * @param bundle - Bundle to validate
 * @returns Validation result with errors if any
 */
export function validateBundle(bundle: unknown): {
  valid: boolean;
  errors: string[];
} {
  const errors: string[] = [];

  if (!bundle || typeof bundle !== 'object') {
    return { valid: false, errors: ['Bundle must be an object'] };
  }

  const b = bundle as Partial<ExportBundle>;

  // Check bundle version
  if (!b.bundleVersion || typeof b.bundleVersion !== 'string') {
    errors.push('Missing or invalid bundleVersion');
  } else if (b.bundleVersion !== '1.0') {
    errors.push(`Unsupported bundle version: ${b.bundleVersion}`);
  }

  // Check exportedAt
  if (!b.exportedAt || typeof b.exportedAt !== 'number') {
    errors.push('Missing or invalid exportedAt');
  }

  // Check dataset
  if (!b.dataset || typeof b.dataset !== 'object') {
    errors.push('Missing or invalid dataset');
  } else {
    const ds = b.dataset;

    if (!ds.version || typeof ds.version !== 'string') {
      errors.push('Missing or invalid dataset.version');
    }

    if (!Array.isArray(ds.observations)) {
      errors.push('dataset.observations must be an array');
    }

    if (!Array.isArray(ds.capsules)) {
      errors.push('dataset.capsules must be an array');
    }

    if (!Array.isArray(ds.summaries)) {
      errors.push('dataset.summaries must be an array');
    }

    if (!Array.isArray(ds.pins)) {
      errors.push('dataset.pins must be an array');
    }
  }

  return {
    valid: errors.length === 0,
    errors,
  };
}

/**
 * Serialize export bundle to JSON string
 *
 * @param bundle - Bundle to serialize
 * @param pretty - Use pretty formatting (default: false)
 * @returns JSON string
 */
export function serializeBundle(bundle: ExportBundle, pretty = false): string {
  return JSON.stringify(bundle, null, pretty ? 2 : 0);
}

/**
 * Deserialize export bundle from JSON string
 *
 * @param json - JSON string to parse
 * @returns Parsed bundle
 * @throws Error if JSON is invalid
 */
export function deserializeBundle(json: string): ExportBundle {
  try {
    const bundle = JSON.parse(json);
    const validation = validateBundle(bundle);

    if (!validation.valid) {
      throw new Error(`Invalid bundle: ${validation.errors.join(', ')}`);
    }

    return bundle as ExportBundle;
  } catch (err) {
    if (err instanceof SyntaxError) {
      throw new Error(`Invalid JSON: ${err.message}`);
    }
    throw err;
  }
}
