#!/usr/bin/env node
const { dbPath, runJson } = require('./lib/kindling.js');

const cwd = process.cwd();
const db = dbPath(cwd);

// `kindling status --db <db> --json` →
//   { database: { path, size, sizeBytes },
//     counts:   { observations, capsules, summaries, pins, redacted, openCapsules },
//     activity: { latestTimestamp, latestDate } }
//
// The Rust status reports aggregate counts only; the old script's per-session
// "Recent Sessions" list (recent capsule rows) is not part of the CLI's status
// output, so it is omitted here. Latest activity is shown instead.
const status = runJson(['status', '--db', db, '--json']);
const counts = (status && status.counts) || {};
const database = (status && status.database) || {};
const activity = (status && status.activity) || {};

console.log('=== kindling Memory Status ===');
console.log('');
console.log('Observations: ' + (counts.observations || 0));
console.log('Sessions:     ' + (counts.capsules || 0) + ' (' + (counts.openCapsules || 0) + ' open)');
console.log('Pins:         ' + (counts.pins || 0));
console.log('Summaries:    ' + (counts.summaries || 0));
console.log('Redacted:     ' + (counts.redacted || 0));
console.log('Database:     ' + (database.path || db));
console.log('Project:      ' + cwd);

if (activity.latestDate) {
  console.log('');
  console.log('Latest activity: ' + new Date(activity.latestTimestamp).toLocaleString());
}
