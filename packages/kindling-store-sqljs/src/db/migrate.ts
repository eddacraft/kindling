/**
 * Database migration infrastructure for sql.js
 */

import type { Database } from 'sql.js';
import { getMigrations } from './migrations.js';

/**
 * Migration options
 */
export interface MigrationOptions {
  /** Enable verbose logging */
  verbose?: boolean;
  /** Enable FTS5 migrations (skip if false) */
  enableFts?: boolean;
}

/**
 * Get the current schema version
 */
function getCurrentVersion(db: Database): number {
  try {
    const result = db.exec(`
      SELECT MAX(version) as version FROM schema_migrations
    `);

    if (result.length === 0 || result[0].values.length === 0) {
      return 0;
    }

    const version = result[0].values[0][0];
    return typeof version === 'number' ? version : 0;
  } catch {
    // Table doesn't exist yet
    return 0;
  }
}

/**
 * Run all pending migrations
 *
 * Migrations are:
 * - Additive only (never destructive)
 * - Idempotent (safe to re-run)
 * - Applied in order
 *
 * @param db - Database instance
 * @param options - Migration options
 * @returns Number of migrations applied
 */
export function runMigrations(db: Database, options: MigrationOptions = {}): number {
  const { verbose = false, enableFts = true } = options;
  const currentVersion = getCurrentVersion(db);
  const migrations = getMigrations();

  let applied = 0;

  for (const migration of migrations) {
    if (migration.version > currentVersion) {
      // Skip FTS migration if FTS is not enabled
      if (migration.name === '002_fts' && !enableFts) {
        if (verbose) {
          console.log(`Skipping migration ${migration.name} (FTS5 not enabled)`);
        }
        // Record the migration as applied so we don't try again
        db.run(
          `INSERT OR IGNORE INTO schema_migrations (version, name, applied_at)
           VALUES (?, ?, ?)`,
          [migration.version, migration.name + '_skipped', Date.now()],
        );
        continue;
      }

      if (verbose) {
        console.log(`Applying migration ${migration.name}...`);
      }

      // Run migration
      // Note: sql.js doesn't have native transaction support via API,
      // but the SQL itself can use BEGIN/COMMIT
      db.run('BEGIN TRANSACTION');
      try {
        db.run(migration.sql);
        db.run('COMMIT');
        applied++;

        if (verbose) {
          console.log(`Applied migration ${migration.name}`);
        }
      } catch (err) {
        db.run('ROLLBACK');
        throw new Error(`Migration ${migration.name} failed: ${err}`);
      }
    }
  }

  if (verbose) {
    if (applied === 0) {
      console.log('Database is up to date');
    } else {
      console.log(`Applied ${applied} migration(s)`);
    }
  }

  return applied;
}

/**
 * Get migration status
 */
export function getMigrationStatus(db: Database): {
  currentVersion: number;
  latestVersion: number;
  appliedMigrations: string[];
  pendingMigrations: string[];
} {
  const currentVersion = getCurrentVersion(db);
  const migrations = getMigrations();
  const latestVersion = Math.max(...migrations.map((m) => m.version));

  const appliedMigrations: string[] = [];
  const pendingMigrations: string[] = [];

  for (const migration of migrations) {
    if (migration.version <= currentVersion) {
      appliedMigrations.push(migration.name);
    } else {
      pendingMigrations.push(migration.name);
    }
  }

  return {
    currentVersion,
    latestVersion,
    appliedMigrations,
    pendingMigrations,
  };
}
