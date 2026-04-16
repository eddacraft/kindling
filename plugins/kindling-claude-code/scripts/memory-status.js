#!/usr/bin/env node
const { init, cleanup } = require('../hooks/lib/init.js');
const cwd = process.cwd();
const { db, dbPath } = init(cwd);

try {
  const stats = db.prepare('SELECT COUNT(*) as count FROM observations').get();
  const capsuleStats = db
    .prepare(
      "SELECT COUNT(*) as total, SUM(CASE WHEN status = 'open' THEN 1 ELSE 0 END) as open_count FROM capsules",
    )
    .get();
  const pinStats = db.prepare('SELECT COUNT(*) as count FROM pins').get();
  const recentCapsules = db
    .prepare(
      'SELECT id, intent, status, opened_at, closed_at FROM capsules ORDER BY opened_at DESC LIMIT 5',
    )
    .all();

  console.log('=== Kindling Memory Status ===');
  console.log('');
  console.log('Observations: ' + stats.count);
  console.log(
    'Sessions:     ' + capsuleStats.total + ' (' + (capsuleStats.open_count || 0) + ' open)',
  );
  console.log('Pins:         ' + pinStats.count);
  console.log('Database:     ' + dbPath);
  console.log('Project:      ' + cwd);
  console.log('');

  if (recentCapsules.length > 0) {
    console.log('Recent Sessions:');
    recentCapsules.forEach((c, i) => {
      const date = new Date(c.opened_at).toLocaleDateString();
      const status = c.status === 'open' ? '(active)' : '';
      console.log('  ' + (i + 1) + '. ' + date + ' ' + status);
    });
  }
} finally {
  cleanup(db);
}
