/**
 * Regenerates ts-golden.db — the golden-file fixture proving that a database
 * created by the existing TypeScript store is readable by the Rust store.
 *
 * Uses the built TypeScript store (run `pnpm run build` first) with fixed
 * timestamps so the fixture is deterministic.
 *
 * Usage (from the repo root):
 *   node crates/kindling-store/tests/fixtures/generate-golden-db.mjs
 */

import { unlinkSync, existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const storePkg = join(here, '..', '..', '..', '..', 'packages', 'kindling-store-sqlite');
const { openDatabase, SqliteKindlingStore } = await import(join(storePkg, 'dist', 'index.js'));

const outPath = join(here, 'ts-golden.db');
for (const suffix of ['', '-wal', '-shm']) {
  if (existsSync(outPath + suffix)) unlinkSync(outPath + suffix);
}

const db = openDatabase({ path: outPath });
const store = new SqliteKindlingStore(db);

const scope = { sessionId: 'sess-golden', repoId: 'repo-golden', agentId: 'agent-1' };

store.insertObservation({
  id: 'obs-1',
  kind: 'tool_call',
  content: 'ran the database migration against staging',
  provenance: { tool: 'Bash', exitCode: 0 },
  ts: 1700000000001,
  scopeIds: scope,
  redacted: false,
});

store.insertObservation({
  id: 'obs-2',
  kind: 'error',
  content: 'connection timeout while deploying service',
  provenance: {},
  ts: 1700000000002,
  scopeIds: scope,
  redacted: false,
});

store.insertObservation({
  id: 'obs-3',
  kind: 'message',
  content: 'unrelated note in another session',
  provenance: {},
  ts: 1700000000003,
  scopeIds: { sessionId: 'sess-other' },
  redacted: false,
});

store.insertObservation({
  id: 'obs-redacted',
  kind: 'command',
  content: 'export API_KEY=super-secret-value',
  provenance: {},
  ts: 1700000000004,
  scopeIds: scope,
  redacted: false,
});
store.redactObservation('obs-redacted');

store.createCapsule({
  id: 'cap-1',
  type: 'session',
  intent: 'golden fixture capsule',
  status: 'open',
  openedAt: 1700000000000,
  scopeIds: scope,
  observationIds: [],
});
store.attachObservationToCapsule('cap-1', 'obs-1');
store.attachObservationToCapsule('cap-1', 'obs-2');

store.insertSummary({
  id: 'sum-1',
  capsuleId: 'cap-1',
  content: 'migrated the database and hit a deploy timeout',
  confidence: 0.95,
  createdAt: 1700000000500,
  evidenceRefs: ['obs-1', 'obs-2'],
});
store.closeCapsule('cap-1', 1700000001000, 'sum-1');

store.createCapsule({
  id: 'cap-2',
  type: 'pocketflow_node',
  intent: 'still open',
  status: 'open',
  openedAt: 1700000002000,
  scopeIds: { sessionId: 'sess-other' },
  observationIds: [],
});

store.insertPin({
  id: 'pin-1',
  targetType: 'observation',
  targetId: 'obs-1',
  reason: 'migration reference',
  createdAt: 1700000000600,
  scopeIds: scope,
});

store.insertPin({
  id: 'pin-2',
  targetType: 'summary',
  targetId: 'sum-1',
  reason: 'session summary',
  createdAt: 1700000000700,
  expiresAt: 9999999999999,
  scopeIds: scope,
});

store.insertPin({
  id: 'pin-expired',
  targetType: 'observation',
  targetId: 'obs-2',
  createdAt: 1700000000800,
  expiresAt: 1700000000900,
  scopeIds: scope,
});

db.close();
console.log(`golden fixture written to ${outPath}`);
