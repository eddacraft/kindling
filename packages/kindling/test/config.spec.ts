/**
 * Unit tests for binary resolution in `config.ts`.
 *
 * These are pure (no daemon, no filesystem fixtures) and cover
 * `resolveConfig().binaryPath` precedence, including the fall-through to
 * `null` (which lets the transport spawn `kindling` from `PATH`).
 */

import { afterEach, describe, expect, it } from 'vitest';

import { resolveConfig } from '../src/config.js';

describe('resolveConfig().binaryPath', () => {
  const original = process.env.KINDLING_BIN;
  afterEach(() => {
    if (original === undefined) delete process.env.KINDLING_BIN;
    else process.env.KINDLING_BIN = original;
  });

  it('prefers an explicit binaryPath option over everything', () => {
    process.env.KINDLING_BIN = '/env/kindling';
    expect(resolveConfig({ binaryPath: '/explicit/kindling' }).binaryPath).toBe(
      '/explicit/kindling',
    );
  });

  it('uses $KINDLING_BIN when no explicit option is given', () => {
    process.env.KINDLING_BIN = '/env/kindling';
    expect(resolveConfig().binaryPath).toBe('/env/kindling');
  });

  it('falls through to null when no binary can be resolved', () => {
    // In the test workspace no bundled `bin/kindling` exists, so resolution must reach the PATH fallback
    // sentinel (`null`) rather than returning a non-existent bundled path.
    delete process.env.KINDLING_BIN;
    expect(resolveConfig().binaryPath).toBeNull();
  });
});
