/**
 * OpenCode event type definitions
 *
 * These represent events emitted by OpenCode during development sessions.
 * Note: These are example types - adapt to actual OpenCode event schema.
 */

/**
 * Base event structure
 */
export interface BaseEvent {
  type: string;
  timestamp: number;
  sessionId: string;
  repoId?: string;
}

/**
 * Tool call event
 *
 * Fired when a tool is invoked (e.g., file read, command execution)
 */
export interface ToolCallEvent extends BaseEvent {
  type: 'tool_call';
  toolName: string;
  args: Record<string, unknown>;
  result?: unknown;
  duration_ms?: number;
  error?: string;
}

/**
 * Command execution event
 *
 * Fired when a shell command is executed
 */
export interface CommandEvent extends BaseEvent {
  type: 'command';
  command: string;
  exitCode: number;
  stdout?: string;
  stderr?: string;
  cwd?: string;
}

/**
 * File change event
 *
 * Fired when files are modified
 */
export interface FileChangeEvent extends BaseEvent {
  type: 'file_change';
  paths: string[];
  diff?: string;
  additions?: number;
  deletions?: number;
}

/**
 * Error event
 *
 * Fired when an error occurs
 */
export interface ErrorEvent extends BaseEvent {
  type: 'error';
  message: string;
  stack?: string;
  source?: string;
}

/**
 * Message event
 *
 * Fired for user/assistant messages
 */
export interface MessageEvent extends BaseEvent {
  type: 'message';
  role: 'user' | 'assistant';
  content: string;
  model?: string;
}

/**
 * Session lifecycle events
 */
export interface SessionStartEvent extends BaseEvent {
  type: 'session_start';
  intent?: string;
}

export interface SessionEndEvent extends BaseEvent {
  type: 'session_end';
  reason?: string;
}

/**
 * Union of all event types
 */
export type OpenCodeEvent =
  | ToolCallEvent
  | CommandEvent
  | FileChangeEvent
  | ErrorEvent
  | MessageEvent
  | SessionStartEvent
  | SessionEndEvent;

/**
 * Type guard for OpenCode events
 */
export function isOpenCodeEvent(event: unknown): event is OpenCodeEvent {
  if (typeof event !== 'object' || event === null) {
    return false;
  }

  const e = event as Record<string, unknown>;
  return (
    typeof e.type === 'string' && typeof e.timestamp === 'number' && typeof e.sessionId === 'string'
  );
}
