/**
 * End-to-end tests for the `/memory` slash-command scripts.
 *
 * These scripts shell out to the `kindling` binary, so the tests need a built
 * binary. Point `KINDLING_BIN` at it (e.g. the Rust workspace's
 * `target/debug/kindling`). When the binary is absent the whole suite is
 * skipped, so `pnpm -r test` never hard-fails on machines without it.
 *
 * Each test seeds observations via `kindling log ... --db <tmp>` and then runs
 * the `/memory` scripts against the same temp DB (via `KINDLING_DB_PATH`),
 * asserting the human-readable output and the resulting DB state.
 *
 * Run:
 *   KINDLING_BIN=/abs/path/to/target/debug/kindling node --test test/
 */

const test = require('node:test');
const assert = require('node:assert');
const { execFileSync } = require('node:child_process');
const { mkdtempSync, mkdirSync, existsSync } = require('node:fs');
const { tmpdir } = require('node:os');
const { join, resolve } = require('node:path');

const SCRIPTS = join(__dirname, '..', 'scripts');
const { projectRoot } = require(join(SCRIPTS, 'lib', 'kindling.js'));

function resolveBin() {
  const bin = process.env.KINDLING_BIN;
  if (!bin) return null;
  if (!existsSync(bin)) return null;
  return bin;
}

const KINDLING_BIN = resolveBin();
const skip = KINDLING_BIN
  ? false
  : 'KINDLING_BIN not set or binary missing — skipping /memory command tests';

/** Fresh temp DB + env for one test. */
function freshEnv() {
  const dir = mkdtempSync(join(tmpdir(), 'kindling-mem-'));
  const db = join(dir, 'kindling.db');
  return {
    db,
    env: {
      ...process.env,
      KINDLING_BIN,
      KINDLING_DB_PATH: db,
    },
  };
}

/** `kindling log <content> --db <db>` (seed an observation). */
function seed(db, content) {
  execFileSync(KINDLING_BIN, ['log', content, '--db', db], { stdio: 'pipe' });
}

/** Run a `/memory` script with the given args and return stdout. */
function runScript(name, args, env) {
  return execFileSync('node', [join(SCRIPTS, name), ...args], {
    env,
    encoding: 'utf-8',
  });
}

/** `kindling <verb...> --json` against the temp DB, parsed. */
function cliJson(db, args) {
  const out = execFileSync(KINDLING_BIN, [...args, '--db', db, '--json'], {
    encoding: 'utf-8',
  });
  return JSON.parse(out);
}

// --- projectRoot path containment (no binary required) ----------------------
//
// KINDLING_REPO_ROOT must only be honoured when the cwd is genuinely inside the
// root. A raw string prefix test would wrongly accept a sibling directory
// (e.g. `/work/repo-copy` for root `/work/repo`), letting a command target
// another project's hash → DB. These tests pin the real path-boundary check.

test('projectRoot honours KINDLING_REPO_ROOT for the exact root', () => {
  const root = mkdtempSync(join(tmpdir(), 'kindling-root-'));
  const saved = process.env.KINDLING_REPO_ROOT;
  process.env.KINDLING_REPO_ROOT = root;
  try {
    assert.equal(projectRoot(root), root);
  } finally {
    if (saved === undefined) delete process.env.KINDLING_REPO_ROOT;
    else process.env.KINDLING_REPO_ROOT = saved;
  }
});

test('projectRoot honours KINDLING_REPO_ROOT for a child path', () => {
  const root = mkdtempSync(join(tmpdir(), 'kindling-root-'));
  const child = join(root, 'nested', 'sub');
  mkdirSync(child, { recursive: true });
  const saved = process.env.KINDLING_REPO_ROOT;
  process.env.KINDLING_REPO_ROOT = root;
  try {
    assert.equal(projectRoot(child), root);
  } finally {
    if (saved === undefined) delete process.env.KINDLING_REPO_ROOT;
    else process.env.KINDLING_REPO_ROOT = saved;
  }
});

test('projectRoot rejects a sibling path sharing a string prefix', () => {
  const root = mkdtempSync(join(tmpdir(), 'kindling-root-'));
  // A sibling whose path starts with `root` as a raw string but is NOT inside
  // it: `<root>-copy`. The override must be ignored here.
  const sibling = root + '-copy';
  mkdirSync(sibling, { recursive: true });
  const saved = process.env.KINDLING_REPO_ROOT;
  process.env.KINDLING_REPO_ROOT = root;
  try {
    const resolved = projectRoot(sibling);
    assert.notEqual(resolved, root, 'sibling path must not resolve to the override root');
    assert.equal(resolved, resolve(sibling));
  } finally {
    if (saved === undefined) delete process.env.KINDLING_REPO_ROOT;
    else process.env.KINDLING_REPO_ROOT = saved;
  }
});

test('memory-search finds seeded content', { skip }, () => {
  const { db, env } = freshEnv();
  seed(db, 'implemented oauth token refresh in the auth module');
  seed(db, 'fixed caching bug in the retrieval layer');

  const out = runScript('memory-search.js', ['oauth', 'token'], env);
  assert.match(out, /Search Results/);
  assert.match(out, /oauth token refresh/);
});

test('memory-status reports counts and db path', { skip }, () => {
  const { db, env } = freshEnv();
  seed(db, 'first observation');
  seed(db, 'second observation');

  const out = runScript('memory-status.js', [], env);
  assert.match(out, /Observations: 2/);
  assert.match(out, /Pins:\s+0/);
  assert.ok(out.includes(db), 'status should print the db path');
});

test('memory-pin pins the most recent observation; pins lists it', { skip }, () => {
  const { db, env } = freshEnv();
  seed(db, 'older observation');
  seed(db, 'newest observation about widgets');

  const pinOut = runScript('memory-pin.js', ['note about widgets'], env);
  assert.match(pinOut, /Pinned observation:/);
  assert.match(pinOut, /newest observation about widgets/);

  // The most-recent observation is the one that got pinned.
  const recent = cliJson(db, ['list', 'observations', '--limit', '1']);
  const pins = cliJson(db, ['list', 'pins']);
  assert.equal(pins.length, 1);
  assert.equal(pins[0].targetId, recent[0].id);
  assert.equal(pins[0].reason, 'note about widgets');

  const pinsOut = runScript('memory-pins.js', [], env);
  assert.match(pinsOut, /Pinned Observations/);
  assert.match(pinsOut, /note about widgets/);
});

test('memory-pin parses a --ttl token into an expiry', { skip }, () => {
  const { db, env } = freshEnv();
  seed(db, 'ttl target observation');

  const before = Date.now();
  const out = runScript('memory-pin.js', ['keep me', '--ttl', '7d'], env);
  assert.match(out, /Expires:/);

  const pins = cliJson(db, ['list', 'pins']);
  assert.equal(pins.length, 1);
  // 7d = 604800000 ms; allow a generous window for execution time.
  const expected = before + 7 * 86400000;
  assert.ok(
    Math.abs(pins[0].expiresAt - expected) < 60000,
    `expiresAt ${pins[0].expiresAt} should be ~7d from now (${expected})`,
  );
  // The note must have the --ttl token stripped out.
  assert.equal(pins[0].reason, 'keep me');
});

test('memory-pin reports nothing to pin on an empty db', { skip }, () => {
  const { env } = freshEnv();
  const out = runScript('memory-pin.js', ['note'], env);
  assert.match(out, /No observations to pin yet\./);
});

test('memory-unpin removes a pin by id prefix', { skip }, () => {
  const { db, env } = freshEnv();
  seed(db, 'observation to pin then unpin');
  runScript('memory-pin.js', ['temp pin'], env);

  const pins = cliJson(db, ['list', 'pins']);
  assert.equal(pins.length, 1);
  const prefix = pins[0].id.slice(0, 8);

  const out = runScript('memory-unpin.js', [prefix], env);
  assert.match(out, /Removed pin:/);
  assert.match(out, /Remaining pins: 0/);

  const after = cliJson(db, ['list', 'pins']);
  assert.equal(after.length, 0);
});

test('memory-unpin reports not-found for an unknown prefix', { skip }, () => {
  const { db, env } = freshEnv();
  seed(db, 'an observation');
  const out = runScript('memory-unpin.js', ['nopenope'], env);
  assert.match(out, /Pin not found: nopenope/);
});

test('memory-unpin aborts on an ambiguous prefix without unpinning', { skip }, () => {
  const { db, env } = freshEnv();
  // Every pin id starts with the literal `pin_`, so that prefix matches all of
  // them — a deterministic ambiguous prefix once more than one pin exists.
  seed(db, 'first pin target');
  runScript('memory-pin.js', ['pin one'], env);
  seed(db, 'second pin target');
  runScript('memory-pin.js', ['pin two'], env);

  const before = cliJson(db, ['list', 'pins']);
  assert.equal(before.length, 2);

  const out = runScript('memory-unpin.js', ['pin_'], env);
  assert.match(out, /Ambiguous pin id/);
  assert.doesNotMatch(out, /Removed pin:/);

  // Both pins must survive.
  const after = cliJson(db, ['list', 'pins']);
  assert.equal(after.length, 2);
});

test('memory-forget redacts by prefix; search no longer finds it', { skip }, () => {
  const { db, env } = freshEnv();
  seed(db, 'sensitive observation mentioning secretword');
  seed(db, 'unrelated observation');

  // Confirm search finds it first.
  const before = runScript('memory-search.js', ['secretword'], env);
  assert.match(before, /secretword/);

  const obs = cliJson(db, ['list', 'observations']);
  const target = obs.find((o) => o.content.includes('secretword'));
  const prefix = target.id.slice(0, 8);

  const out = runScript('memory-forget.js', [prefix], env);
  assert.match(out, /Redacted observation:/);
  assert.match(out, /secretword/);

  // After redaction, search must not surface it.
  const after = runScript('memory-search.js', ['secretword'], env);
  assert.doesNotMatch(after, /Search Results/);
});

test('memory-forget reports not-found for an unknown prefix', { skip }, () => {
  const { db, env } = freshEnv();
  seed(db, 'an observation');
  const out = runScript('memory-forget.js', ['nopenope'], env);
  assert.match(out, /Observation not found: nopenope/);
});

test('memory-forget aborts on an ambiguous prefix without redacting', { skip }, () => {
  const { db, env } = freshEnv();
  // Observation ids are UUIDs (16 possible first hex chars). Seeding 17 of them
  // guarantees, by pigeonhole, that at least two share a first character — a
  // deterministic ambiguous one-char prefix.
  for (let i = 0; i < 17; i++) seed(db, 'ambiguity observation number ' + i);
  const obs = cliJson(db, ['list', 'observations']);

  const counts = new Map();
  for (const o of obs) {
    const c = String(o.id)[0];
    counts.set(c, (counts.get(c) || 0) + 1);
  }
  let prefix = null;
  for (const [c, n] of counts) {
    if (n >= 2) {
      prefix = c;
      break;
    }
  }
  assert.ok(prefix, 'expected at least two observation ids to share a first char');

  const out = runScript('memory-forget.js', [prefix], env);
  assert.match(out, /Ambiguous observation id/);
  assert.doesNotMatch(out, /Redacted observation:/);

  // Nothing must have been redacted.
  const after = cliJson(db, ['list', 'observations']);
  assert.ok(
    after.every((o) => !o.redacted),
    'no observation should be redacted after an aborted ambiguous forget',
  );
});

test('memory-forget redacts an exact id older than the first list page', { skip }, () => {
  const { db, env } = freshEnv();
  // Shrink the list window so three rows span two pages without seeding 500+.
  env.KINDLING_FORGET_PAGE_SIZE = '2';
  seed(db, 'oldest observation unique-forget-needle');
  seed(db, 'middle observation');
  seed(db, 'newest observation');

  const obs = cliJson(db, ['list', 'observations', '--limit', '10']);
  assert.equal(obs.length, 3);
  const oldest = obs[obs.length - 1];
  assert.ok(oldest.content.includes('unique-forget-needle'));

  const out = runScript('memory-forget.js', [oldest.id], env);
  assert.match(out, /Redacted observation:/);
  assert.match(out, /unique-forget-needle/);

  const after = cliJson(db, ['list', 'observations', '--limit', '10']);
  const row = after.find((o) => o.id === oldest.id);
  assert.equal(row.redacted, 1);
});

test('memory-forget treats a prefix as ambiguous when matches span list pages', { skip }, () => {
  const { db, env } = freshEnv();
  const pageSize = 2;
  env.KINDLING_FORGET_PAGE_SIZE = String(pageSize);

  // UUIDs: 16 first hex chars. 17 rows guarantee a shared prefix; extra newer
  // rows then push the older match past the first page.
  for (let i = 0; i < 17; i++) seed(db, 'cross-page observation ' + i);

  function listed() {
    return cliJson(db, ['list', 'observations', '--limit', '100']);
  }

  function matchIndices(rows, prefix) {
    return rows
      .map((o, i) => (String(o.id).startsWith(prefix) ? i : -1))
      .filter((i) => i >= 0);
  }

  let obs = listed();
  const counts = new Map();
  for (const o of obs) {
    const c = String(o.id)[0];
    counts.set(c, (counts.get(c) || 0) + 1);
  }
  let prefix = null;
  for (const [c, n] of counts) {
    if (n >= 2) {
      prefix = c;
      break;
    }
  }
  assert.ok(prefix, 'expected at least two observation ids to share a first char');

  while (true) {
    obs = listed();
    const idxs = matchIndices(obs, prefix);
    assert.ok(idxs.length >= 2, 'prefix should keep matching at least two ids');
    if (Math.max(...idxs) >= pageSize) break;
    seed(db, 'newer observation to push a match onto the next page');
  }

  const out = runScript('memory-forget.js', [prefix], env);
  assert.match(out, /Ambiguous observation id/);
  assert.doesNotMatch(out, /Redacted observation:/);

  const after = listed();
  assert.ok(
    after.every((o) => !o.redacted),
    'no observation should be redacted after an aborted cross-page ambiguous forget',
  );
});

test('scripts fail soft (exit 0) when the binary is missing', { skip }, () => {
  const { env } = freshEnv();
  const badEnv = { ...env, KINDLING_BIN: '/nonexistent/kindling-binary' };
  // execFileSync throws on non-zero exit; a clean run means exit 0.
  const out = execFileSync('node', [join(SCRIPTS, 'memory-status.js')], {
    env: badEnv,
    encoding: 'utf-8',
  });
  assert.match(out, /could not run the `kindling` binary/);
});
