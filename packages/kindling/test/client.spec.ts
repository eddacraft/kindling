/**
 * Integration tests for the thin {@link Kindling} client against a REAL daemon.
 *
 * The daemon is the debug `kindling` binary built from the workspace
 * (`cargo build -p kindling --bin kindling` → `target/debug/kindling`). Each
 * test gets a fresh temp `KINDLING_HOME` and a unique socket path, and relies on
 * the client's own auto-spawn (pointed at the built binary via `binaryPath`) to
 * start the daemon. No native Node modules are involved.
 */

import { mkdtempSync, existsSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { afterEach, describe, expect, it } from 'vitest';

import { Kindling } from '../src/index.js';
import { ApiError, SchemaMismatchError } from '../src/errors.js';

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(here, '..', '..', '..');
const BINARY = join(repoRoot, 'target', 'debug', 'kindling');

/** A fixed project root so DB routing is deterministic per test. */
const PROJECT_ROOT = '/tmp/kindling-thin-client-test/repo';

/** Track temp homes for cleanup. */
const tempHomes: string[] = [];

/** Make a fresh temp home + unique socket path and a client wired to them. */
function freshClient(overrides: Record<string, unknown> = {}) {
  const home = mkdtempSync(join(tmpdir(), 'kindling-thin-'));
  tempHomes.push(home);
  const socketPath = join(home, 'kindling.sock');
  const client = new Kindling({
    socketPath,
    projectRoot: PROJECT_ROOT,
    binaryPath: BINARY,
    connectTimeoutMs: 5000,
    ...overrides,
  });
  return { client, socketPath, home };
}

/**
 * The daemon integration tests require the built binary. When it is absent
 * (e.g. the monorepo's TS-only `pnpm -r test` job, which does not build Rust),
 * they SKIP rather than fail — a dedicated CI job builds the binary and runs
 * them. Build locally with `cargo build -p kindling --bin kindling`.
 */
const HAS_BINARY = existsSync(BINARY);
if (!HAS_BINARY) {
  console.warn(
    `[kindling] daemon binary not found at ${BINARY} — skipping live-daemon ` +
      `integration tests. Build with: cargo build -p kindling --bin kindling`,
  );
}

afterEach(() => {
  for (const home of tempHomes.splice(0)) {
    rmSync(home, { recursive: true, force: true });
  }
});

describe.skipIf(!HAS_BINARY)('Kindling thin client — warm round-trip', () => {
  it('exercises every method against a live daemon', async () => {
    const { client } = freshClient();

    // health() (also schema-version check) — first call cold-spawns the daemon.
    const health = await client.health();
    expect(typeof health.version).toBe('string');
    expect(typeof health.schemaVersion).toBe('number');

    // openCapsule
    const capsule = await client.openCapsule({
      kind: 'session',
      intent: 'thin-client round trip',
      scopeIds: { sessionId: 's1', repoId: PROJECT_ROOT },
    });
    expect(capsule.id).toBeTruthy();
    expect(capsule.type).toBe('session');
    expect(capsule.status).toBe('open');

    // getOpenCapsule resolves the same capsule by session id
    const open = await client.getOpenCapsule('s1');
    expect(open?.id).toBe(capsule.id);

    // appendObservation (attached to the capsule, with the project header)
    const observation = await client.appendObservation(
      {
        kind: 'message',
        content: 'thin client searchable needle',
        scopeIds: { sessionId: 's1', repoId: PROJECT_ROOT },
      },
      { capsuleId: capsule.id },
    );
    expect(observation.id).toBeTruthy();
    expect(observation.content).toContain('needle');

    // retrieve surfaces the observation
    const result = await client.retrieve({
      query: 'needle',
      scopeIds: { sessionId: 's1', repoId: PROJECT_ROOT },
    });
    const hit = result.candidates.some((c) => c.entity.content.includes('needle'));
    expect(hit).toBe(true);

    // pin then unpin the observation
    const pin = await client.pin({
      targetType: 'observation',
      targetId: observation.id,
      note: 'keep me',
    });
    expect(pin.id).toBeTruthy();
    expect(pin.targetId).toBe(observation.id);
    await expect(client.unpin(pin.id)).resolves.toBeUndefined();

    // forget — append a fresh observation, redact it, and confirm it no longer
    // surfaces in retrieval.
    const forgettable = await client.appendObservation(
      {
        kind: 'message',
        content: 'thin client forgettable phrase',
        scopeIds: { sessionId: 's1', repoId: PROJECT_ROOT },
      },
      { capsuleId: capsule.id },
    );
    const beforeForget = await client.retrieve({
      query: 'forgettable',
      scopeIds: { sessionId: 's1', repoId: PROJECT_ROOT },
    });
    expect(beforeForget.candidates.some((c) => c.entity.id === forgettable.id)).toBe(true);

    await expect(client.forget(forgettable.id)).resolves.toBeUndefined();

    const afterForget = await client.retrieve({
      query: 'forgettable',
      scopeIds: { sessionId: 's1', repoId: PROJECT_ROOT },
    });
    expect(afterForget.candidates.some((c) => c.entity.id === forgettable.id)).toBe(false);

    // forgetting an unknown id → ApiError 404
    let forgetErr: unknown;
    try {
      await client.forget('does-not-exist');
    } catch (err) {
      forgetErr = err;
    }
    expect(forgetErr).toBeInstanceOf(ApiError);
    expect((forgetErr as ApiError).status).toBe(404);

    // closeCapsule
    const closed = await client.closeCapsule(capsule.id);
    expect(closed.id).toBe(capsule.id);
    expect(closed.status).toBe('closed');
  });
});

describe.skipIf(!HAS_BINARY)('Kindling thin client — auto-spawn from clean state', () => {
  it('cold-spawns the daemon on first call and succeeds', async () => {
    const { client, socketPath } = freshClient();
    expect(existsSync(socketPath)).toBe(false);

    const t0 = performance.now();
    const health = await client.health();
    const coldMs = performance.now() - t0;

    expect(health.schemaVersion).toBeGreaterThan(0);
    expect(existsSync(socketPath)).toBe(true);
    // Logged so the report can cite a real measured number.
    console.log(`[auto-spawn] cold latency: ${coldMs.toFixed(1)}ms`);
  });
});

describe.skipIf(!HAS_BINARY)('Kindling thin client — schema check', () => {
  it('throws SchemaMismatchError on a wrong expectedSchemaVersion', async () => {
    const { client } = freshClient({ expectedSchemaVersion: 999 });
    await expect(client.health()).rejects.toBeInstanceOf(SchemaMismatchError);
  });
});

describe.skipIf(!HAS_BINARY)('Kindling thin client — error mapping', () => {
  it('maps closing a nonexistent capsule to ApiError 404', async () => {
    const { client } = freshClient();
    // Warm the daemon first so this is purely an API error, not a spawn error.
    await client.health();
    let caught: unknown;
    try {
      await client.closeCapsule('does-not-exist');
    } catch (err) {
      caught = err;
    }
    expect(caught).toBeInstanceOf(ApiError);
    expect((caught as ApiError).status).toBe(404);
  });
});

describe('@eddacraft/kindling package — no native modules', () => {
  it('declares zero native dependencies', () => {
    const pkg = JSON.parse(readFileSync(join(here, '..', 'package.json'), 'utf8')) as {
      dependencies?: Record<string, string>;
      devDependencies?: Record<string, string>;
    };
    const deps = { ...(pkg.dependencies ?? {}), ...(pkg.devDependencies ?? {}) };
    const native = ['better-sqlite3', 'sqlite3', 'node-gyp', 'bindings', 'sql.js'];
    for (const name of native) {
      expect(deps[name], `${name} must not be a dependency`).toBeUndefined();
    }
    // There must be no runtime dependencies at all.
    expect(Object.keys(pkg.dependencies ?? {})).toHaveLength(0);
  });
});
