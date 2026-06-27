#!/usr/bin/env node
// Derive the PUBLIC, versioned hook-payload fixtures from the internal source.
//
// The engine's source of truth for Claude Code capture mapping is
// `crates/kindling/tests/fixtures/capture-cases.json` (generated from the REAL
// Node adapter and asserted byte-for-byte against the Rust hook by
// `crates/kindling/tests/capture_mapping.rs`). Adapter authors should NOT copy
// doc examples; they should test against a published, pinnable artifact derived
// from that same source so it can never drift from the engine's real behaviour.
//
// This script wraps the internal cases in a versioned envelope and writes two
// identical copies:
//
//   1. `fixtures/hook-payloads/claude-code.json` — the canonical, repo-root,
//      publicly discoverable artifact (parallels the top-level `schema/`).
//   2. `packages/kindling/fixtures/hook-payloads/claude-code.json` — the vendored
//      copy npm actually ships (npm only includes files under the package dir),
//      exposed via the `@eddacraft/kindling` `./fixtures` export.
//
// Usage:
//   node packages/kindling/scripts/sync-fixtures.mjs          # write
//   node packages/kindling/scripts/sync-fixtures.mjs --check  # verify, no write
//
// The `--check` mode (and the `fixtures.spec.ts` drift test) fail on any drift,
// the same guarantee `sync-types.mjs` gives the generated bindings. Deterministic
// and offline: no network, no clock, stable key order.

import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const packageRoot = join(here, '..');
const repoRoot = join(packageRoot, '..', '..');

// Bump when the ENVELOPE shape changes in a breaking way. Consumers pin on this.
const FIXTURE_VERSION = '1.0';

const sourceRel = 'crates/kindling/tests/fixtures/capture-cases.json';
const sourcePath = join(repoRoot, sourceRel);

const outputs = [
  join(repoRoot, 'fixtures', 'hook-payloads', 'claude-code.json'),
  join(packageRoot, 'fixtures', 'hook-payloads', 'claude-code.json'),
];

/** Build the deterministic published payload string from the internal source. */
async function buildPayload() {
  if (!existsSync(sourcePath)) {
    console.error(`error: internal fixtures source missing: ${sourcePath}`);
    process.exit(1);
  }
  const cases = JSON.parse(await readFile(sourcePath, 'utf8'));
  const payload = {
    version: FIXTURE_VERSION,
    description:
      'Public hook-payload fixtures for kindling adapter authors. Each case is a ' +
      'captured hook input (snake_case stdin shape) and the exact observation the ' +
      'engine produces. Derived from the internal capture-mapping source; do not edit by hand.',
    adapter: 'claude-code',
    source: sourceRel,
    generator: 'packages/kindling/scripts/sync-fixtures.mjs',
    cases,
  };
  return JSON.stringify(payload, null, 2) + '\n';
}

async function main() {
  const check = process.argv.includes('--check');
  const expected = await buildPayload();

  if (check) {
    // Compare STRUCTURALLY (whitespace-independent): the committed copies are
    // reformatted by the repo formatter (oxfmt), so only the data is asserted.
    // Re-stringify both through a compact serializer — key order is preserved by
    // JSON.parse, so this stays a strict, order-sensitive content check.
    const expectedCompact = JSON.stringify(JSON.parse(expected));
    let drift = false;
    for (const out of outputs) {
      const rel = relative(repoRoot, out);
      if (!existsSync(out)) {
        console.error(`drift: missing ${rel} (run \`node ${relative(repoRoot, fileURLToPath(import.meta.url))}\`)`);
        drift = true;
        continue;
      }
      const actualCompact = JSON.stringify(JSON.parse(await readFile(out, 'utf8')));
      if (actualCompact !== expectedCompact) {
        console.error(`drift: ${rel} is out of date with ${sourceRel}`);
        drift = true;
      }
    }
    if (drift) process.exit(1);
    console.log(`fixtures up to date with ${sourceRel}`);
    return;
  }

  for (const out of outputs) {
    await mkdir(dirname(out), { recursive: true });
    await writeFile(out, expected, 'utf8');
    console.log(`wrote ${relative(repoRoot, out)}`);
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
