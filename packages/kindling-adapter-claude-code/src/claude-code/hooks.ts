/**
 * Claude Code hook handlers
 *
 * Provides hook handler implementations for Claude Code integration.
 * These handlers can be registered with Claude Code's hook system.
 */

import type { CapsuleStore, Observation, ID } from '@eddacraft/kindling-core';
import { SessionManager } from './session.js';
import {
  createPostToolUseEvent,
  createStopEvent,
  createSubagentStopEvent,
  createUserPromptEvent,
  type SessionStartContext,
  type PostToolUseContext,
  type StopContext,
  type SubagentStopContext,
  type UserPromptSubmitContext,
} from './events.js';

/**
 * Store interface required by hooks
 */
export type HookStore = CapsuleStore & {
  insertObservation(observation: Observation): void;
  attachObservationToCapsule(capsuleId: ID, observationId: ID): void;
};

/**
 * Hook handler configuration
 */
export interface HookHandlerConfig {
  /** Whether to capture tool results */
  captureResults?: boolean;
  /** Whether to capture user messages */
  captureUserMessages?: boolean;
  /** Whether to capture subagent outputs */
  captureSubagents?: boolean;
  /** Custom intent for sessions */
  defaultIntent?: string;
}

/**
 * Creates Claude Code hook handlers connected to a kindling store
 *
 * Usage:
 * ```typescript
 * const store = new SqliteKindlingStore(db);
 * const handlers = createHookHandlers(store);
 *
 * // Register with Claude Code hooks
 * // SessionStart hook: handlers.onSessionStart
 * // PostToolUse hook: handlers.onPostToolUse
 * // Stop hook: handlers.onStop
 * ```
 */
export function createHookHandlers(store: HookStore, config: HookHandlerConfig = {}) {
  const {
    captureResults = true,
    captureUserMessages = true,
    captureSubagents = true,
    defaultIntent = 'Claude Code session',
  } = config;

  const sessionManager = new SessionManager(store);

  return {
    /**
     * SessionStart hook handler
     *
     * Opens a new capsule for the session.
     */
    onSessionStart: (ctx: SessionStartContext) => {
      sessionManager.onSessionStart({
        sessionId: ctx.sessionId,
        cwd: ctx.cwd,
        intent: defaultIntent,
      });

      // Return continue signal (don't block)
      return { continue: true };
    },

    /**
     * PostToolUse hook handler
     *
     * Captures tool calls as observations.
     */
    onPostToolUse: (ctx: PostToolUseContext) => {
      if (!captureResults) {
        return { continue: true };
      }

      const event = createPostToolUseEvent(ctx);
      sessionManager.onEvent(event);

      return { continue: true };
    },

    /**
     * Stop hook handler
     *
     * Closes the session capsule.
     */
    onStop: (ctx: StopContext) => {
      const event = createStopEvent(ctx);

      try {
        sessionManager.onStop(event.sessionId, {
          reason: ctx.reason,
          summaryContent: ctx.summary,
        });
      } catch {
        // Session may not exist if started before adapter was installed
        console.warn(`Could not close session ${event.sessionId}: session not found`);
      }

      return { continue: true };
    },

    /**
     * SubagentStop hook handler
     *
     * Captures subagent completions as observations.
     */
    onSubagentStop: (ctx: SubagentStopContext) => {
      if (!captureSubagents) {
        return { continue: true };
      }

      const event = createSubagentStopEvent(ctx);
      sessionManager.onEvent(event);

      return { continue: true };
    },

    /**
     * UserPromptSubmit hook handler
     *
     * Captures user messages as observations.
     */
    onUserPromptSubmit: (ctx: UserPromptSubmitContext) => {
      if (!captureUserMessages) {
        return { continue: true };
      }

      const event = createUserPromptEvent(ctx);
      sessionManager.onEvent(event);

      return { continue: true };
    },

    /**
     * Get the session manager for advanced usage
     */
    getSessionManager: () => sessionManager,

    /**
     * Check if a session is active
     */
    isSessionActive: (sessionId: string) => sessionManager.isSessionActive(sessionId),

    /**
     * Get session statistics
     */
    getSessionStats: (sessionId: string) => sessionManager.getSessionStats(sessionId),
  };
}

/**
 * Hook handler type for TypeScript consumers
 */
export type HookHandlers = ReturnType<typeof createHookHandlers>;
