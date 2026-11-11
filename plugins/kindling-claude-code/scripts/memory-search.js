#!/usr/bin/env node
const { dbPath, runJson } = require('./lib/kindling.js');

const cwd = process.cwd();
const db = dbPath(cwd);
const query = process.argv.slice(2).join(' ') || '';

// `kindling search <query> --db <db> --json` → full RetrieveResult (camelCase):
//   { pins: [{ pin, target }], candidates: [{ entity, score, matchContext }], ... }
//
// No `--repo` scope filter: the database is already per-project-isolated by its
// path (sha256 of the project root), and the capture hooks store `repoId` as the
// raw cwd, so a git-toplevel `--repo` filter would miss observations captured
// from a subdirectory. Searching the whole project DB is both simpler and more
// faithful to "the memory for this project".
const result = runJson(['search', query, '--db', db, '--max', '15', '--json']);

const hasPins = result.pins && result.pins.length > 0;
const hasCandidates = result.candidates && result.candidates.length > 0;

if (hasPins) {
  console.log('=== Pinned Items ===');
  result.pins.forEach((p) => {
    const entity = (p && p.target) || {};
    const note = (p && p.pin && p.pin.reason) || 'Pin';
    const preview = (entity.content || '').substring(0, 200).replace(/\n/g, ' ');
    const id = entity.id ? String(entity.id) : '';
    console.log('  [PIN] [' + id + '] ' + note + ': ' + preview);
  });
  console.log('');
}

if (hasCandidates) {
  console.log('=== Search Results ===');
  result.candidates.forEach((c, i) => {
    const e = c.entity || {};
    const ts = e.ts ? new Date(e.ts).toLocaleString() : '';
    const preview = (e.content || '').substring(0, 300).replace(/\n/g, ' ');
    const id = e.id ? String(e.id) : '';
    console.log(i + 1 + '. [' + id + '] [' + ts + '] ' + (e.kind || '') + ': ' + preview);
  });
}

if (!hasPins && !hasCandidates) {
  console.log('No results found for: ' + query);
} else {
  console.log('');
  console.log('Use /memory forget <id> to redact an observation.');
}
