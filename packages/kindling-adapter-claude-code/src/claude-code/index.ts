/**
 * Claude Code adapter module
 *
 * Exports all public APIs for the Claude Code adapter.
 */

// Event types and factories
export {
  type HookContext,
  type ToolUseContext,
  type PostToolUseContext,
  type SessionStartContext,
  type StopContext,
  type SubagentStopContext,
  type UserPromptSubmitContext,
  type PreCompactContext,
  type ClaudeCodeEvent,
  createSessionStartEvent,
  createPostToolUseEvent,
  createStopEvent,
  createSubagentStopEvent,
  createUserPromptEvent,
  createPreCompactEvent,
  isClaudeCodeEvent,
} from './events.js';

// Mapping
export { mapEvent, mapEvents, type MapEventResult } from './mapping.js';

// Provenance
export {
  extractProvenance,
  extractToolUseProvenance,
  extractUserPromptProvenance,
  extractSubagentProvenance,
  extractStopProvenance,
} from './provenance.js';

// Filtering
export {
  filterContent,
  filterToolResult,
  truncateContent,
  maskSecrets,
  containsSecrets,
  isExcludedPath,
  shouldCaptureToolResult,
  createRedactionReason,
  MAX_CONTENT_LENGTH,
  MAX_RESULT_LENGTH,
  type FilterOptions,
} from './filter.js';

// Session management
export {
  SessionManager,
  type SessionContext,
  type SessionStartOptions,
  type SessionEndSignals,
  type EventProcessingResult,
} from './session.js';

// Hook handlers
export {
  createHookHandlers,
  type HookStore,
  type HookHandlerConfig,
  type HookHandlers,
} from './hooks.js';
