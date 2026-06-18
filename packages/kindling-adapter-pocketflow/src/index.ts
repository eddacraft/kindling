/**
 * Kindling PocketFlow Adapter
 *
 * PocketFlow workflow integration for node-level capsules.
 * Captures node execution lifecycle as observations with intent
 * inference and confidence tracking.
 */

// Core lifecycle classes
export {
  KindlingNode,
  KindlingFlow,
  type KindlingNodeContext,
  type NodeMetadata,
} from './pocketflow/lifecycle.js';

// Intent inference
export { inferIntent, type IntentMapping, DEFAULT_INTENT_PATTERNS } from './pocketflow/intent.js';

// Confidence tracking
export {
  ConfidenceTracker,
  calculateConfidence,
  type ConfidenceConfig,
  type ConfidenceState,
  type ExecutionResult,
} from './pocketflow/confidence.js';

// Base PocketFlow classes (re-exported for convenience)
export { BaseNode, Node, Flow } from './pocketflow/lifecycle.js';
