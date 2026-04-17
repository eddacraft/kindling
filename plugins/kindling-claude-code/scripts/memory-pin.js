#!/usr/bin/env node
const { init, cleanup, getProjectRoot } = require('../hooks/lib/init.js');
const { randomUUID } = require('crypto');
const cwd = process.cwd();
const repoRoot = getProjectRoot(cwd);
const args = process.argv.slice(2).join(' ');

// Parse TTL if present (e.g., '7d', '24h', '30m')
let note = args;
let ttlMs = null;
const ttlMatch = args.match(/--ttl\s+(\d+)([dhm])/);
if (ttlMatch) {
  const val = parseInt(ttlMatch[1]);
  const unit = ttlMatch[2];
  const multipliers = { d: 86400000, h: 3600000, m: 60000 };
  ttlMs = val * multipliers[unit];
  note = args.replace(/--ttl\s+\d+[dhm]/, '').trim();
}
if (!note) note = 'Pinned observation';

const { db, store } = init(cwd);
try {
  const lastObs = db
    .prepare('SELECT * FROM observations WHERE repo_id = ? ORDER BY ts DESC LIMIT 1')
    .get(repoRoot);
  if (!lastObs) {
    console.log('No observations to pin yet.');
    process.exit(0);
  }

  const pin = {
    id: randomUUID(),
    targetType: 'observation',
    targetId: lastObs.id,
    note: note,
    createdAt: Date.now(),
    expiresAt: ttlMs ? Date.now() + ttlMs : null,
    scopeIds: { repoId: repoRoot },
  };

  store.insertPin(pin);

  const preview = (lastObs.content || '').substring(0, 100).replace(/\n/g, ' ');
  console.log('Pinned observation:');
  console.log('  Kind: ' + lastObs.kind);
  console.log('  Note: ' + note);
  console.log('  Content: ' + preview + '...');
  if (ttlMs) console.log('  Expires: ' + new Date(pin.expiresAt).toLocaleString());
  console.log('');
  console.log('Use /memory pins to see all pinned items.');
} finally {
  cleanup(db);
}
