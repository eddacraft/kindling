/**
 * Tests for the OpenCode memory commands.
 *
 * Commands backed by the daemon (`search`, `pin`, `status`) run against a REAL
 * `kindling` binary (`cargo build -p kindling --bin kindling`) via the thin
 * {@link Kindling} client, and SKIP when the binary is absent. Commands the
 * daemon does not support (`forget`, `export`) are now stubs and are asserted
 * as pure functions with no daemon.
 */

import { existsSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, it, expect, afterEach } from 'vitest';

import { Kindling } from '@eddacraft/kindling';
import { memoryStatus, formatStatus } from '../src/opencode/commands/status.js';
import { memorySearch, formatSearchResults } from '../src/opencode/commands/search.js';
import { memoryPin, formatPinResult } from '../src/opencode/commands/pin.js';
import { memoryForget, formatForgetResult } from '../src/opencode/commands/forget.js';
import { memoryExport, formatExportResult } from '../src/opencode/commands/export.js';
import type { RetrieveResult } from '@eddacraft/kindling';

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(here, '..', '..', '..');
const BINARY = join(repoRoot, 'target', 'debug', 'kindling');
const HAS_BINARY = existsSync(BINARY);

if (!HAS_BINARY) {
  console.warn(
    `[opencode-adapter] daemon binary not found at ${BINARY} — skipping live-daemon ` +
      `command tests. Build with: cargo build -p kindling --bin kindling`,
  );
}

const PROJECT_ROOT = '/tmp/kindling-opencode-cmd-test/repo';
const tempHomes: string[] = [];

function freshClient(): Kindling {
  const home = mkdtempSync(join(tmpdir(), 'kindling-occmd-'));
  tempHomes.push(home);
  return new Kindling({
    socketPath: join(home, 'kindling.sock'),
    projectRoot: PROJECT_ROOT,
    binaryPath: BINARY,
    connectTimeoutMs: 5000,
  });
}

afterEach(() => {
  for (const home of tempHomes.splice(0)) {
    rmSync(home, { recursive: true, force: true });
  }
});

describe.skipIf(!HAS_BINARY)('/memory status (live daemon)', () => {
  it('reports daemon health', async () => {
    const client = freshClient();
    const result = await memoryStatus(client);
    expect(typeof result.version).toBe('string');
    expect(result.schemaVersion).toBeGreaterThan(0);
    expect(Array.isArray(result.projects)).toBe(true);
  });

  it('formats status as readable text', () => {
    const formatted = formatStatus({
      version: '0.2.0',
      schemaVersion: 3,
      projects: ['/repo/a'],
    });
    expect(formatted).toContain('Memory Status');
    expect(formatted).toContain('Daemon version: 0.2.0');
    expect(formatted).toContain('Schema version: 3');
    expect(formatted).toContain('/repo/a');
  });
});

describe.skipIf(!HAS_BINARY)('/memory search (live daemon)', () => {
  it('executes a search and returns results', async () => {
    const client = freshClient();
    const capsule = await client.openCapsule({
      kind: 'session',
      intent: 'search test',
      scopeIds: { sessionId: 's1', repoId: PROJECT_ROOT },
    });
    await client.appendObservation(
      {
        kind: 'message',
        content: 'distinctsearchtoken in a message',
        scopeIds: { sessionId: 's1', repoId: PROJECT_ROOT },
      },
      { capsuleId: capsule.id },
    );

    const result = await memorySearch(client, {
      query: 'distinctsearchtoken',
      scopeIds: { sessionId: 's1', repoId: PROJECT_ROOT },
    });

    expect(result.provenance.query).toBe('distinctsearchtoken');
    expect(
      result.candidates.some((c) => c.entity.content.includes('distinctsearchtoken')),
    ).toBe(true);
  });

  it('formats search results as readable text', () => {
    const result: RetrieveResult = {
      pins: [
        {
          pin: {
            id: 'pin-1',
            targetType: 'observation',
            targetId: 'obs-1',
            reason: 'Important',
            createdAt: 1000,
            scopeIds: { sessionId: 's1' },
          },
          target: {
            id: 'obs-1',
            kind: 'message',
            content: 'Pinned message',
            provenance: {},
            ts: 1000,
            scopeIds: { sessionId: 's1' },
            redacted: false,
          },
        },
      ],
      currentSummary: {
        id: 'sum-1',
        capsuleId: 'cap-1',
        content: 'Session summary',
        confidence: 0.95,
        createdAt: 2000,
        evidenceRefs: [],
      },
      candidates: [
        {
          entity: {
            id: 'obs-2',
            kind: 'message',
            content: 'Search result',
            provenance: {},
            ts: 3000,
            scopeIds: { sessionId: 's1' },
            redacted: false,
          },
          score: 0.8,
          matchContext: 'exact match',
        },
      ],
      provenance: {
        query: 'test',
        scopeIds: { sessionId: 's1' },
        totalCandidates: 1,
        returnedCandidates: 1,
        truncatedDueToTokenBudget: false,
        providerUsed: 'local-fts',
      },
    };

    const formatted = formatSearchResults(result);
    expect(formatted).toContain('Search Results');
    expect(formatted).toContain('Query: "test"');
    expect(formatted).toContain('Pinned message');
    expect(formatted).toContain('Reason: Important');
    expect(formatted).toContain('Session summary');
    expect(formatted).toContain('Search result');
  });
});

describe.skipIf(!HAS_BINARY)('/memory pin (live daemon)', () => {
  it('pins an observation successfully', async () => {
    const client = freshClient();
    const capsule = await client.openCapsule({
      kind: 'session',
      intent: 'pin test',
      scopeIds: { sessionId: 's1', repoId: PROJECT_ROOT },
    });
    const obs = await client.appendObservation(
      {
        kind: 'message',
        content: 'pin me',
        scopeIds: { sessionId: 's1', repoId: PROJECT_ROOT },
      },
      { capsuleId: capsule.id },
    );

    const result = await memoryPin(client, {
      targetType: 'observation',
      targetId: obs.id,
      reason: 'Important finding',
      scopeIds: { sessionId: 's1', repoId: PROJECT_ROOT },
    });

    expect(result.created).toBe(true);
    expect(result.targetId).toBe(obs.id);
    expect(result.pinId).toBeTruthy();
  });

  it('formats pin result', () => {
    const success = formatPinResult({
      pinId: 'pin-1',
      targetId: 'obs-1',
      targetType: 'observation',
      created: true,
    });
    expect(success).toContain('Pinned observation obs-1');

    const failure = formatPinResult({
      pinId: '',
      targetId: 'obs-1',
      targetType: 'observation',
      created: false,
      error: 'Not found',
    });
    expect(failure).toContain('Failed to pin');
  });
});

describe('/memory pin — error mapping (no daemon)', () => {
  it('maps a thrown client error to a failed result', async () => {
    const throwingService = {
      pin: async () => {
        throw new Error('daemon returned 404: target not found');
      },
    };

    const result = await memoryPin(throwingService, {
      targetType: 'observation',
      targetId: 'obs-x',
    });

    expect(result.created).toBe(false);
    expect(result.error).toContain('target not found');
  });
});

describe('/memory forget (stub — daemon has no redaction endpoint)', () => {
  it('always reports unsupported', () => {
    const result = memoryForget({ observationId: 'obs-1' });
    expect(result.redacted).toBe(false);
    expect(result.error).toContain('not supported');
  });

  it('formats the unsupported result as a failure', () => {
    const result = memoryForget({ observationId: 'obs-1' });
    const formatted = formatForgetResult(result);
    expect(formatted).toContain('Failed to redact');
  });
});

describe('/memory export (stub — daemon has no export endpoint)', () => {
  it('always reports unsupported', () => {
    const result = memoryExport({ outputPath: '/tmp/x.json' });
    expect(result.filePath).toBe('');
    expect(result.error).toContain('not supported');
  });

  it('formats the unsupported result as a failure', () => {
    const result = memoryExport();
    const formatted = formatExportResult(result);
    expect(formatted).toContain('Export failed');
  });
});
