/**
 * Unit tests for config resolution in `config.ts`.
 *
 * These are pure (no daemon, no filesystem fixtures) and cover
 * `resolveConfig().binaryPath` precedence, including the fall-through to
 * `null` (which lets the transport spawn `kindling` from `PATH`), and
 * `resolveProjectRoot`'s `KINDLING_REPO_ROOT` containment check.
 */

import { afterEach, describe, expect, it } from 'vitest';

import { resolveConfig, resolveProjectRoot } from '../src/config.js';

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

describe('resolveProjectRoot KINDLING_REPO_ROOT', () => {
  const original = process.env.KINDLING_REPO_ROOT;
  afterEach(() => {
    if (original === undefined) delete process.env.KINDLING_REPO_ROOT;
    else process.env.KINDLING_REPO_ROOT = original;
  });

  it('honours the override for the exact root and a child path', () => {
    process.env.KINDLING_REPO_ROOT = '/work/repo';
    expect(resolveProjectRoot('/work/repo')).toBe('/work/repo');
    expect(resolveProjectRoot('/work/repo/src')).toBe('/work/repo');
  });

  it('rejects a sibling directory that shares a string prefix', () => {
    process.env.KINDLING_REPO_ROOT = '/work/repo';
    expect(resolveProjectRoot('/work/repo-copy')).not.toBe('/work/repo');
  });

  it('honours a relative override when cwd is under the resolved root', () => {
    process.env.KINDLING_REPO_ROOT = 'repo';
    expect(resolveProjectRoot('repo/src')).toBe('repo');
    process.env.KINDLING_REPO_ROOT = './repo';
    expect(resolveProjectRoot('repo/src')).toBe('./repo');
  });

  it('rejects a relative sibling that shares a string prefix', () => {
    process.env.KINDLING_REPO_ROOT = 'repo';
    expect(resolveProjectRoot('repo-copy')).not.toBe('repo');
  });
});
