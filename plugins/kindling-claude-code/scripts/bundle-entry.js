/**
 * Entry point for the Kindling plugin bundle.
 *
 * This file re-exports everything the plugin hooks and commands need
 * from the monorepo packages. esbuild bundles this into a single CJS file.
 */

// Store
export {
  openDatabase,
  closeDatabase,
  SqliteKindlingStore,
  runMigrations,
} from '@eddacraft/kindling-store-sqlite';

// Provider
export { LocalFtsProvider } from '@eddacraft/kindling-provider-local';

// Core
export {
  KindlingService,
  validateObservation,
  validateCapsule,
  validateSummary,
} from '@eddacraft/kindling-core';

// Adapter
export {
  createHookHandlers,
  mapEvent,
  filterContent,
  filterToolResult,
  maskSecrets,
  truncateContent,
  extractProvenance,
} from '@eddacraft/kindling-adapter-claude-code';
