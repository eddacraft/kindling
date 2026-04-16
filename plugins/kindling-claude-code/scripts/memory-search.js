#!/usr/bin/env node
const { init, cleanup, getProjectRoot } = require('../hooks/lib/init.js');
const cwd = process.cwd();
const repoRoot = getProjectRoot(cwd);
const query = process.argv.slice(2).join(' ') || '';

const { db, service } = init(cwd);
service
  .retrieve({ query, scopeIds: { repoId: repoRoot }, maxCandidates: 15 })
  .then((results) => {
    if (results.pins && results.pins.length > 0) {
      console.log('=== Pinned Items ===');
      results.pins.forEach((p) => {
        const preview = (p.content || '').substring(0, 200).replace(/\n/g, ' ');
        console.log('  [PIN] ' + (p.note || 'Pin') + ': ' + preview);
      });
      console.log('');
    }
    if (results.candidates && results.candidates.length > 0) {
      console.log('=== Search Results ===');
      results.candidates.forEach((c, i) => {
        const e = c.entity || c;
        const ts = e.ts ? new Date(e.ts).toLocaleString() : '';
        const preview = (e.content || '').substring(0, 300).replace(/\n/g, ' ');
        console.log(i + 1 + '. [' + ts + '] ' + (e.kind || '') + ': ' + preview);
      });
    }
    if (
      (!results.pins || results.pins.length === 0) &&
      (!results.candidates || results.candidates.length === 0)
    ) {
      console.log('No results found for: ' + query);
    }
  })
  .catch((err) => console.error('Search error:', err.message))
  .finally(() => {
    cleanup(db);
  });
