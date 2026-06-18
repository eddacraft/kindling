/**
 * Shared helpers for the Kindling `/memory` slash-command scripts.
 *
 * These commands shell out to the `kindling` binary (the plugin's documented
 * prerequisite) and operate the per-project SQLite database in-process via the
 * binary's `--db <path>` flag — the same database the capture/injection hooks
 * (`kindling hook <type>`) write to. Using `--db` keeps `/memory` working
 * whether or not the `kindling serve` daemon is running, matching the old
 * in-process model.
 *
 * The project-root and database-path resolution below MUST stay byte-for-byte
 * identical to the hooks' path logic (formerly `hooks/lib/init.js`), so that
 * `/memory` targets exactly the DB the hooks populated:
 *   - projectRoot: KINDLING_REPO_ROOT (only if cwd is under it) → git toplevel → resolved cwd
 *   - dbPath:      KINDLING_DB_PATH override → ~/.kindling/projects/<sha256(root)[:12]>/kindling.db
 */

const { createHash } = require('crypto');
const { execFileSync } = require('child_process');
const { existsSync, mkdirSync } = require('fs');
const { homedir } = require('os');
const { join, resolve, dirname } = require('path');

/**
 * Resolve the project root directory.
 *
 * Checks KINDLING_REPO_ROOT first (and only honours it when the cwd is under
 * that root, to prevent cross-project contamination), then falls back to the
 * git toplevel (so the same hash is used regardless of which subdirectory
 * Claude Code launched from), then the resolved cwd.
 *
 * Mirrors `getProjectRoot` in the former `hooks/lib/init.js`.
 */
function projectRoot(cwd) {
  const cached = process.env.KINDLING_REPO_ROOT;
  if (cached && resolve(cwd).startsWith(cached)) {
    return cached;
  }
  try {
    return execFileSync('git', ['rev-parse', '--show-toplevel'], {
      cwd,
      encoding: 'utf-8',
      stdio: ['pipe', 'pipe', 'pipe'],
    }).trim();
  } catch {
    return resolve(cwd);
  }
}

/**
 * Derive the project-scoped database path from the working directory.
 *
 * KINDLING_DB_PATH overrides everything; otherwise the path is
 * ~/.kindling/projects/<sha256(projectRoot).hex[:12]>/kindling.db.
 * The containing directory is created if missing.
 *
 * Mirrors `getDbPath` in the former `hooks/lib/init.js`.
 */
function dbPath(cwd) {
  if (process.env.KINDLING_DB_PATH) {
    const dir = dirname(process.env.KINDLING_DB_PATH);
    if (!existsSync(dir)) {
      mkdirSync(dir, { recursive: true });
    }
    return process.env.KINDLING_DB_PATH;
  }

  const root = projectRoot(cwd);
  const projectId = createHash('sha256').update(root).digest('hex').slice(0, 12);
  const dir = join(homedir(), '.kindling', 'projects', projectId);
  if (!existsSync(dir)) {
    mkdirSync(dir, { recursive: true });
  }
  return join(dir, 'kindling.db');
}

/**
 * Resolve the `kindling` binary: KINDLING_BIN env override, else `kindling`
 * on PATH.
 */
function kindlingBin() {
  return process.env.KINDLING_BIN || 'kindling';
}

/**
 * Print a friendly "binary missing/failed" message and exit 0.
 *
 * The `kindling` binary is the plugin's documented prerequisite (see README).
 * Rather than throwing a stack trace at the user, the `/memory` commands fail
 * soft with guidance.
 */
function binaryUnavailable(detail) {
  console.log('Kindling could not run the `kindling` binary.');
  console.log('');
  console.log('The `kindling` binary is required for /memory commands.');
  console.log('Install it (see the plugin README) and ensure it is on your PATH,');
  console.log('or set KINDLING_BIN to its absolute path.');
  if (detail) {
    console.log('');
    console.log('Details: ' + detail);
  }
  process.exit(0);
}

/**
 * Run `kindling <args...>` and return parsed JSON from stdout.
 *
 * On a missing binary or non-zero exit, prints a friendly message and exits 0
 * (never surfaces a stack trace). The caller's `args` should include `--json`.
 */
function runJson(args) {
  let stdout;
  try {
    stdout = execFileSync(kindlingBin(), args, {
      encoding: 'utf-8',
      stdio: ['pipe', 'pipe', 'pipe'],
      maxBuffer: 32 * 1024 * 1024,
    });
  } catch (err) {
    // ENOENT → binary not found. Otherwise a non-zero exit: the CLI prints a
    // JSON `{ "error": ... }` to stdout, which we surface as guidance.
    if (err && err.code === 'ENOENT') {
      binaryUnavailable('command not found: ' + kindlingBin());
    }
    const out = (err && err.stdout) || '';
    let message = (err && err.message) || 'unknown error';
    try {
      const parsed = JSON.parse(out);
      if (parsed && parsed.error) message = parsed.error;
    } catch {
      // stdout was not JSON; fall back to the raw error message.
    }
    binaryUnavailable(message);
  }

  try {
    return JSON.parse(stdout);
  } catch (err) {
    binaryUnavailable('could not parse kindling output: ' + err.message);
  }
  return undefined;
}

module.exports = { projectRoot, dbPath, kindlingBin, runJson, binaryUnavailable };
