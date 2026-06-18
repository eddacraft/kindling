/**
 * CLI utility functions
 */

import { homedir } from 'os';
import { join } from 'path';
import { openDatabase } from '@eddacraft/kindling-store-sqlite';
import { SqliteKindlingStore } from '@eddacraft/kindling-store-sqlite';
import { LocalFtsProvider } from '@eddacraft/kindling-provider-local';
import { KindlingService } from '@eddacraft/kindling-core';
import type Database from 'better-sqlite3';

/**
 * Get default database path
 */
export function getDefaultDbPath(): string {
  return join(homedir(), '.kindling', 'kindling.db');
}

/**
 * Initialize kindling service with database
 *
 * @param dbPath - Optional database path (defaults to ~/.kindling/kindling.db)
 * @returns Service instance and database instance
 */
export function initializeService(dbPath?: string): {
  service: KindlingService;
  db: Database.Database;
} {
  const path = dbPath || getDefaultDbPath();
  const db = openDatabase({ path });
  const store = new SqliteKindlingStore(db);
  const provider = new LocalFtsProvider(db);
  const service = new KindlingService({ store, provider });

  return { service, db };
}

/**
 * Format a timestamp for display
 */
export function formatTimestamp(ts: number): string {
  const date = new Date(ts);
  return date
    .toISOString()
    .replace('T', ' ')
    .replace(/\.\d{3}Z$/, '');
}

/**
 * Truncate text to a maximum length
 */
export function truncate(text: string, maxLength: number): string {
  if (text.length <= maxLength) return text;
  return text.slice(0, maxLength - 3) + '...';
}

/**
 * Format JSON output
 */
export function formatJson(data: unknown, pretty = false): string {
  return JSON.stringify(data, null, pretty ? 2 : 0);
}

/**
 * Format an error for display
 */
export function formatError(error: unknown, asJson = false): string {
  const message = error instanceof Error ? error.message : String(error);
  return asJson ? formatJson({ error: message }) : `Error: ${message}`;
}

/**
 * Handle command errors
 *
 * @param error - Error to handle
 * @param asJson - Format as JSON
 * @param exit - Whether to call process.exit (default: true)
 */
export function handleError(error: unknown, asJson = false, exit = true): void {
  console.error(formatError(error, asJson));

  if (exit) {
    process.exit(1);
  }
}
