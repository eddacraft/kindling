/**
 * Consumer round-trip test for the PUBLISHED hook-payload fixtures.
 *
 * Adapter authors import `@eddacraft/kindling/fixtures` to test their capture
 * pipelines against real engine output instead of copying doc examples. This
 * test plays that consumer: it loads the shipped JSON, validates every case's
 * shape against the published domain types (ObservationKind, ScopeIds), and
 * asserts the published cases have NOT drifted from the internal source of truth
 * (`crates/kindling/tests/fixtures/capture-cases.json`).
 *
 * Fast and offline: pure JSON imports, no daemon, no clock.
 */

import { describe, expect, it } from 'vitest';

import type { ObservationKind, ScopeIds } from '../src/generated/index.js';

// The published, versioned artifact (the npm-shipped copy under the package).
import fixtures from '../fixtures/hook-payloads/claude-code.json';
// The internal source of truth the published fixtures are derived from.
import internalCases from '../../../crates/kindling/tests/fixtures/capture-cases.json';

// Runtime allow-list of observation kinds, kept in lock-step with the generated
// `ObservationKind` union: `satisfies` makes an invalid literal a compile error,
// so this list cannot silently diverge from the published type.
const OBSERVATION_KINDS = [
  'tool_call',
  'command',
  'file_diff',
  'error',
  'message',
  'node_start',
  'node_end',
  'node_output',
  'node_error',
] as const satisfies readonly ObservationKind[];

// Every key ScopeIds may carry. `satisfies` ties this to the published type.
const SCOPE_KEYS = [
  'sessionId',
  'repoId',
  'agentId',
  'userId',
  'taskId',
] as const satisfies readonly (keyof ScopeIds)[];

interface FixtureCase {
  name: string;
  hookType: string;
  hookInput: Record<string, unknown>;
  expected: {
    kind: string;
    content: string;
    provenance?: Record<string, unknown>;
    scopeIds: Record<string, unknown>;
  };
}

const cases = fixtures.cases as FixtureCase[];

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

describe('published hook-payload fixtures', () => {
  it('carries a pinnable version and adapter identity', () => {
    expect(typeof fixtures.version).toBe('string');
    expect(fixtures.version).not.toBe('');
    expect(fixtures.adapter).toBe('claude-code');
    expect(fixtures.source).toBe('crates/kindling/tests/fixtures/capture-cases.json');
  });

  it('ships a non-empty case list', () => {
    expect(Array.isArray(cases)).toBe(true);
    expect(cases.length).toBeGreaterThan(0);
  });

  it.each(cases.map((c) => [c.name, c] as const))('case %s has a valid envelope', (_name, c) => {
    expect(typeof c.name).toBe('string');
    expect(typeof c.hookType).toBe('string');
    expect(isPlainObject(c.hookInput)).toBe(true);
    expect(isPlainObject(c.expected)).toBe(true);
  });

  it.each(cases.map((c) => [c.name, c] as const))(
    'case %s expected observation matches the published types',
    (_name, c) => {
      const { kind, content, provenance, scopeIds } = c.expected;

      // kind is a published ObservationKind
      expect(OBSERVATION_KINDS as readonly string[]).toContain(kind);

      // content is always a string
      expect(typeof content).toBe('string');

      // provenance, when present, is a plain JSON object
      if (provenance !== undefined) {
        expect(isPlainObject(provenance)).toBe(true);
      }

      // scopeIds uses only known dimensions and string values
      expect(isPlainObject(scopeIds)).toBe(true);
      for (const [key, value] of Object.entries(scopeIds)) {
        expect(SCOPE_KEYS as readonly string[]).toContain(key);
        expect(typeof value).toBe('string');
      }
    },
  );

  it.each(cases.map((c) => [c.name, c] as const))(
    'case %s survives a JSON round-trip unchanged',
    (_name, c) => {
      expect(JSON.parse(JSON.stringify(c))).toEqual(c);
    },
  );

  it('has not drifted from the internal capture-mapping source', () => {
    // The published `cases` must be byte-equal (structurally) to the internal
    // source. If this fails, run `pnpm --filter @eddacraft/kindling sync-fixtures`.
    expect(cases).toEqual(internalCases);
  });
});
