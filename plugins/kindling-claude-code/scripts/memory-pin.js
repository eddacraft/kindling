#!/usr/bin/env node
const { dbPath, runJson } = require('./lib/kindling.js');

const cwd = process.cwd();
const db = dbPath(cwd);
const args = process.argv.slice(2).join(' ');

// Parse TTL if present (e.g., '7d', '24h', '30m').
let note = args;
let ttlMs = null;
const ttlMatch = args.match(/--ttl\s+(\d+)([dhm])/);
if (ttlMatch) {
  const val = parseInt(ttlMatch[1], 10);
  const unit = ttlMatch[2];
  const multipliers = { d: 86400000, h: 3600000, m: 60000 };
  ttlMs = val * multipliers[unit];
  note = args.replace(/--ttl\s+\d+[dhm]/, '').trim();
}
if (!note) note = 'Pinned observation';

// Resolve the most-recent observation: `kindling list observations --limit 1`.
// Raw rows (snake_case): { id, kind, content, ts, scope_ids, redacted }.
const recent = runJson(['list', 'observations', '--db', db, '--limit', '1', '--json']);

const lastObs = recent && recent[0];
if (!lastObs) {
  console.log('No observations to pin yet.');
  process.exit(0);
}

// Pin it: `kindling pin observation <id> --note <s> [--ttl <ms>] --json`.
const pinArgs = ['pin', 'observation', lastObs.id, '--db', db, '--note', note];
if (ttlMs) {
  pinArgs.push('--ttl', String(ttlMs));
}
pinArgs.push('--json');
const pin = runJson(pinArgs);

const preview = (lastObs.content || '').substring(0, 100).replace(/\n/g, ' ');
console.log('Pinned observation:');
console.log('  Kind: ' + lastObs.kind);
console.log('  Note: ' + note);
console.log('  Content: ' + preview + '...');
if (pin && pin.expiresAt) {
  console.log('  Expires: ' + new Date(pin.expiresAt).toLocaleString());
}
console.log('');
console.log('Use /memory pins to see all pinned items.');
