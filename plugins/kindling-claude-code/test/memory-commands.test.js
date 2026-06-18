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
const { mkdtempSync, existsSync } = require('node:fs');
const { tmpdir } = require('node:os');
const { join } = require('node:path');

const SCRIPTS = join(__dirname, '..', 'scripts');

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
