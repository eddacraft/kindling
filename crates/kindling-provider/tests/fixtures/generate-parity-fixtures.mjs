/**
 * Regenerates ts-parity.db and ts-parity.json — the cross-implementation
 * parity fixtures proving that identical queries against the same database
 * produce the same ranked results from the Rust provider as from the
 * existing TypeScript provider and orchestrator.
 *
 * Date.now() is pinned to FIXED_NOW before any search so recency scoring and
 * pin expiry are deterministic; the Rust side replays the same clock through
 * `search(…, now)` / `retrieve_at(…, now)`.
 *
 * Uses the built TypeScript packages (run `pnpm run build` first). Node
 * v25.6.0 (via fnm) is required for the better-sqlite3 ABI:
 *
 *   fnm exec --using=v25.6.0 node crates/kindling-provider/tests/fixtures/generate-parity-fixtures.mjs
 */

import { unlinkSync, existsSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..', '..', '..', '..');
const pkg = (name) => join(root, 'packages', name, 'dist', 'index.js');

const { openDatabase, closeDatabase, SqliteKindlingStore } = await import(
  pkg('kindling-store-sqlite')
);
const { LocalFtsProvider } = await import(pkg('kindling-provider-local'));
const { retrieve } = await import(pkg('kindling-core'));

const FIXED_NOW = 1750000000000;
const HOUR = 60 * 60 * 1000;
const DAY = 24 * HOUR;

const dbPath = join(here, 'ts-parity.db');
for (const suffix of ['', '-wal', '-shm']) {
  if (existsSync(dbPath + suffix)) unlinkSync(dbPath + suffix);
}

const db = openDatabase({ path: dbPath });
const store = new SqliteKindlingStore(db);

const scopeA = { sessionId: 'sess-1', repoId: 'repo-1' };
const scopeB = { sessionId: 'sess-2', repoId: 'repo-2', agentId: 'agent-9' };

// ===== Observations =====

const observations = [
  {
    id: 'obs-1',
    kind: 'error',
    content: 'deploy failed with database connection timeout',
    provenance: { tool: 'Bash', exitCode: 1 },
    ts: FIXED_NOW - 1 * HOUR,
    scopeIds: scopeA,
    redacted: false,
  },
  {
    id: 'obs-2',
    kind: 'command',
    content: 'database migration completed successfully',
    provenance: {},
    ts: FIXED_NOW - 5 * DAY,
    scopeIds: scopeA,
    redacted: false,
  },
  {
    id: 'obs-3',
    kind: 'message',
    content: 'fixed flaky test in retrieval module near the database layer',
    provenance: {},
    ts: FIXED_NOW - 12 * DAY,
    scopeIds: scopeA,
    redacted: false,
  },
  {
    id: 'obs-4',
    kind: 'message',
    content: 'database database database heavy term frequency note',
    provenance: {},
    ts: FIXED_NOW - 45 * DAY, // older than the 30-day recency window
    scopeIds: scopeA,
    redacted: false,
  },
  {
    id: 'obs-5',
    kind: 'tool_call',
    content: `database ${'x'.repeat(150)}`, // match-context truncation case
    provenance: { tool: 'Read' },
    ts: FIXED_NOW - 2 * DAY,
    scopeIds: scopeA,
    redacted: false,
  },
  {
    id: 'obs-6',
    kind: 'message',
    content: `database \u{1F525} ${'y'.repeat(120)}`, // unicode + truncation
    provenance: {},
    ts: FIXED_NOW - 3 * DAY,
    scopeIds: scopeA,
    redacted: false,
  },
  {
    id: 'obs-7',
    kind: 'message',
    content: 'database secret token value',
    provenance: {},
    ts: FIXED_NOW - 1 * DAY,
    scopeIds: scopeA,
    redacted: true, // never FTS-indexed
  },
  {
    id: 'obs-8',
    kind: 'message',
    content: 'database tuning notes for the other repository',
    provenance: {},
    ts: FIXED_NOW - 1 * DAY,
    scopeIds: scopeB,
    redacted: false,
  },
];

for (const obs of observations) store.insertObservation(obs);

// ===== Capsules + summaries (one summary per capsule) =====

store.createCapsule({
  id: 'cap-1',
  type: 'session',
  intent: 'database reliability work',
  status: 'open',
  openedAt: FIXED_NOW - 2 * HOUR,
  scopeIds: scopeA,
  observationIds: [],
});
store.insertSummary({
  id: 'sum-1',
  capsuleId: 'cap-1',
  content: 'Session focused on database reliability and deploy fixes',
  confidence: 0.9,
  createdAt: FIXED_NOW - 30 * 60 * 1000,
  evidenceRefs: ['obs-1', 'obs-2'],
});

store.createCapsule({
  id: 'cap-2',
  type: 'session',
  intent: 'index performance',
  status: 'open',
  openedAt: FIXED_NOW - 9 * DAY,
  scopeIds: scopeB,
  observationIds: [],
});
store.insertSummary({
  id: 'sum-2',
  capsuleId: 'cap-2',
  content: 'Investigated database index performance regressions',
  confidence: 0.75,
  createdAt: FIXED_NOW - 8 * DAY,
  evidenceRefs: ['obs-8'],
});

// ===== Pins =====

store.insertPin({
  id: 'pin-1',
  targetType: 'observation',
  targetId: 'obs-2',
  reason: 'migration baseline',
  createdAt: FIXED_NOW - 4 * DAY,
  scopeIds: { sessionId: 'sess-1' },
});
store.insertPin({
  id: 'pin-2',
  targetType: 'summary',
  targetId: 'sum-2',
  createdAt: FIXED_NOW - 3 * DAY,
  scopeIds: { sessionId: 'sess-1' },
});
store.insertPin({
  id: 'pin-3',
  targetType: 'observation',
  targetId: 'obs-3',
  createdAt: FIXED_NOW - 10 * DAY,
  expiresAt: FIXED_NOW - 1, // expired
  scopeIds: { sessionId: 'sess-1' },
});

// ===== Pin the clock, run the cases =====

Date.now = () => FIXED_NOW;

const provider = new LocalFtsProvider(db);

const searchCases = [
  { name: 'broad query, no scope', options: { query: 'database', scopeIds: {} } },
  { name: 'scoped to session', options: { query: 'database', scopeIds: { sessionId: 'sess-1' } } },
  {
    name: 'scoped to repo with maxResults',
    options: { query: 'database', scopeIds: { repoId: 'repo-1' }, maxResults: 3 },
  },
  { name: 'OR operator', options: { query: 'deploy OR migration', scopeIds: {} } },
  { name: 'phrase query', options: { query: '"database migration"', scopeIds: {} } },
  {
    name: 'excludeIds',
    options: { query: 'database', scopeIds: {}, excludeIds: ['obs-1', 'sum-2'] },
  },
  {
    name: 'includeRedacted (redacted rows are not FTS-indexed)',
    options: { query: 'database', scopeIds: {}, includeRedacted: true },
  },
  { name: 'malformed query', options: { query: 'AND OR', scopeIds: {} } },
  { name: 'prefix query', options: { query: 'datab*', scopeIds: {} } },
  { name: 'no matches', options: { query: 'nonexistentterm', scopeIds: {} } },
];

const retrieveCases = [
  { name: 'session scope: pins + current summary + candidates', options: { query: 'database', scopeIds: { sessionId: 'sess-1' } } },
  {
    name: 'session scope with maxCandidates',
    options: { query: 'database', scopeIds: { sessionId: 'sess-1' }, maxCandidates: 2 },
  },
  { name: 'no session: pins unscoped, no current summary', options: { query: 'database', scopeIds: {} } },
  { name: 'other session: scoped pins absent, own summary', options: { query: 'database', scopeIds: { sessionId: 'sess-2' } } },
];

const fixture = {
  now: FIXED_NOW,
  searchCases: [],
  retrieveCases: [],
};

for (const { name, options } of searchCases) {
  const expected = await provider.search(options);
  fixture.searchCases.push({ name, options, expected });
}

for (const { name, options } of retrieveCases) {
  const expected = await retrieve(store, provider, options);
  fixture.retrieveCases.push({ name, options, expected });
}

writeFileSync(join(here, 'ts-parity.json'), JSON.stringify(fixture, null, 2) + '\n');

closeDatabase(db);
for (const suffix of ['-wal', '-shm']) {
  if (existsSync(dbPath + suffix)) unlinkSync(dbPath + suffix);
}

console.log(
  `wrote ts-parity.db and ts-parity.json (${fixture.searchCases.length} search cases, ${fixture.retrieveCases.length} retrieve cases)`,
);
