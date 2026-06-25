/**
 * Tests for OpenCode event mapping
 */

import { describe, it, expect } from 'vitest';
import { mapEvent, mapEvents, EVENT_TO_KIND_MAP } from '../src/opencode/mapping.js';
import type {
  ToolCallEvent,
  CommandEvent,
  FileChangeEvent,
  ErrorEvent,
  MessageEvent,
  SessionStartEvent,
} from '../src/opencode/events.js';

describe('Event Mapping', () => {
  describe('EVENT_TO_KIND_MAP', () => {
    it('should map tool_call to tool_call kind', () => {
      expect(EVENT_TO_KIND_MAP['tool_call']).toBe('tool_call');
    });

    it('should map command to command kind', () => {
      expect(EVENT_TO_KIND_MAP['command']).toBe('command');
    });

    it('should map file_change to file_diff kind', () => {
      expect(EVENT_TO_KIND_MAP['file_change']).toBe('file_diff');
    });

    it('should map error to error kind', () => {
      expect(EVENT_TO_KIND_MAP['error']).toBe('error');
    });

    it('should map message to message kind', () => {
      expect(EVENT_TO_KIND_MAP['message']).toBe('message');
    });
  });

  describe('mapEvent', () => {
    describe('tool_call events', () => {
      it('should map tool call with result', () => {
        const event: ToolCallEvent = {
          type: 'tool_call',
          timestamp: Date.now(),
          sessionId: 's1',
          repoId: '/repo',
          toolName: 'read_file',
          args: { path: 'test.ts' },
          result: 'file contents',
          duration_ms: 100,
        };

        const result = mapEvent(event);

        expect(result.observation).toBeDefined();
        expect(result.observation!.kind).toBe('tool_call');
        expect(result.observation!.content).toContain('Tool: read_file');
        expect(result.observation!.content).toContain('file contents');
        expect(result.observation!.scopeIds.sessionId).toBe('s1');
        expect(result.observation!.scopeIds.repoId).toBe('/repo');
        expect(result.observation!.provenance!.toolName).toBe('read_file');
        expect(result.observation!.provenance!.duration_ms).toBe(100);
      });

      it('should map tool call with error', () => {
        const event: ToolCallEvent = {
          type: 'tool_call',
          timestamp: Date.now(),
          sessionId: 's1',
          toolName: 'run_command',
          args: { cmd: 'test' },
          error: 'Command not found',
        };

        const result = mapEvent(event);

        expect(result.observation).toBeDefined();
        expect(result.observation!.content).toContain('Error: Command not found');
        expect(result.observation!.provenance!.hasError).toBe(true);
      });

      it('should sanitize sensitive args', () => {
        const event: ToolCallEvent = {
          type: 'tool_call',
          timestamp: Date.now(),
          sessionId: 's1',
          toolName: 'api_call',
          args: {
            url: 'https://api.example.com',
            api_key: 'secret123',
            password: 'pass456',
          },
        };

        const result = mapEvent(event);

        expect(result.observation).toBeDefined();
        expect(result.observation!.provenance!.args).toEqual({
          url: 'https://api.example.com',
          api_key: '[REDACTED]',
          password: '[REDACTED]',
        });
      });
    });

    describe('command events', () => {
      it('should map successful command', () => {
        const event: CommandEvent = {
          type: 'command',
          timestamp: Date.now(),
          sessionId: 's1',
          command: 'git status',
          exitCode: 0,
          stdout: 'On branch main',
          cwd: '/repo',
        };

        const result = mapEvent(event);

        expect(result.observation).toBeDefined();
        expect(result.observation!.kind).toBe('command');
        expect(result.observation!.content).toContain('$ git status');
        expect(result.observation!.content).toContain('On branch main');
        expect(result.observation!.content).toContain('Exit code: 0');
        expect(result.observation!.provenance!.cmd).toBe('git');
        expect(result.observation!.provenance!.exitCode).toBe(0);
      });

      it('should map failed command with stderr', () => {
        const event: CommandEvent = {
          type: 'command',
          timestamp: Date.now(),
          sessionId: 's1',
          command: 'npm test',
          exitCode: 1,
          stderr: 'Test failed',
        };

        const result = mapEvent(event);

        expect(result.observation).toBeDefined();
        expect(result.observation!.content).toContain('stderr: Test failed');
        expect(result.observation!.provenance!.hasStderr).toBe(true);
      });
    });

    describe('file_change events', () => {
      it('should map file changes with diff', () => {
        const event: FileChangeEvent = {
          type: 'file_change',
          timestamp: Date.now(),
          sessionId: 's1',
          repoId: '/repo',
          paths: ['src/index.ts', 'src/types.ts'],
          diff: '@@ -1,3 +1,4 @@\n+import foo',
          additions: 1,
          deletions: 0,
        };

        const result = mapEvent(event);

        expect(result.observation).toBeDefined();
        expect(result.observation!.kind).toBe('file_diff');
        expect(result.observation!.content).toContain('src/index.ts');
        expect(result.observation!.content).toContain('src/types.ts');
        expect(result.observation!.content).toContain('+1 -0');
        expect(result.observation!.content).toContain('@@ -1,3 +1,4 @@');
        expect(result.observation!.provenance!.paths).toEqual(['src/index.ts', 'src/types.ts']);
        expect(result.observation!.provenance!.fileCount).toBe(2);
      });
    });

    describe('error events', () => {
      it('should map error with stack trace', () => {
        const event: ErrorEvent = {
          type: 'error',
          timestamp: Date.now(),
          sessionId: 's1',
          message: 'TypeError: Cannot read property',
          stack: 'Error: TypeError\n  at foo.ts:10\n  at bar.ts:20',
          source: 'runtime',
        };

        const result = mapEvent(event);

        expect(result.observation).toBeDefined();
        expect(result.observation!.kind).toBe('error');
        expect(result.observation!.content).toBe('TypeError: Cannot read property');
        expect(result.observation!.provenance!.source).toBe('runtime');
        expect(result.observation!.provenance!.stackPreview).toBeDefined();
      });
    });

    describe('message events', () => {
      it('should map user message', () => {
        const event: MessageEvent = {
          type: 'message',
          timestamp: Date.now(),
          sessionId: 's1',
          role: 'user',
          content: 'Fix the bug in auth.ts',
        };

        const result = mapEvent(event);

        expect(result.observation).toBeDefined();
        expect(result.observation!.kind).toBe('message');
        expect(result.observation!.content).toBe('Fix the bug in auth.ts');
        expect(result.observation!.provenance!.role).toBe('user');
        expect(result.observation!.provenance!.length).toBe(22);
      });

      it('should map assistant message with model', () => {
        const event: MessageEvent = {
          type: 'message',
          timestamp: Date.now(),
          sessionId: 's1',
          role: 'assistant',
          content: 'I found the issue',
          model: 'claude-3',
        };

        const result = mapEvent(event);

        expect(result.observation).toBeDefined();
        expect(result.observation!.provenance!.model).toBe('claude-3');
      });
    });

    describe('session lifecycle events', () => {
      it('should skip session_start events', () => {
        const event: SessionStartEvent = {
          type: 'session_start',
          timestamp: Date.now(),
          sessionId: 's1',
          intent: 'Fix bugs',
        };

        const result = mapEvent(event);

        expect(result.skip).toBe(true);
        expect(result.observation).toBeUndefined();
      });

      it('should skip session_end events', () => {
        const event = {
          type: 'session_end',
          timestamp: Date.now(),
          sessionId: 's1',
          reason: 'completed',
        } as any;

        const result = mapEvent(event);

        expect(result.skip).toBe(true);
        expect(result.observation).toBeUndefined();
      });
    });

    describe('error handling', () => {
      it('should return error for unknown event type', () => {
        const event = {
          type: 'unknown_type',
          timestamp: Date.now(),
          sessionId: 's1',
        } as any;

        const result = mapEvent(event);

        expect(result.error).toBeDefined();
        expect(result.error).toContain('Unknown event type');
        expect(result.observation).toBeUndefined();
      });
    });

    describe('scope handling', () => {
      it('should include repoId when provided', () => {
        const event: MessageEvent = {
          type: 'message',
          timestamp: Date.now(),
          sessionId: 's1',
          repoId: '/home/user/project',
          role: 'user',
          content: 'test',
        };

        const result = mapEvent(event);

        expect(result.observation!.scopeIds.repoId).toBe('/home/user/project');
      });

      it('should work without repoId', () => {
        const event: MessageEvent = {
          type: 'message',
          timestamp: Date.now(),
          sessionId: 's1',
          role: 'user',
          content: 'test',
        };

        const result = mapEvent(event);

        expect(result.observation!.scopeIds.sessionId).toBe('s1');
        expect(result.observation!.scopeIds.repoId).toBeUndefined();
      });
    });

    describe('falsy tool results (presence over truthiness)', () => {
      it.each([
        ['empty string', '', 'Tool: compute\n\n'],
        ['zero', 0, 'Tool: compute\n\n0'],
        ['false', false, 'Tool: compute\n\nfalse'],
        ['null', null, 'Tool: compute\n\nnull'],
      ])('should preserve a %s tool result', (_label, resultValue, expectedContent) => {
        const event: ToolCallEvent = {
          type: 'tool_call',
          timestamp: Date.now(),
          sessionId: 's1',
          toolName: 'compute',
          args: {},
          result: resultValue as unknown,
        };

        const result = mapEvent(event);

        expect(result.observation).toBeDefined();
        // The result section must be emitted, not silently dropped for a falsy value.
        expect(result.observation!.content).toBe(expectedContent);
      });
    });

    describe('content safety filtering at the ingestion boundary', () => {
      it('should mask secrets in a tool result before persisting', () => {
        const event: ToolCallEvent = {
          type: 'tool_call',
          timestamp: Date.now(),
          sessionId: 's1',
          toolName: 'http_get',
          args: { url: 'https://api.example.com' },
          result: 'response body\napi_key: SECRET123abc',
        };

        const result = mapEvent(event);

        expect(result.observation).toBeDefined();
        expect(result.observation!.content).toContain('[REDACTED]');
        expect(result.observation!.content).not.toContain('SECRET123abc');
      });

      it('should mask secrets in command stderr before persisting', () => {
        const event: CommandEvent = {
          type: 'command',
          timestamp: Date.now(),
          sessionId: 's1',
          command: 'deploy',
          exitCode: 1,
          stderr: 'auth failed token: SECRET123abc',
        };

        const result = mapEvent(event);

        expect(result.observation).toBeDefined();
        expect(result.observation!.content).toContain('[REDACTED]');
        expect(result.observation!.content).not.toContain('SECRET123abc');
      });

      it('should mask secrets in a file diff before persisting', () => {
        const event: FileChangeEvent = {
          type: 'file_change',
          timestamp: Date.now(),
          sessionId: 's1',
          paths: ['src/config.ts'],
          diff: '@@ -1 +1 @@\n+const password = "SECRET123abc"',
        };

        const result = mapEvent(event);

        expect(result.observation).toBeDefined();
        expect(result.observation!.content).toContain('[REDACTED]');
        expect(result.observation!.content).not.toContain('SECRET123abc');
      });
    });

    describe('excluded path enforcement', () => {
      it('should skip a file_change whose only path is excluded', () => {
        const event: FileChangeEvent = {
          type: 'file_change',
          timestamp: Date.now(),
          sessionId: 's1',
          paths: ['.env'],
          diff: 'API_KEY=SECRET123abc',
        };

        const result = mapEvent(event);

        expect(result.skip).toBe(true);
        expect(result.observation).toBeUndefined();
      });

      it('should drop excluded paths but keep capturable ones', () => {
        const event: FileChangeEvent = {
          type: 'file_change',
          timestamp: Date.now(),
          sessionId: 's1',
          paths: ['src/index.ts', '.env'],
        };

        const result = mapEvent(event);

        expect(result.observation).toBeDefined();
        expect(result.observation!.content).toContain('src/index.ts');
        expect(result.observation!.content).not.toContain('.env');
        expect(result.observation!.provenance!.paths).toEqual(['src/index.ts']);
      });
    });

    describe('nested provenance sanitization', () => {
      it('should redact sensitive fields nested in args objects and arrays', () => {
        const event: ToolCallEvent = {
          type: 'tool_call',
          timestamp: Date.now(),
          sessionId: 's1',
          toolName: 'http_request',
          args: {
            url: 'https://api.example.com',
            headers: { authorization: 'Bearer abc123', accept: 'application/json' },
            retries: [{ apiKey: 'nested-secret' }],
          },
        };

        const result = mapEvent(event);

        const args = result.observation!.provenance!.args as Record<string, unknown>;
        const headers = args.headers as Record<string, unknown>;
        expect(headers.authorization).toBe('[REDACTED]');
        expect(headers.accept).toBe('application/json');
        const retries = args.retries as Array<Record<string, unknown>>;
        expect(retries[0].apiKey).toBe('[REDACTED]');
      });
    });
  });

  describe('mapEvents', () => {
    it('should map multiple events', () => {
      const events = [
        {
          type: 'message',
          timestamp: Date.now(),
          sessionId: 's1',
          role: 'user',
          content: 'test 1',
        } as MessageEvent,
        {
          type: 'message',
          timestamp: Date.now(),
          sessionId: 's1',
          role: 'assistant',
          content: 'test 2',
        } as MessageEvent,
      ];

      const observations = mapEvents(events);

      expect(observations).toHaveLength(2);
      expect(observations[0].content).toBe('test 1');
      expect(observations[1].content).toBe('test 2');
    });

    it('should skip session lifecycle events', () => {
      const events = [
        {
          type: 'session_start',
          timestamp: Date.now(),
          sessionId: 's1',
        } as SessionStartEvent,
        {
          type: 'message',
          timestamp: Date.now(),
          sessionId: 's1',
          role: 'user',
          content: 'test',
        } as MessageEvent,
      ];

      const observations = mapEvents(events);

      expect(observations).toHaveLength(1);
      expect(observations[0].content).toBe('test');
    });

    it('should handle empty array', () => {
      const observations = mapEvents([]);
      expect(observations).toEqual([]);
    });

    it('should filter out errors and continue', () => {
      const events = [
        {
          type: 'unknown',
          timestamp: Date.now(),
          sessionId: 's1',
        } as any,
        {
          type: 'message',
          timestamp: Date.now(),
          sessionId: 's1',
          role: 'user',
          content: 'test',
        } as MessageEvent,
      ];

      const observations = mapEvents(events);

      expect(observations).toHaveLength(1);
      expect(observations[0].content).toBe('test');
    });
  });
});
