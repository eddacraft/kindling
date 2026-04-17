#!/usr/bin/env node
const { init, cleanup, getProjectRoot } = require('../hooks/lib/init.js');
const cwd = process.cwd();
const repoRoot = getProjectRoot(cwd);
const obsId = process.argv[2] || '';

if (!obsId) {
  console.log('Usage: /memory forget <observation-id>');
  console.log('Find observation IDs using /memory search');
  process.exit(0);
}

const { db, store } = init(cwd);
try {
  const obs = db
    .prepare('SELECT id, kind, content FROM observations WHERE repo_id = ? AND id LIKE ? LIMIT 1')
    .get(repoRoot, obsId + '%');

  if (!obs) {
    console.log('Observation not found: ' + obsId);
    process.exit(0);
  }

  store.redactObservation(obs.id);

  const preview = (obs.content || '').substring(0, 100).replace(/\n/g, ' ');
  console.log('Redacted observation:');
  console.log('  ID: ' + obs.id.substring(0, 8));
  console.log('  Kind: ' + obs.kind);
  console.log('  Content: ' + preview + '...');
  console.log('');
  console.log('This observation has been removed from search results.');
} finally {
  cleanup(db);
}
