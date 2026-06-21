/**
 * Unit tests for binary/platform resolution in `config.ts`.
 *
 * These are pure (no daemon, no filesystem fixtures) and cover:
 *   - the exhaustive `(platform, arch, libc)` -> package-name mapping, and
 *   - `resolveConfig().binaryPath` precedence, including the fall-through to
 *     `null` (which lets the transport spawn `kindling` from `PATH`).
 */

import { afterEach, describe, expect, it } from 'vitest';

import { platformPackageName, platformPackageNameFor, resolveConfig } from '../src/config.js';

describe('platformPackageNameFor', () => {
  it.each([
    ['linux', 'x64', 'glibc', '@eddacraft/kindling-linux-x64'],
    ['linux', 'arm64', 'glibc', '@eddacraft/kindling-linux-arm64'],
    ['linux', 'x64', 'musl', '@eddacraft/kindling-linux-x64-musl'],
    ['linux', 'arm64', 'musl', '@eddacraft/kindling-linux-arm64-musl'],
    ['darwin', 'x64', null, '@eddacraft/kindling-darwin-x64'],
    ['darwin', 'arm64', null, '@eddacraft/kindling-darwin-arm64'],
    ['win32', 'x64', null, '@eddacraft/kindling-win32-x64'],
  ] as const)('maps %s/%s/%s -> %s', (platform, arch, libc, expected) => {
    expect(platformPackageNameFor(platform, arch, libc)).toBe(expected);
  });

  it.each([
    ['linux', 'ia32', 'glibc'],
    ['linux', 'arm', 'musl'],
    ['darwin', 'ia32', null],
    ['win32', 'arm64', null], // no win-arm64 in the release matrix
    ['freebsd', 'x64', null],
  ] as const)('returns null for unsupported %s/%s', (platform, arch, libc) => {
    expect(platformPackageNameFor(platform, arch, libc)).toBeNull();
  });

  it('ignores libc off Linux (darwin never gets a -musl suffix)', () => {
    expect(platformPackageNameFor('darwin', 'arm64', 'musl')).toBe(
      '@eddacraft/kindling-darwin-arm64',
    );
  });
});

describe('platformPackageName (host)', () => {
  it('returns a scoped kindling package or null for the running host', () => {
    const name = platformPackageName();
    if (name !== null) {
      expect(name).toMatch(/^@eddacraft\/kindling-/);
    }
  });
});

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
    // In the test workspace no platform package is installed and no legacy
    // bundled `bin/kindling` exists, so resolution must reach the PATH fallback
    // sentinel (`null`) rather than returning a non-existent bundled path.
    delete process.env.KINDLING_BIN;
    expect(resolveConfig().binaryPath).toBeNull();
  });
});
