/**
 * Resolve a `/memory forget` id against `kindling list observations`.
 *
 * The CLI list is newest-first and `--limit` has no offset, so the plugin used
 * to scan only the first 500 rows. Exact ids older than that window were
 * reported missing, and a unique prefix in the window could hide a second match
 * later. This helper pages by growing `--limit` so exact equality is honoured
 * wherever the row sits, and prefix uniqueness is checked across the complete
 * set.
 */

const DEFAULT_PAGE_SIZE = 500;

/**
 * Page size for listing observations while resolving a forget id.
 *
 * `KINDLING_FORGET_PAGE_SIZE` is a test hook so regressions can cover a later
 * page without seeding 500+ rows. Invalid or missing values fall back to 500,
 * matching the historical list window.
 */
function forgetListPageSize(env) {
  const source = env || process.env;
  const raw = source.KINDLING_FORGET_PAGE_SIZE;
  const n = raw === undefined || raw === '' ? DEFAULT_PAGE_SIZE : Number(raw);
  if (!Number.isFinite(n) || n < 1) return DEFAULT_PAGE_SIZE;
  return Math.floor(n);
}

/**
 * Resolve `obsId` against newest-first pages from `listWithLimit(limit)`.
 *
 * `listWithLimit` must return the newest `limit` observations (the CLI list
 * shape). An exact id match wins as soon as it is seen — `kindling forget`
 * takes an exact id, so we never treat a complete id as an ambiguous prefix.
 * Otherwise prefix matches are collected across every page.
 *
 * @returns {{ kind: 'exact', observation: object }
 *   | { kind: 'prefix', observation: object }
 *   | { kind: 'ambiguous', matches: object[] }
 *   | { kind: 'missing' }}
 */
function resolveObservationIdFromPages(listWithLimit, obsId, pageSize) {
  const id = String(obsId);
  const size = pageSize >= 1 ? Math.floor(pageSize) : DEFAULT_PAGE_SIZE;
  const prefixMatches = [];
  let seen = 0;

  while (true) {
    const listed = listWithLimit(seen + size);
    const rows = Array.isArray(listed) ? listed : [];
    const page = rows.slice(seen);
    if (page.length === 0) break;

    for (const observation of page) {
      const observationId = String(observation.id);
      if (observationId === id) {
        return { kind: 'exact', observation };
      }
      if (observationId.startsWith(id)) {
        prefixMatches.push(observation);
      }
    }

    seen += page.length;
    if (page.length < size) break;
  }

  if (prefixMatches.length === 0) return { kind: 'missing' };
  if (prefixMatches.length > 1) return { kind: 'ambiguous', matches: prefixMatches };
  return { kind: 'prefix', observation: prefixMatches[0] };
}

module.exports = {
  DEFAULT_PAGE_SIZE,
  forgetListPageSize,
  resolveObservationIdFromPages,
};
