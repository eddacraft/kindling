/**
 * Unit tests for `/memory forget` id resolution.
 *
 * These cover the page-window bugs without a `kindling` binary: exact ids past
 * the first page, and prefix matches that only become ambiguous once a later
 * page is included.
 */

const test = require('node:test');
const assert = require('node:assert');
const {
  DEFAULT_PAGE_SIZE,
  forgetListPageSize,
  resolveObservationIdFromPages,
} = require('../scripts/lib/resolve-observation-id.js');

function newestFirst(rows) {
  return (limit) => rows.slice(0, limit);
}

test('forgetListPageSize defaults to 500', () => {
  assert.equal(forgetListPageSize({}), DEFAULT_PAGE_SIZE);
  assert.equal(forgetListPageSize({ KINDLING_FORGET_PAGE_SIZE: '' }), DEFAULT_PAGE_SIZE);
  assert.equal(forgetListPageSize({ KINDLING_FORGET_PAGE_SIZE: 'nope' }), DEFAULT_PAGE_SIZE);
  assert.equal(forgetListPageSize({ KINDLING_FORGET_PAGE_SIZE: '0' }), DEFAULT_PAGE_SIZE);
  assert.equal(forgetListPageSize({ KINDLING_FORGET_PAGE_SIZE: '2' }), 2);
});

test('resolves an exact id on a later list page', () => {
  const rows = [{ id: 'n1' }, { id: 'n2' }, { id: 'old-exact' }];
  const result = resolveObservationIdFromPages(newestFirst(rows), 'old-exact', 2);
  assert.equal(result.kind, 'exact');
  assert.equal(result.observation.id, 'old-exact');
});

test('resolves a unique prefix on a later list page', () => {
  const rows = [{ id: 'aaaa' }, { id: 'bbbb' }, { id: 'zzzz-old' }];
  const result = resolveObservationIdFromPages(newestFirst(rows), 'zz', 2);
  assert.equal(result.kind, 'prefix');
  assert.equal(result.observation.id, 'zzzz-old');
});

test('treats a prefix as ambiguous when matches span list pages', () => {
  // First page (newest two) has one `ab*` match; the older row on page 2 is
  // another. Scanning only page 1 would wrongly redact `ab-new`.
  const rows = [{ id: 'ab-new' }, { id: 'cd-other' }, { id: 'ab-old' }];
  const result = resolveObservationIdFromPages(newestFirst(rows), 'ab', 2);
  assert.equal(result.kind, 'ambiguous');
  assert.deepEqual(
    result.matches.map((o) => o.id),
    ['ab-new', 'ab-old'],
  );
});

test('an exact id wins even when another row shares it as a prefix', () => {
  const rows = [{ id: 'aa-extra' }, { id: 'other' }, { id: 'aa' }];
  const result = resolveObservationIdFromPages(newestFirst(rows), 'aa', 2);
  assert.equal(result.kind, 'exact');
  assert.equal(result.observation.id, 'aa');
});

test('reports missing when no page contains the id or prefix', () => {
  const rows = [{ id: 'aaaa' }, { id: 'bbbb' }, { id: 'cccc' }];
  const result = resolveObservationIdFromPages(newestFirst(rows), 'nope', 2);
  assert.equal(result.kind, 'missing');
});

test('a non-array list result is treated as empty', () => {
  const result = resolveObservationIdFromPages(() => ({ error: 'nope' }), 'aa', 2);
  assert.equal(result.kind, 'missing');
});
