/**
 * Tests for export/import coordination
 */

import { describe, it, expect } from 'vitest';
import {
  createExportBundle,
  getBundleStats,
  validateBundle,
  serializeBundle,
  deserializeBundle,
  type ExportStore,
  type ExportBundle,
} from '../src/export/bundle.js';
import {
  restoreFromBundle,
  mergeBundles,
  compareBundles,
  type ImportStore,
} from '../src/export/restore.js';

/**
 * Mock export store
 */
class MockExportStore implements ExportStore {
  dataset = {
    version: '1.0',
    exportedAt: Date.now(),
    observations: [],
    capsules: [],
    summaries: [],
    pins: [],
  };

  exportDatabase(options?: any) {
    return { ...this.dataset, ...options };
  }
}

/**
 * Mock import store
 */
class MockImportStore implements ImportStore {
  imported: any[] = [];

  importDatabase(dataset: any) {
    this.imported.push(dataset);
    return {
      observations: dataset.observations.length,
      capsules: dataset.capsules.length,
      summaries: dataset.summaries.length,
      pins: dataset.pins.length,
      errors: [],
    };
  }
}

describe('Export/Import Coordination', () => {
  describe('createExportBundle', () => {
    it('should create bundle with dataset', () => {
      const store = new MockExportStore();
      store.dataset.observations = [{ id: 'obs-1', content: 'test' }];

      const bundle = createExportBundle(store);

      expect(bundle.bundleVersion).toBe('1.0');
      expect(bundle.exportedAt).toBeGreaterThan(0);
      expect(bundle.dataset.observations).toHaveLength(1);
    });

    it('should include metadata when provided', () => {
      const store = new MockExportStore();
      const bundle = createExportBundle(store, {
        metadata: {
          description: 'Test backup',
          tags: ['backup', 'test'],
        },
      });

      expect(bundle.metadata).toBeDefined();
      expect(bundle.metadata!.description).toBe('Test backup');
      expect(bundle.metadata!.tags).toEqual(['backup', 'test']);
    });

    it('should pass options to store', () => {
      const store = new MockExportStore();
      const bundle = createExportBundle(store, {
        scope: { sessionId: 's1' },
        includeRedacted: true,
        limit: 100,
      });

      expect(bundle.dataset.scope).toEqual({ sessionId: 's1' });
      expect(bundle.dataset.includeRedacted).toBe(true);
      expect(bundle.dataset.limit).toBe(100);
    });
  });

  describe('getBundleStats', () => {
    it('should return correct entity counts', () => {
      const bundle: ExportBundle = {
        bundleVersion: '1.0',
        exportedAt: Date.now(),
        dataset: {
          version: '1.0',
          exportedAt: Date.now(),
          observations: [{ id: '1' }, { id: '2' }],
          capsules: [{ id: '1' }],
          summaries: [{ id: '1' }],
          pins: [],
        },
      };

      const stats = getBundleStats(bundle);

      expect(stats.observations).toBe(2);
      expect(stats.capsules).toBe(1);
      expect(stats.summaries).toBe(1);
      expect(stats.pins).toBe(0);
      expect(stats.totalSize).toBeGreaterThan(0);
    });
  });

  describe('validateBundle', () => {
    it('should validate correct bundle', () => {
      const bundle: ExportBundle = {
        bundleVersion: '1.0',
        exportedAt: Date.now(),
        dataset: {
          version: '1.0',
          exportedAt: Date.now(),
          observations: [],
          capsules: [],
          summaries: [],
          pins: [],
        },
      };

      const result = validateBundle(bundle);

      expect(result.valid).toBe(true);
      expect(result.errors).toHaveLength(0);
    });

    it('should reject non-object bundle', () => {
      const result = validateBundle('not an object');

      expect(result.valid).toBe(false);
      expect(result.errors).toContain('Bundle must be an object');
    });

    it('should reject missing bundleVersion', () => {
      const bundle = {
        exportedAt: Date.now(),
        dataset: {
          version: '1.0',
          exportedAt: Date.now(),
          observations: [],
          capsules: [],
          summaries: [],
          pins: [],
        },
      };

      const result = validateBundle(bundle);

      expect(result.valid).toBe(false);
      expect(result.errors.some((e) => e.includes('bundleVersion'))).toBe(true);
    });

    it('should reject unsupported bundle version', () => {
      const bundle = {
        bundleVersion: '2.0',
        exportedAt: Date.now(),
        dataset: {
          version: '1.0',
          exportedAt: Date.now(),
          observations: [],
          capsules: [],
          summaries: [],
          pins: [],
        },
      };

      const result = validateBundle(bundle);

      expect(result.valid).toBe(false);
      expect(result.errors.some((e) => e.includes('Unsupported bundle version'))).toBe(true);
    });

    it('should reject invalid dataset arrays', () => {
      const bundle = {
        bundleVersion: '1.0',
        exportedAt: Date.now(),
        dataset: {
          version: '1.0',
          exportedAt: Date.now(),
          observations: 'not an array',
          capsules: [],
          summaries: [],
          pins: [],
        },
      };

      const result = validateBundle(bundle);

      expect(result.valid).toBe(false);
      expect(result.errors.some((e) => e.includes('must be an array'))).toBe(true);
    });
  });

  describe('serializeBundle / deserializeBundle', () => {
    it('should round-trip serialize/deserialize', () => {
      const bundle: ExportBundle = {
        bundleVersion: '1.0',
        exportedAt: 123456,
        dataset: {
          version: '1.0',
          exportedAt: Date.now(),
          observations: [{ id: 'obs-1' }],
          capsules: [],
          summaries: [],
          pins: [],
        },
      };

      const json = serializeBundle(bundle);
      const deserialized = deserializeBundle(json);

      expect(deserialized).toEqual(bundle);
    });

    it('should support pretty formatting', () => {
      const bundle: ExportBundle = {
        bundleVersion: '1.0',
        exportedAt: 123456,
        dataset: {
          version: '1.0',
          exportedAt: Date.now(),
          observations: [],
          capsules: [],
          summaries: [],
          pins: [],
        },
      };

      const json = serializeBundle(bundle, true);

      expect(json).toContain('\n'); // Pretty format has newlines
      expect(json).toContain('  '); // Pretty format has indentation
    });

    it('should throw on invalid JSON', () => {
      expect(() => deserializeBundle('not valid json')).toThrow('Invalid JSON');
    });

    it('should throw on invalid bundle structure', () => {
      const invalidBundle = JSON.stringify({ invalid: true });

      expect(() => deserializeBundle(invalidBundle)).toThrow('Invalid bundle');
    });
  });

  describe('restoreFromBundle', () => {
    it('should import valid bundle', () => {
      const store = new MockImportStore();
      const bundle: ExportBundle = {
        bundleVersion: '1.0',
        exportedAt: Date.now(),
        dataset: {
          version: '1.0',
          exportedAt: Date.now(),
          observations: [{ id: 'obs-1' }],
          capsules: [{ id: 'cap-1' }],
          summaries: [],
          pins: [],
        },
      };

      const result = restoreFromBundle(store, bundle);

      expect(result.observations).toBe(1);
      expect(result.capsules).toBe(1);
      expect(result.errors).toHaveLength(0);
      expect(result.dryRun).toBe(false);
      expect(store.imported).toHaveLength(1);
    });

    it('should reject invalid bundle', () => {
      const store = new MockImportStore();
      const bundle = {
        invalid: true,
      } as any;

      const result = restoreFromBundle(store, bundle);

      expect(result.observations).toBe(0);
      expect(result.errors.length).toBeGreaterThan(0);
      expect(store.imported).toHaveLength(0);
    });

    it('should skip validation when requested', () => {
      const store = new MockImportStore();
      const bundle = {
        bundleVersion: '1.0',
        exportedAt: Date.now(),
        dataset: {
          version: '1.0',
          exportedAt: Date.now(),
          observations: [],
          capsules: [],
          summaries: [],
          pins: [],
        },
      } as ExportBundle;

      const result = restoreFromBundle(store, bundle, {
        skipValidation: true,
      });

      expect(result.errors).toHaveLength(0);
    });

    it('should support dry run', () => {
      const store = new MockImportStore();
      const bundle: ExportBundle = {
        bundleVersion: '1.0',
        exportedAt: Date.now(),
        dataset: {
          version: '1.0',
          exportedAt: Date.now(),
          observations: [{ id: '1' }, { id: '2' }],
          capsules: [{ id: '1' }],
          summaries: [],
          pins: [],
        },
      };

      const result = restoreFromBundle(store, bundle, { dryRun: true });

      expect(result.observations).toBe(2);
      expect(result.capsules).toBe(1);
      expect(result.dryRun).toBe(true);
      expect(store.imported).toHaveLength(0); // Not actually imported
    });
  });

  describe('mergeBundles', () => {
    it('should merge multiple bundles', () => {
      const bundle1: ExportBundle = {
        bundleVersion: '1.0',
        exportedAt: Date.now(),
        dataset: {
          version: '1.0',
          exportedAt: Date.now(),
          observations: [{ id: 'obs-1' }],
          capsules: [{ id: 'cap-1' }],
          summaries: [],
          pins: [],
        },
      };

      const bundle2: ExportBundle = {
        bundleVersion: '1.0',
        exportedAt: Date.now(),
        dataset: {
          version: '1.0',
          exportedAt: Date.now(),
          observations: [{ id: 'obs-2' }],
          capsules: [],
          summaries: [{ id: 'sum-1' }],
          pins: [],
        },
      };

      const merged = mergeBundles([bundle1, bundle2]);

      expect(merged.dataset.observations).toHaveLength(2);
      expect(merged.dataset.capsules).toHaveLength(1);
      expect(merged.dataset.summaries).toHaveLength(1);
    });

    it('should deduplicate by ID', () => {
      const bundle1: ExportBundle = {
        bundleVersion: '1.0',
        exportedAt: Date.now(),
        dataset: {
          version: '1.0',
          exportedAt: Date.now(),
          observations: [{ id: 'obs-1', content: 'first' }],
          capsules: [],
          summaries: [],
          pins: [],
        },
      };

      const bundle2: ExportBundle = {
        bundleVersion: '1.0',
        exportedAt: Date.now(),
        dataset: {
          version: '1.0',
          exportedAt: Date.now(),
          observations: [{ id: 'obs-1', content: 'duplicate' }],
          capsules: [],
          summaries: [],
          pins: [],
        },
      };

      const merged = mergeBundles([bundle1, bundle2]);

      expect(merged.dataset.observations).toHaveLength(1);
      expect(merged.dataset.observations[0].content).toBe('first'); // First occurrence kept
    });

    it('should include metadata', () => {
      const bundle: ExportBundle = {
        bundleVersion: '1.0',
        exportedAt: Date.now(),
        dataset: {
          version: '1.0',
          exportedAt: Date.now(),
          observations: [],
          capsules: [],
          summaries: [],
          pins: [],
        },
      };

      const merged = mergeBundles([bundle], {
        description: 'Merged backup',
        tags: ['merged'],
      });

      expect(merged.metadata!.description).toBe('Merged backup');
      expect(merged.metadata!.tags).toEqual(['merged']);
    });

    it('should throw on empty array', () => {
      expect(() => mergeBundles([])).toThrow('At least one bundle required for merge');
    });
  });

  describe('compareBundles', () => {
    it('should detect added entities', () => {
      const bundle1: ExportBundle = {
        bundleVersion: '1.0',
        exportedAt: Date.now(),
        dataset: {
          version: '1.0',
          exportedAt: Date.now(),
          observations: [],
          capsules: [],
          summaries: [],
          pins: [],
        },
      };

      const bundle2: ExportBundle = {
        bundleVersion: '1.0',
        exportedAt: Date.now(),
        dataset: {
          version: '1.0',
          exportedAt: Date.now(),
          observations: [{ id: 'obs-1' }],
          capsules: [],
          summaries: [],
          pins: [],
        },
      };

      const diff = compareBundles(bundle1, bundle2);

      expect(diff.observations.added).toBe(1);
      expect(diff.observations.removed).toBe(0);
      expect(diff.observations.common).toBe(0);
    });

    it('should detect removed entities', () => {
      const bundle1: ExportBundle = {
        bundleVersion: '1.0',
        exportedAt: Date.now(),
        dataset: {
          version: '1.0',
          exportedAt: Date.now(),
          observations: [{ id: 'obs-1' }],
          capsules: [],
          summaries: [],
          pins: [],
        },
      };

      const bundle2: ExportBundle = {
        bundleVersion: '1.0',
        exportedAt: Date.now(),
        dataset: {
          version: '1.0',
          exportedAt: Date.now(),
          observations: [],
          capsules: [],
          summaries: [],
          pins: [],
        },
      };

      const diff = compareBundles(bundle1, bundle2);

      expect(diff.observations.removed).toBe(1);
      expect(diff.observations.added).toBe(0);
    });

    it('should detect common entities', () => {
      const bundle1: ExportBundle = {
        bundleVersion: '1.0',
        exportedAt: Date.now(),
        dataset: {
          version: '1.0',
          exportedAt: Date.now(),
          observations: [{ id: 'obs-1' }],
          capsules: [],
          summaries: [],
          pins: [],
        },
      };

      const bundle2: ExportBundle = {
        bundleVersion: '1.0',
        exportedAt: Date.now(),
        dataset: {
          version: '1.0',
          exportedAt: Date.now(),
          observations: [{ id: 'obs-1' }],
          capsules: [],
          summaries: [],
          pins: [],
        },
      };

      const diff = compareBundles(bundle1, bundle2);

      expect(diff.observations.common).toBe(1);
      expect(diff.observations.added).toBe(0);
      expect(diff.observations.removed).toBe(0);
    });
  });
});
