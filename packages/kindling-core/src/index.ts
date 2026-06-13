/**
 * Kindling Core
 *
 * Domain model, capsule lifecycle, and retrieval orchestration.
 */

// Re-export all types
export * from './types/index.js';

// Re-export all validation functions
export * from './validation/index.js';

// Re-export capsule lifecycle
export * from './capsule/index.js';

// Re-export retrieval orchestration
export * from './retrieval/index.js';

// Re-export export/import coordination
export * from './export/index.js';

// Re-export service orchestration
export * from './service/index.js';

// Re-export intent capture (append-only store + high-signal emitters)
export * from './intent/index.js';
