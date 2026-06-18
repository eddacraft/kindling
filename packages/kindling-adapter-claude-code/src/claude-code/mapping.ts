/**
 * Event to observation mapping for Claude Code
 *
 * Maps Claude Code hook events to kindling observations with provenance.
 */

import type { ObservationKind, ObservationInput } from '@eddacraft/kindling-core';
import type { ClaudeCodeEvent } from './events.js';
import { extractProvenance } from './provenance.js';
import { filterContent, filterToolResult } from './filter.js';

/**
 * Mapping from tool names to observation kinds
 */
const TOOL_TO_KIND_MAP: Record<string, ObservationKind> = {
  // File operations -> file_diff
  Write: 'file_diff',
  Edit: 'file_diff',

  // Shell commands -> command
  Bash: 'command',

  // Everything else -> tool_call
  Read: 'tool_call',
  Glob: 'tool_call',
  Grep: 'tool_call',
  Task: 'tool_call',
  WebFetch: 'tool_call',
  WebSearch: 'tool_call',
  AskUserQuestion: 'tool_call',
  Skill: 'tool_call',
};

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
 * Map a Claude Code event to a kindling observation
 */
export function mapEvent(event: ClaudeCodeEvent): MapEventResult {
  switch (event.type) {
    case 'session_start':
    case 'pre_compact':
      // Session lifecycle handled separately
      return { skip: true };

    case 'post_tool_use':
      return mapToolUseEvent(event);

    case 'user_prompt':
      return mapUserPromptEvent(event);

    case 'subagent_stop':
      return mapSubagentStopEvent(event);

    case 'stop':
      // Stop event triggers capsule close, handled separately
      return { skip: true };

    default:
      return { error: `Unknown event type: ${event.type}` };
  }
}

/**
 * Map a tool use event to an observation
 */
function mapToolUseEvent(event: ClaudeCodeEvent): MapEventResult {
  if (!event.toolName) {
    return { error: 'Tool use event missing toolName' };
  }

  // Determine observation kind
  const kind = TOOL_TO_KIND_MAP[event.toolName] ?? 'tool_call';

  // Format content based on tool type
  const content = formatToolContent(event);

  // Extract provenance
  const provenance = extractProvenance(event);

  // Build scope IDs
  const scopeIds: Record<string, string> = {
    sessionId: event.sessionId,
    repoId: event.cwd,
  };

  return {
    observation: {
      kind,
      content,
      provenance,
      scopeIds,
    },
  };
}

/**
 * Map a user prompt event to an observation
 */
function mapUserPromptEvent(event: ClaudeCodeEvent): MapEventResult {
  if (!event.userContent) {
    return { error: 'User prompt event missing content' };
  }

  const content = filterContent(event.userContent, { maxLength: 10000 });
  const provenance = extractProvenance(event);

  const scopeIds: Record<string, string> = {
    sessionId: event.sessionId,
    repoId: event.cwd,
  };

  return {
    observation: {
      kind: 'message',
      content,
      provenance,
      scopeIds,
    },
  };
}

/**
 * Map a subagent stop event to an observation
 */
function mapSubagentStopEvent(event: ClaudeCodeEvent): MapEventResult {
  const content = formatSubagentContent(event);
  const provenance = extractProvenance(event);

  const scopeIds: Record<string, string> = {
    sessionId: event.sessionId,
    repoId: event.cwd,
  };

  return {
    observation: {
      kind: 'node_end',
      content,
      provenance,
      scopeIds,
    },
  };
}

/**
 * Format tool content for human readability
 */
function formatToolContent(event: ClaudeCodeEvent): string {
  const toolName = event.toolName ?? 'unknown';
  const parts: string[] = [`Tool: ${toolName}`];

  // Add tool-specific content
  switch (toolName) {
    case 'Read': {
      const filePath = event.toolInput?.file_path;
      if (filePath) parts.push(`File: ${filePath}`);
      break;
    }

    case 'Write': {
      const filePath = event.toolInput?.file_path;
      if (filePath) parts.push(`File: ${filePath}`);
      parts.push('Action: Created/overwrote file');
      break;
    }

    case 'Edit': {
      const filePath = event.toolInput?.file_path;
      if (filePath) parts.push(`File: ${filePath}`);
      parts.push('Action: Edited file');
      break;
    }

    case 'Bash': {
      const command = event.toolInput?.command;
      if (command) parts.push(`$ ${command}`);
      const resultStr = filterToolResult(toolName, event.toolResult);
      if (resultStr) parts.push(resultStr);
      break;
    }

    case 'Glob': {
      const pattern = event.toolInput?.pattern;
      const path = event.toolInput?.path;
      if (pattern) parts.push(`Pattern: ${pattern}`);
      if (path) parts.push(`Path: ${path}`);
      break;
    }

    case 'Grep': {
      const pattern = event.toolInput?.pattern;
      const path = event.toolInput?.path;
      if (pattern) parts.push(`Pattern: ${pattern}`);
      if (path) parts.push(`Path: ${path}`);
      break;
    }

    case 'Task': {
      const agentType = event.toolInput?.subagent_type;
      const description = event.toolInput?.description;
      if (agentType) parts.push(`Agent: ${agentType}`);
      if (description) parts.push(`Task: ${description}`);
      break;
    }

    case 'WebFetch': {
      const url = event.toolInput?.url;
      if (url) parts.push(`URL: ${url}`);
      break;
    }

    case 'WebSearch': {
      const query = event.toolInput?.query;
      if (query) parts.push(`Query: ${query}`);
      break;
    }

    default: {
      // For unknown tools, show input keys
      if (event.toolInput) {
        const keys = Object.keys(event.toolInput).join(', ');
        parts.push(`Input keys: ${keys}`);
      }
    }
  }

  // Add error if present
  if (event.toolError) {
    parts.push(`Error: ${event.toolError}`);
  }

  return parts.join('\n\n');
}

/**
 * Format subagent content
 */
function formatSubagentContent(event: ClaudeCodeEvent): string {
  const parts: string[] = [`Subagent: ${event.agentType ?? 'unknown'}`];

  if (event.agentOutput) {
    const output = filterContent(event.agentOutput, { maxLength: 5000 });
    parts.push(`Output:\n${output}`);
  }

  return parts.join('\n\n');
}

/**
 * Batch map multiple events
 */
export function mapEvents(events: ClaudeCodeEvent[]): ObservationInput[] {
  const observations: ObservationInput[] = [];

  for (const event of events) {
    const result = mapEvent(event);

    if (result.observation) {
      observations.push(result.observation);
    } else if (result.error) {
      console.warn(`Failed to map event: ${result.error}`, event);
    }
  }

  return observations;
}
