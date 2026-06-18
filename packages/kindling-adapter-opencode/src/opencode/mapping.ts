/**
 * Event to observation mapping
 *
 * Maps OpenCode events to Kindling observations with provenance.
 */

import type { ObservationKind, ObservationInput, ScopeIds } from '@eddacraft/kindling';
import type { OpenCodeEvent } from './events.js';
import {
  extractToolCallProvenance,
  extractCommandProvenance,
  extractFileDiffProvenance,
  extractErrorProvenance,
  extractMessageProvenance,
} from './provenance.js';

/**
 * Mapping from OpenCode event types to Kindling observation kinds
 */
export const EVENT_TO_KIND_MAP: Record<string, ObservationKind> = {
  tool_call: 'tool_call',
  command: 'command',
  file_change: 'file_diff',
  error: 'error',
  message: 'message',
} as const;

/**
 * Result of event mapping
 */
export interface MapEventResult {
  /** Successfully mapped observation */
  observation?: ObservationInput;
  /** Error if mapping failed */
  error?: string;
  /** Whether this event should be skipped */
  skip?: boolean;
}

/**
 * Map an OpenCode event to a Kindling observation
 *
 * @param event - OpenCode event to map
 * @returns Mapped observation or error
 */
export function mapEvent(event: OpenCodeEvent): MapEventResult {
  // Skip session lifecycle events (handled separately)
  if (event.type === 'session_start' || event.type === 'session_end') {
    return { skip: true };
  }

  // Get observation kind
  const kind = EVENT_TO_KIND_MAP[event.type];
  if (!kind) {
    return {
      error: `Unknown event type: ${event.type}`,
    };
  }

  // Extract content
  const content = extractContent(event);
  if (!content) {
    return {
      error: `Could not extract content from event type: ${event.type}`,
    };
  }

  // Extract provenance
  const provenance = extractProvenance(event);

  // Build scope IDs
  const scopeIds: ScopeIds = {
    sessionId: event.sessionId,
  };

  if (event.repoId) {
    scopeIds.repoId = event.repoId;
  }

  // Return observation input
  return {
    observation: {
      kind,
      content,
      // Provenance values come from arbitrary event payloads; they are JSON
      // over the wire and the daemon validates. The generated `JsonValue`-keyed
      // type is narrower than this call site can statically prove.
      provenance: provenance as ObservationInput['provenance'],
      scopeIds,
    },
  };
}

/**
 * Extract content string from event
 */
function extractContent(event: OpenCodeEvent): string | null {
  switch (event.type) {
    case 'tool_call':
      return formatToolCallContent(event);

    case 'command':
      return formatCommandContent(event);

    case 'file_change':
      return formatFileChangeContent(event);

    case 'error':
      return event.message;

    case 'message':
      return event.content;

    default:
      return null;
  }
}

/**
 * Format tool call as human-readable content
 */
function formatToolCallContent(event: OpenCodeEvent & { type: 'tool_call' }): string {
  const parts = [`Tool: ${event.toolName}`];

  if (event.result) {
    const resultStr =
      typeof event.result === 'string' ? event.result : JSON.stringify(event.result, null, 2);
    parts.push(resultStr);
  }

  if (event.error) {
    parts.push(`Error: ${event.error}`);
  }

  return parts.join('\n\n');
}

/**
 * Format command as human-readable content
 */
function formatCommandContent(event: OpenCodeEvent & { type: 'command' }): string {
  const parts = [`$ ${event.command}`];

  if (event.stdout) {
    parts.push(event.stdout);
  }

  if (event.stderr) {
    parts.push(`stderr: ${event.stderr}`);
  }

  parts.push(`Exit code: ${event.exitCode}`);

  return parts.join('\n\n');
}

/**
 * Format file change as human-readable content
 */
function formatFileChangeContent(event: OpenCodeEvent & { type: 'file_change' }): string {
  const parts = [`Modified files:\n${event.paths.map((p) => `  ${p}`).join('\n')}`];

  if (event.additions !== undefined || event.deletions !== undefined) {
    parts.push(`+${event.additions || 0} -${event.deletions || 0}`);
  }

  if (event.diff) {
    parts.push(event.diff);
  }

  return parts.join('\n\n');
}

/**
 * Extract provenance from event
 */
function extractProvenance(event: OpenCodeEvent): Record<string, unknown> {
  switch (event.type) {
    case 'tool_call':
      return extractToolCallProvenance(event);

    case 'command':
      return extractCommandProvenance(event);

    case 'file_change':
      return extractFileDiffProvenance(event);

    case 'error':
      return extractErrorProvenance(event);

    case 'message':
      return extractMessageProvenance(event);

    default:
      return {};
  }
}

/**
 * Batch map multiple events
 *
 * @param events - Events to map
 * @returns Array of mapped observations (skipped events excluded)
 */
export function mapEvents(events: OpenCodeEvent[]): ObservationInput[] {
  const observations: ObservationInput[] = [];

  for (const event of events) {
    const result = mapEvent(event);

    if (result.observation) {
      observations.push(result.observation);
    } else if (result.error) {
      console.warn(`Failed to map event: ${result.error}`, event);
    }
    // Skip events are silently ignored
  }

  return observations;
}
