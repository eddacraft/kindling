/**
 * Client configuration, default socket/project-root resolution, and the
 * compiled schema version.
 *
 * Mirrors `kindling_client::config` (Rust): same default socket path
 * (`~/.kindling/kindling.sock`, `KINDLING_SOCK` override), the same project-root
 * resolution the Claude Code hook uses, and the same schema version sourced from
 * the canonical `schema/version.json`.
 */

import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/** Socket file name under the kindling home. */
const SOCKET_FILE = 'kindling.sock';

/**
 * The schema version this client expects the daemon to report, read from the
 * canonical `schema/version.json` — the same single source of truth the Rust
 * store/client embed, so the TS client never disagrees about the contract.
 *
 * Resolved at module load by walking up from this file to the repo/package
 * root. A vendored copy is bundled into `dist/` at build time (see
 * `scripts/sync-types.mjs` companion `copy:schema` step) so it resolves in the
 * published package too.
 */
export const EXPECTED_SCHEMA_VERSION: number = loadExpectedSchemaVersion();

function loadExpectedSchemaVersion(): number {
  const here = dirname(fileURLToPath(import.meta.url));
  // Candidate locations, in order:
  //   1. bundled next to the build output: dist/schema/version.json
  //   2. package-local during dev: packages/kindling/schema/version.json
  //   3. repo-root canonical: <repo>/schema/version.json
  const candidates = [
    join(here, 'schema', 'version.json'),
    join(here, '..', 'schema', 'version.json'),
    join(here, '..', '..', '..', 'schema', 'version.json'),
  ];
  for (const candidate of candidates) {
    try {
      const raw = readFileSync(candidate, 'utf8');
      const parsed = JSON.parse(raw) as { version?: unknown };
      if (typeof parsed.version === 'number') {
        return parsed.version;
      }
    } catch {
      // try next candidate
    }
  }
  throw new Error(
    'could not locate schema/version.json to determine the expected schema version',
  );
}

/**
 * Default daemon socket path: `$KINDLING_SOCK` if set, else
 * `~/.kindling/kindling.sock`. Replicates the Rust client / hook resolution.
 */
export function defaultSocketPath(): string {
  const override = process.env.KINDLING_SOCK;
  if (override && override.length > 0) {
    return override;
  }
  const home =
    (process.env.HOME && process.env.HOME.length > 0 && process.env.HOME) ||
    (process.env.USERPROFILE &&
      process.env.USERPROFILE.length > 0 &&
      process.env.USERPROFILE) ||
    homedir();
  return join(home, '.kindling', SOCKET_FILE);
}

/**
 * Resolve the project root used for the `X-Kindling-Project` header, matching
 * the Claude Code hook's `getProjectRoot(cwd)`:
 *
 *   1. `KINDLING_REPO_ROOT` if set AND `resolve(cwd)` starts with it.
 *   2. `git rev-parse --show-toplevel` run in `cwd`, trimmed.
 *   3. The resolved `cwd`.
 */
export function resolveProjectRoot(cwd: string = process.cwd()): string {
  const resolved = resolve(cwd);

  const envRoot = process.env.KINDLING_REPO_ROOT;
  if (envRoot && envRoot.length > 0 && resolved.startsWith(envRoot)) {
    return envRoot;
  }

  try {
    const top = execFileSync('git', ['rev-parse', '--show-toplevel'], {
      cwd,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
    }).trim();
    if (top.length > 0) {
      return top;
    }
  } catch {
    // git unavailable or not a repo — fall through
  }

  return resolved;
}

/** Options for constructing a {@link Kindling} client. */
export interface KindlingOptions {
  /** Daemon socket path. Default: `$KINDLING_SOCK` or `~/.kindling/kindling.sock`. */
  socketPath?: string;
  /**
   * Project root string sent as `X-Kindling-Project` on every data endpoint.
   * Default: the hook-parity resolution of `process.cwd()`.
   */
  projectRoot?: string;
  /**
   * Path to the `kindling` binary used for auto-spawn. Default resolution:
   * `$KINDLING_BIN` → `node_modules/@eddacraft/kindling/bin/kindling` → `kindling` on PATH.
   */
  binaryPath?: string;
  /** Schema version to require from `/v1/health`. Default {@link EXPECTED_SCHEMA_VERSION}. */
  expectedSchemaVersion?: number;
  /** Total auto-spawn connect budget in ms (connect + spawn + poll). Default 1000. */
  connectTimeoutMs?: number;
  /** Interval between socket-connect attempts while polling, in ms. Default 10. */
  pollIntervalMs?: number;
  /** Whether to auto-spawn the daemon on first connect failure. Default true. */
  autoSpawn?: boolean;
}

/** Fully-resolved client configuration. */
export interface ResolvedConfig {
  socketPath: string;
  projectRoot: string;
  binaryPath: string | null;
  expectedSchemaVersion: number;
  connectTimeoutMs: number;
  pollIntervalMs: number;
  autoSpawn: boolean;
}

/** Resolve the bundled binary path under this package's `bin/` directory. */
function packagedBinaryPath(): string {
  const here = dirname(fileURLToPath(import.meta.url));
  // dist/config.js -> ../bin/kindling ; src/config.ts -> ../bin/kindling
  return join(here, '..', 'bin', 'kindling');
}

/**
 * Resolve the binary path for auto-spawn:
 *   `$KINDLING_BIN` → packaged `bin/kindling` → `kindling` on PATH.
 *
 * Returns `null` when none can be statically determined (PATH lookup is
 * deferred to spawn time, represented by the bare name `"kindling"`).
 */
function resolveBinaryPath(explicit?: string): string {
  if (explicit && explicit.length > 0) return explicit;
  const env = process.env.KINDLING_BIN;
  if (env && env.length > 0) return env;
  return packagedBinaryPath();
}

/** Apply defaults to {@link KindlingOptions}. */
export function resolveConfig(options: KindlingOptions = {}): ResolvedConfig {
  return {
    socketPath: options.socketPath ?? defaultSocketPath(),
    projectRoot: options.projectRoot ?? resolveProjectRoot(),
    binaryPath: resolveBinaryPath(options.binaryPath),
    expectedSchemaVersion:
      options.expectedSchemaVersion ?? EXPECTED_SCHEMA_VERSION,
    connectTimeoutMs: options.connectTimeoutMs ?? 1000,
    pollIntervalMs: options.pollIntervalMs ?? 10,
    autoSpawn: options.autoSpawn ?? true,
  };
}
