/**
 * Integration tests for KindlingService
 *
 * Tests end-to-end workflows using the service API
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import Database from 'better-sqlite3';
import { KindlingService } from '../src/service/kindling-service.js';
import { SqliteKindlingStore } from '../../kindling-store-sqlite/src/store/sqlite.js';
import { LocalFtsProvider } from '../../kindling-provider-local/src/provider/local-fts.js';
import { openDatabase } from '../../kindling-store-sqlite/src/db/open.js';
import type { Observation } from '../src/types/observation.js';

describe('KindlingService Integration', () => {
  let db: Database.Database;
  let service: KindlingService;

  beforeEach(() => {
    // Create in-memory database for testing
    db = openDatabase({ path: ':memory:' });
    const store = new SqliteKindlingStore(db);
    const provider = new LocalFtsProvider(db);
    service = new KindlingService({ store, provider });
  });

  afterEach(() => {
    db.close();
  });

  it('should complete a full session workflow', async () => {
    // Open a capsule for a development session
    const capsule = service.openCapsule({
      type: 'session',
      intent: 'debug',
      scopeIds: {
        sessionId: 'session-1',
        repoId: 'my-project',
      },
    });

    expect(capsule.id).toBeDefined();
    expect(capsule.status).toBe('open');

    // Append some observations
    const obs1: Observation = {
      id: 'obs-1',
      kind: 'command',
      content: 'npm test failed with authentication error',
      provenance: { command: 'npm test', exitCode: 1 },
      ts: Date.now(),
      scopeIds: { sessionId: 'session-1', repoId: 'my-project' },
      redacted: false,
    };

    service.appendObservation(obs1, { capsuleId: capsule.id });

    const obs2: Observation = {
      id: 'obs-2',
      kind: 'error',
      content: 'AuthenticationError: Invalid token',
      provenance: { stack: 'Error at login.ts:42' },
      ts: Date.now() + 1000,
      scopeIds: { sessionId: 'session-1', repoId: 'my-project' },
      redacted: false,
    };

    service.appendObservation(obs2, { capsuleId: capsule.id });

    // Retrieve observations
    const results = await service.retrieve({
      query: 'authentication',
      scopeIds: { sessionId: 'session-1' },
    });

    expect(results.candidates.length).toBeGreaterThan(0);
    expect(results.candidates.some((c) => c.entity.id === 'obs-1' || c.entity.id === 'obs-2')).toBe(
      true,
    );

    // Pin an important observation
    const pin = service.pin({
      targetType: 'observation',
      targetId: 'obs-2',
      note: 'Root cause identified',
      scopeIds: { sessionId: 'session-1', repoId: 'my-project' },
    });

    expect(pin.id).toBeDefined();
    expect(pin.targetId).toBe('obs-2');

    // Close capsule with summary
    const closedCapsule = service.closeCapsule(capsule.id, {
      generateSummary: true,
      summaryContent: 'Fixed authentication bug by updating token validation',
      confidence: 0.9,
    });

    expect(closedCapsule.status).toBe('closed');
    expect(closedCapsule.closedAt).toBeDefined();

    // Verify pin is in retrieval results
    const pinnedResults = await service.retrieve({
      query: 'error',
      scopeIds: { sessionId: 'session-1' },
    });

    expect(pinnedResults.pins.length).toBe(1);
    expect(pinnedResults.pins[0].target.id).toBe('obs-2');
  });

  it('should export and import data', () => {
    // Create some data
    const capsule = service.openCapsule({
      type: 'session',
      intent: 'test',
      scopeIds: { sessionId: 'session-2' },
    });

    const obs: Observation = {
      id: 'obs-export-1',
      kind: 'message',
      content: 'Test observation for export',
      provenance: {},
      ts: Date.now(),
      scopeIds: { sessionId: 'session-2' },
      redacted: false,
    };

    service.appendObservation(obs, { capsuleId: capsule.id });

    // Export
    const bundle = service.export();

    expect(bundle.dataset.observations.length).toBe(1);
    expect(bundle.dataset.capsules.length).toBe(1);

    // Create new database for import
    const db2 = openDatabase({ path: ':memory:' });
    const store2 = new SqliteKindlingStore(db2);
    const provider2 = new LocalFtsProvider(db2);
    const service2 = new KindlingService({ store: store2, provider: provider2 });

    // Import
    const result = service2.import(bundle);

    expect(result.observations).toBe(1);
    expect(result.capsules).toBe(1);
    expect(result.errors.length).toBe(0);

    // Verify data is accessible
    const importedObs = service2.getObservation('obs-export-1');
    expect(importedObs).toBeDefined();
    expect(importedObs?.content).toBe('Test observation for export');

    db2.close();
  });

  it('should handle redaction', async () => {
    const obs: Observation = {
      id: 'obs-secret',
      kind: 'message',
      content: 'API key: secret123',
      provenance: {},
      ts: Date.now(),
      scopeIds: {},
      redacted: false,
    };

    service.appendObservation(obs);

    // Verify observation exists
    let retrieved = service.getObservation('obs-secret');
    expect(retrieved?.content).toBe('API key: secret123');

    // Redact the observation
    service.forget('obs-secret');

    // Verify it's redacted in retrieval
    const results = await service.retrieve({
      query: 'API',
      scopeIds: {},
      includeRedacted: false,
    });

    expect(results.candidates.find((c) => c.entity.id === 'obs-secret')).toBeUndefined();

    // But still exists with redacted flag
    retrieved = service.getObservation('obs-secret');
    expect(retrieved?.redacted).toBe(true);
  });
});
