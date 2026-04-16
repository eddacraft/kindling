/**
 * Provenance extraction for OpenCode events
 *
 * Extracts structured metadata from events for queryability and explainability.
 */

import type {
  ToolCallEvent,
  CommandEvent,
  FileChangeEvent,
  ErrorEvent,
  MessageEvent,
} from './events.js';

/**
 * Extract provenance from tool call event
 */
export function extractToolCallProvenance(event: ToolCallEvent): Record<string, unknown> {
  return {
    toolName: event.toolName,
    // Sanitize args to remove sensitive data
    args: sanitizeArgs(event.args),
    duration_ms: event.duration_ms,
    hasError: !!event.error,
  };
}

/**
 * Extract provenance from command event
 */
export function extractCommandProvenance(event: CommandEvent): Record<string, unknown> {
  return {
    // Only store the command name, not full args (may contain secrets)
    cmd: extractCommandName(event.command),
    exitCode: event.exitCode,
    cwd: event.cwd,
    hasStderr: !!event.stderr,
  };
}

/**
 * Extract provenance from file change event
 */
export function extractFileDiffProvenance(event: FileChangeEvent): Record<string, unknown> {
  return {
    paths: event.paths,
    additions: event.additions,
    deletions: event.deletions,
    fileCount: event.paths.length,
  };
}

/**
 * Extract provenance from error event
 */
export function extractErrorProvenance(event: ErrorEvent): Record<string, unknown> {
  return {
    source: event.source,
    // Truncate stack trace to avoid excessive storage
    stackPreview: event.stack?.substring(0, 200),
  };
}

/**
 * Extract provenance from message event
 */
export function extractMessageProvenance(event: MessageEvent): Record<string, unknown> {
  return {
    role: event.role,
    model: event.model,
    length: event.content.length,
  };
}

/**
 * Sanitize tool arguments to remove sensitive data
 */
function sanitizeArgs(args: Record<string, unknown>): Record<string, unknown> {
  const sanitized: Record<string, unknown> = {};

  for (const [key, value] of Object.entries(args)) {
    // Skip known sensitive fields
    if (isSensitiveField(key)) {
      sanitized[key] = '[REDACTED]';
      continue;
    }

    // Truncate long string values
    if (typeof value === 'string' && value.length > 100) {
      sanitized[key] = value.substring(0, 100) + '...';
    } else {
      sanitized[key] = value;
    }
  }

  return sanitized;
}

/**
 * Check if a field name suggests sensitive data
 */
function isSensitiveField(fieldName: string): boolean {
  const lowerName = fieldName.toLowerCase();
  const sensitivePatterns = ['password', 'token', 'secret', 'key', 'auth', 'credential'];

  return sensitivePatterns.some((pattern) => lowerName.includes(pattern));
}

/**
 * Extract just the command name from a full command string
 */
function extractCommandName(command: string): string {
  // Get first word (command name) without args
  const parts = command.trim().split(/\s+/);
  return parts[0] || command;
}
