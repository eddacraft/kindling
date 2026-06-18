// Capture-mapping fixture generator.
//
// Imports the REAL Node adapter mapping/provenance/events/filter (the
// byte-for-byte source of truth) via tsx and emits, for a set of representative
// hook inputs, the expected `{ kind, content, provenance, scopeIds }`. The Rust
// `kindling-hook` mapping must produce byte-identical results (compared as
// serde_json::Value, so object key order is not asserted).
//
// Run from the worktree root with tsx:
//   tsx crates/kindling-hook/tests/fixtures/generate-fixtures.mjs \
//     > crates/kindling-hook/tests/fixtures/capture-cases.json
//
// The committed `capture-cases.json` was generated this way from
// `packages/kindling-adapter-claude-code/src/claude-code/*.ts`.

import {
  createPostToolUseEvent,
  createUserPromptEvent,
  createSubagentStopEvent,
} from '../../../../packages/kindling-adapter-claude-code/src/claude-code/events.ts';
import { mapEvent } from '../../../../packages/kindling-adapter-claude-code/src/claude-code/mapping.ts';

const SESSION = 's1';
const CWD = '/repo';

// Each case names a hook type and the stdin-shaped context the Node hook script
// would receive (snake_case keys). We rebuild the ClaudeCodeEvent exactly as the
// hook scripts do, then run mapEvent.
const cases = [];

function postToolUse(name, ctx) {
  const event = createPostToolUseEvent({
    sessionId: SESSION,
    cwd: CWD,
    toolName: ctx.tool_name,
    toolInput: ctx.tool_input ?? {},
    toolResult: ctx.tool_result,
    toolError: ctx.tool_error,
  });
  const result = mapEvent(event);
  cases.push({
    name,
    hookType: ctx.tool_error && ctx.__failure ? 'post-tool-use-failure' : 'post-tool-use',
    hookInput: { session_id: SESSION, cwd: CWD, ...ctx, __failure: undefined },
    expected: result.observation,
  });
}

function userPrompt(name, content) {
  const event = createUserPromptEvent({ sessionId: SESSION, cwd: CWD, content });
  const result = mapEvent(event);
  cases.push({
    name,
    hookType: 'user-prompt-submit',
    hookInput: { session_id: SESSION, cwd: CWD, content },
    expected: result.observation,
  });
}

function subagentStop(name, ctx) {
  const event = createSubagentStopEvent({
    sessionId: SESSION,
    cwd: CWD,
    agentType: ctx.agent_type,
    output: ctx.output,
  });
  const result = mapEvent(event);
  cases.push({
    name,
    hookType: 'subagent-stop',
    hookInput: { session_id: SESSION, cwd: CWD, ...ctx },
    expected: result.observation,
  });
}

// ---- tool-use cases -------------------------------------------------------

postToolUse('read', { tool_name: 'Read', tool_input: { file_path: '/src/a.rs' } });
postToolUse('read_no_path', { tool_name: 'Read', tool_input: {} });
postToolUse('write', { tool_name: 'Write', tool_input: { file_path: '/src/b.rs' } });
postToolUse('edit', {
  tool_name: 'Edit',
  tool_input: { file_path: '/src/c.rs', old_string: 'foo' },
});
postToolUse('edit_no_old_string', { tool_name: 'Edit', tool_input: { file_path: '/src/c.rs' } });
postToolUse('bash_string_result', {
  tool_name: 'Bash',
  tool_input: { command: 'cargo build --workspace' },
  tool_result: 'Compiling kindling\nFinished',
});
postToolUse('bash_object_result_exit_code', {
  tool_name: 'Bash',
  tool_input: { command: 'ls -la /tmp' },
  tool_result: { exitCode: 0 },
});
postToolUse('bash_exit_code_snake', {
  tool_name: 'Bash',
  tool_input: { command: 'false' },
  tool_result: { exit_code: 1 },
});
postToolUse('bash_no_result', { tool_name: 'Bash', tool_input: { command: 'echo hi' } });
postToolUse('glob', { tool_name: 'Glob', tool_input: { pattern: '**/*.rs', path: '/src' } });
postToolUse('grep', { tool_name: 'Grep', tool_input: { pattern: 'TODO', path: '/src' } });
postToolUse('grep_no_path', { tool_name: 'Grep', tool_input: { pattern: 'fn main' } });
postToolUse('task', {
  tool_name: 'Task',
  tool_input: { subagent_type: 'debugger', description: 'find the bug' },
});
postToolUse('webfetch', { tool_name: 'WebFetch', tool_input: { url: 'https://example.com' } });
postToolUse('websearch', { tool_name: 'WebSearch', tool_input: { query: 'rust uds' } });
postToolUse('unknown_tool', {
  tool_name: 'Frobnicate',
  tool_input: { alpha: 1, beta: 2, gamma: 3 },
});
postToolUse('unknown_tool_empty_input', { tool_name: 'Mystery', tool_input: {} });

// error variants
postToolUse('read_with_error', {
  tool_name: 'Read',
  tool_input: { file_path: '/missing' },
  tool_error: 'ENOENT: no such file',
});
postToolUse('bash_with_error', {
  tool_name: 'Bash',
  tool_input: { command: 'exit 1' },
  tool_result: { exitCode: 1 },
  tool_error: 'command failed',
  __failure: true,
});

// ---- user prompt cases ----------------------------------------------------

userPrompt('user_prompt_simple', 'Please refactor the parser.');
userPrompt('user_prompt_multiline', 'line one\nline two\n\nline four');

// ---- subagent stop cases --------------------------------------------------

subagentStop('subagent_with_output', {
  agent_type: 'code-reviewer',
  output: 'Reviewed 3 files. LGTM.',
});
subagentStop('subagent_no_output', { agent_type: 'planner' });
subagentStop('subagent_unknown_type', { output: 'did some work' });

process.stdout.write(JSON.stringify(cases, null, 2) + '\n');
