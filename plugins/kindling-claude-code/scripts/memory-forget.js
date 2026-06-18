#!/usr/bin/env node
const { dbPath, runJson } = require('./lib/kindling.js');

const cwd = process.cwd();
const db = dbPath(cwd);
const obsId = process.argv[2] || '';

if (!obsId) {
  console.log('Usage: /memory forget <observation-id>');
  console.log('Find observation IDs using /memory search');
  process.exit(0);
}

// Prefix-resolve the observation id. `kindling forget` takes an exact id, so we
// scan `list observations --json` (raw rows: { id, kind, content, ... }) for the
// first id that starts with the supplied prefix.
const observations = runJson([
  'list',
  'observations',
  '--db',
  db,
  '--limit',
  '500',
  '--json',
]);
const obs = (observations || []).find((o) => String(o.id).startsWith(obsId));

if (!obs) {
  console.log('Observation not found: ' + obsId);
  process.exit(0);
}

runJson(['forget', obs.id, '--db', db, '--json']);

const preview = (obs.content || '').substring(0, 100).replace(/\n/g, ' ');
console.log('Redacted observation:');
console.log('  ID: ' + String(obs.id).substring(0, 8));
console.log('  Kind: ' + obs.kind);
console.log('  Content: ' + preview + '...');
console.log('');
console.log('This observation has been removed from search results.');
