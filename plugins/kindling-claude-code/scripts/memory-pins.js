#!/usr/bin/env node
const { dbPath, runJson } = require('./lib/kindling.js');

const cwd = process.cwd();
const db = dbPath(cwd);

// `kindling list pins --db <db> --json` → typed Pin[] (camelCase):
//   { id, targetType, targetId, reason, createdAt, expiresAt, scopeIds }
// The CLI prunes expired pins, so no client-side `now` filtering is needed.
// The DB is per-project-isolated by path, so no `--repo` scope filter is used.
const pins = runJson(['list', 'pins', '--db', db, '--json']);

if (!pins || pins.length === 0) {
  console.log('No pins yet. Use /memory pin to pin important observations.');
  process.exit(0);
}

console.log('=== Pinned Observations ===');
console.log('');

pins.forEach((pin, i) => {
  const date = pin.createdAt ? new Date(pin.createdAt).toLocaleDateString() : '';
  console.log(i + 1 + '. [' + date + '] ' + (pin.reason || 'Pin'));
  console.log('   ID: ' + String(pin.id).substring(0, 8));
  // `list pins` does not include the target's content; show the target id.
  console.log('   Target: ' + String(pin.targetId).substring(0, 8));
  if (pin.expiresAt) {
    console.log('   Expires: ' + new Date(pin.expiresAt).toLocaleString());
  }
  console.log('');
});

console.log('Use /memory unpin <id> to remove a pin.');
