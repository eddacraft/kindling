#!/usr/bin/env node
const { dbPath, runJson } = require('./lib/kindling.js');

const cwd = process.cwd();
const db = dbPath(cwd);
const pinId = process.argv[2] || '';

if (!pinId) {
  console.log('Usage: /memory unpin <id>');
  console.log('Get pin IDs from /memory pins');
  process.exit(0);
}

// Prefix-resolve the pin id from `kindling list pins --json`, then unpin the
// full id (the CLI's `unpin` takes an exact id).
const pins = runJson(['list', 'pins', '--db', db, '--json']);
const pin = (pins || []).find((p) => String(p.id).startsWith(pinId));

if (!pin) {
  console.log('Pin not found: ' + pinId);
  console.log('Use /memory pins to see all pin IDs.');
  process.exit(0);
}

runJson(['unpin', pin.id, '--db', db, '--json']);

console.log('Removed pin:');
console.log('  ID: ' + String(pin.id).substring(0, 8));
console.log('  Note: ' + (pin.reason || 'Pin'));
console.log('');
console.log('Remaining pins: ' + (pins.length - 1));
