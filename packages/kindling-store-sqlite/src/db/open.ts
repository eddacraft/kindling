/**
 * Database connection and initialization
 */

import Database from 'better-sqlite3';
import { runMigrations } from './migrate.js';
import { homedir } from 'os';
import { join } from 'path';
import { mkdirSync } from 'fs';

/**
 * Database configuration options
 */
export interface DatabaseOptions {
  /** Path to database file (defaults to ~/.kindling/kindling.db) */
  path?: string;

  /** Enable verbose logging (defaults to false) */
  verbose?: boolean;

  /** Read-only mode (defaults to false) */
  readonly?: boolean;
}

/**
 * Get the default database path
 */
function getDefaultDbPath(): string {
  const kindlingDir = join(homedir(), '.kindling');

  // Ensure directory exists
  try {
    mkdirSync(kindlingDir, { recursive: true });
  } catch (e: unknown) {
    const err = e as { code?: string };
    // Ignore if directory already exists (EEXIST error)
    if (err.code !== 'EEXIST') {
      throw err;
    }
  }

  return join(kindlingDir, 'kindling.db');
}

/**
 * Open and initialize a kindling database
 *
 * Opens database with:
 * - WAL mode (write-ahead logging) for better concurrency
 * - Foreign key enforcement
 * - Busy timeout (5 seconds) for handling concurrent writes
 * - Runs pending migrations
 *
 * @param options - Database configuration options
 * @returns Database instance
 */
export function openDatabase(options: DatabaseOptions = {}): Database.Database {
  const dbPath = options.path ?? getDefaultDbPath();

  // Open database
  const db = new Database(dbPath, {
    verbose: options.verbose ? console.log : undefined,
    readonly: options.readonly ?? false,
  });

  // Enable WAL mode for better concurrency
  // WAL allows readers to not block writers and vice versa
  db.pragma('journal_mode = WAL');

  // Enable foreign key enforcement
  db.pragma('foreign_keys = ON');

  // Set busy timeout (5 seconds)
  // If database is locked, wait up to 5 seconds before failing
  db.pragma('busy_timeout = 5000');

  // Optimize for performance
  db.pragma('synchronous = NORMAL'); // Safe with WAL mode
  db.pragma('cache_size = -64000'); // 64MB cache

  // Run migrations (unless in readonly mode)
  if (!options.readonly) {
    runMigrations(db);
  }

  return db;
}

/**
 * Close a database connection
 */
export function closeDatabase(db: Database.Database): void {
  db.close();
}
