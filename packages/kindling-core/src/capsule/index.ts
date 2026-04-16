/**
 * Capsule lifecycle management
 *
 * Exports capsule manager, lifecycle functions, and related types.
 */

// Types
export type {
  OpenCapsuleOptions,
  CloseCapsuleSignals,
  CapsuleManager as ICapsuleManager,
} from './types.js';

export type { CapsuleStore } from './lifecycle.js';

// Lifecycle functions
export { openCapsule, closeCapsule, getCapsule, getOpenCapsule } from './lifecycle.js';

// Manager
export { CapsuleManager } from './manager.js';
