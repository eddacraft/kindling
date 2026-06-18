/**
 * Database connection and initialization for sql.js
 */

import initSqlJs, { type Database, type SqlJsStatic } from 'sql.js';
import { runMigrations } from './migrate.js';

/**
 * WASM file locator function
 */
export type WasmLocator = (file: string) => string;

/**
 * Database configuration options
 */
export interface DatabaseOptions {
  /**
   * Initial database data (Uint8Array from previous export)
   * If not provided, creates an empty database
   */
  data?: Uint8Array;

  /**
   * Function to locate WASM files
   * Default: uses sql.js CDN
   *
   * Examples:
   * - CDN: (file) => `https://sql.js.org/dist/${file}`
   * - Local: (file) => `/wasm/${file}`
   * - Bundled: (file) => new URL(`./sql-wasm/${file}`, import.meta.url).href
   */
  locateFile?: WasmLocator;

  /**
   * Skip running migrations (useful for readonly scenarios)
   */
  skipMigrations?: boolean;

  /**
   * Enable FTS5 migrations
   * Default: auto-detect (checks if FTS5 is available)
   *
   * Note: Standard sql.js builds don't include FTS5.
   * Set to false to skip FTS migrations, or true to require them.
   */
  enableFts?: boolean;

  /**
   * Enable verbose logging
   */
  verbose?: boolean;
}

// Cache the SQL.js instance to avoid re-loading WASM
let sqlPromise: Promise<SqlJsStatic> | null = null;

/**
 * Check if FTS5 is available in the sql.js build
 */
function hasFts5Support(db: Database): boolean {
  try {
    // Try to create a test FTS5 table
    db.run('CREATE VIRTUAL TABLE IF NOT EXISTS __fts5_test USING fts5(content)');
    db.run('DROP TABLE IF EXISTS __fts5_test');
    return true;
  } catch {
    return false;
  }
}

/**
 * Initialize SQL.js (cached)
 */
async function initSql(locateFile?: WasmLocator): Promise<SqlJsStatic> {
  if (!sqlPromise) {
    sqlPromise = initSqlJs({
      locateFile: locateFile ?? ((file: string) => `https://sql.js.org/dist/${file}`),
    });
  }
  return sqlPromise;
}

/**
 * Open and initialize a kindling database
 *
 * Opens database with:
 * - Foreign key enforcement
 * - Runs pending migrations (unless skipMigrations is true)
 *
 * Note: sql.js does not support WAL mode or pragma settings like
 * busy_timeout since it runs entirely in memory.
 *
 * @param options - Database configuration options
 * @returns Promise resolving to Database instance
 */
export async function openDatabase(options: DatabaseOptions = {}): Promise<Database> {
  const SQL = await initSql(options.locateFile);

  if (options.verbose) {
    console.log('sql.js initialized');
  }

  // Create database (from data or empty)
  const db = options.data ? new SQL.Database(options.data) : new SQL.Database();

  if (options.verbose) {
    console.log(options.data ? 'Loaded database from data' : 'Created new database');
  }

  // Enable foreign key enforcement
  db.run('PRAGMA foreign_keys = ON');

  // Detect FTS5 support if not explicitly set
  let enableFts = options.enableFts;
  if (enableFts === undefined) {
    enableFts = hasFts5Support(db);
    if (options.verbose) {
      console.log(`FTS5 support: ${enableFts ? 'available' : 'not available'}`);
    }
  }

  // Run migrations (unless skipped)
  if (!options.skipMigrations) {
    runMigrations(db, { verbose: options.verbose, enableFts });
  }

  return db;
}

/**
 * Export database to Uint8Array for persistence
 *
 * @param db - Database instance
 * @returns Uint8Array containing the entire database
 */
export function exportDatabaseToBytes(db: Database): Uint8Array {
  return db.export();
}

/**
 * Close a database connection
 *
 * @param db - Database instance
 */
export function closeDatabase(db: Database): void {
  db.close();
}

/**
 * Reset the SQL.js cache (useful for testing)
 */
export function resetSqlCache(): void {
  sqlPromise = null;
}
