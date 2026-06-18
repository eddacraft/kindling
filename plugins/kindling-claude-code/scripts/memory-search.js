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

if (result.pins && result.pins.length > 0) {
  console.log('=== Pinned Items ===');
  result.pins.forEach((p) => {
    const entity = (p && p.target) || {};
    const note = (p && p.pin && p.pin.reason) || 'Pin';
    const preview = (entity.content || '').substring(0, 200).replace(/\n/g, ' ');
    console.log('  [PIN] ' + note + ': ' + preview);
  });
  console.log('');
}

if (result.candidates && result.candidates.length > 0) {
  console.log('=== Search Results ===');
  result.candidates.forEach((c, i) => {
    const e = c.entity || {};
    const ts = e.ts ? new Date(e.ts).toLocaleString() : '';
    const preview = (e.content || '').substring(0, 300).replace(/\n/g, ' ');
    console.log(i + 1 + '. [' + ts + '] ' + (e.kind || '') + ': ' + preview);
  });
}

if (
  (!result.pins || result.pins.length === 0) &&
  (!result.candidates || result.candidates.length === 0)
) {
  console.log('No results found for: ' + query);
}
