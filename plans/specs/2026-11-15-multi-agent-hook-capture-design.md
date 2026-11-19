# Multi-Agent Hook Capture: Codex, Grok Build, Antigravity

| Field      | Value                                                                                           |
| ---------- | ----------------------------------------------------------------------------------------------- |
| Status     | Proposed                                                                                        |
| Owner      | TBD                                                                                             |
| Created    | 2026-11-15                                                                                      |
| APS Module | `plans/modules/08-conversion-surface.aps.md` (sits alongside C13); new work items to be drafted |
| Work Item  | None yet (see [Proposed work items](#proposed-work-items))                                      |

## Context

kindling captures coding-agent activity through **hooks**. A hook is a command
the agent runs at a lifecycle event (session start, tool call, prompt submit,
compaction, stop). It receives a JSON payload on stdin and can return JSON on
stdout. Today only Claude Code is wired up:

- **Hook entry point:** `kindling hook <type>` (`crates/kindling/src/hook/`).
- **Input:** it parses Claude Code's flat snake_case payload: `session_id`,
  `cwd`, `tool_name`, `tool_input`, `tool_response`, `prompt`, `error`, …
  (`HookInput`, `crates/kindling/src/hook/input.rs`).
- **Classification:** it maps tool calls to observation kinds by Claude Code's
  tool names. `Bash` becomes `command`, `Write`/`Edit` become `file_diff`, and
  `Read`, `Glob`, `Grep`, `Task` and `WebFetch` get custom formatting. Any other
  name falls back to a generic `tool_call`
  (`crates/kindling/src/hook/mapping.rs`).
- **Output:** it injects context with Claude Code's reply shape,
  `{"continue": true, "hookSpecificOutput": {"hookEventName": …, "additionalContext": <markdown>}}`
  (`crates/kindling/src/hook/mod.rs`).
- **Packaging:** the Claude Code plugin (`plugins/kindling-claude-code/`)
  registers those hooks. Users install it explicitly with `/plugin marketplace add` and
  `/plugin install`. `kindling init --claude-code` never edits `~/.claude/`.

Other agents are covered only partly or not at all. VS Code, Cursor and
Windsurf capture file saves through the VSIX, which records no agent activity.
OpenCode has a source-only adapter, and C13 plans to bundle it into the binary.
Codex, Grok Build and Antigravity have no capture path.

All three now ship hook systems. This spec records what each would need,
based on research done on 2026-11-14/15 (sources below).

## Decision (proposed)

Generalize the hook entry point instead of writing a separate adapter per agent.
The pieces that matter are already provider-neutral and stay as they are: mapping
to observations, secret masking at the service boundary, capsule lifecycle, and
building injection context. Each new agent then needs only:

1. an **input normalizer** (agent payload → internal hook event),
2. a **tool-name table** (agent tool names → observation kind and formatter),
3. an **output formatter** (injection context → the agent's reply shape),
4. an **opt-in installer** (`kindling init --<agent>`).

Record the originating agent in `scopeIds.agentId`. That field already exists,
has a store column and is retrieval-filterable, so memory from different agents
in one project stays distinguishable without a schema change. No in-repo
integration sets it today: the Claude Code hook leaves it unset
(`crates/kindling/src/hook/mod.rs`), and the TS adapters don't use it.

`agentId` is for **attribution and explicit filters only**. SessionStart and
PreCompact injection stay scoped to the project and do **not** filter by
`agentId`, so a Codex session sees context captured under Claude Code and the
other way round. That cross-tool continuity is the reason to do this instead of
relying on each agent's own memory.

Values are the `--agent` selector names: `claude`, `codex`, `grok`,
`antigravity`. Claude Code observations start carrying `agentId: "claude"` when
the normalization layer lands (see [Sequencing](#sequencing)). Observations
captured before then keep `agentId = null`. There is no backfill: an explicit
`agentId=claude` filter won't match older Claude Code history, and the
integration docs say so. Rewriting stored scope after the fact would be a
retention/mutation policy change, which is out of scope here.

## Goals

- Capture Codex and Grok Build sessions with the same fidelity as Claude Code.
- Make the Antigravity path possible, behind an explicit experimental flag.
- Keep one canonical mapping and masking pipeline. No per-agent forks of the
  filter logic.
- Make every installer opt-in and transparent: print exactly what is written
  where, re-run safely, and provide an uninstall.

## Non-Goals

- Editing another tool's config without an explicit `init --<agent>` invocation.
- Replacing or interoperating with agents' built-in memory features (Codex now
  ships one; see Findings).
- Schema changes. `agentId` and the existing observation kinds are sufficient.
- Cursor and Gemini CLI. They are worth a follow-up pass, but are out of scope
  here.

## Findings

### Summary

| Agent       | Payload vs Claude Code                               | Tool names                              | Context injection                                                | Effort | Confidence                                                |
| ----------- | ---------------------------------------------------- | --------------------------------------- | ---------------------------------------------------------------- | ------ | --------------------------------------------------------- |
| Codex       | Near-identical (snake_case, same field names)        | Claude-like, plus `apply_patch`         | `hookSpecificOutput.additionalContext`                           | S      | High (source)                                             |
| Grok Build  | camelCase with snake_case aliases of the main fields | Grok-native (`run_terminal_command`, …) | `additional_context` (envelope unconfirmed, see open question 2) | S–M    | High for capture (source); injection envelope unconfirmed |
| Antigravity | Different: nested camelCase `toolCall{name,args}`    | `run_command`, `write_to_file`, …       | `PreInvocation` → `injectSteps` (reported)                       | M–L    | Low (third-party, closed source)                          |

### Codex (OpenAI)

Source: `openai/codex` at commit `de83e76`
(hook schemas in `codex-rs/hooks/schema/generated/`, discovery in
`codex-rs/hooks/src/engine/discovery.rs`).

- **Events:**
  - Lifecycle: `SessionStart`, `SessionEnd`, `UserPromptSubmit`, `Stop`, `Interrupt`.
  - Tools and permissions: `PreToolUse`, `PostToolUse`, `PermissionRequest`.
  - Compaction: `PreCompact`, `PostCompact`.
  - Subagents: `SubagentStart`, `SubagentStop`.
  - `ModelRequest`.
  - There is **no `PostToolUseFailure`**.
- **Input fields:** `session_id`, `cwd`, `transcript_path`, `hook_event_name`, `model`,
  `permission_mode`, `turn_id`, `agent_id`, `agent_type`, `tool_name`,
  `tool_input`, `tool_response`, `tool_use_id`, `prompt`,
  `last_assistant_message`, `agent_transcript_path`, `source`, `reason`,
  `trigger`, `stop_hook_active`. This is a superset of what `HookInput` reads.
- **Output:** the same shape as Claude Code (`continue`, `systemMessage`,
  `suppressOutput`, `decision`, `hookSpecificOutput.additionalContext`).
- **Tool names:**
  - Shell calls arrive as `Bash`.
  - **Edits arrive as `tool_name: "apply_patch"`**. Matchers treat `Write`/`Edit` as
    aliases, but the payload carries `apply_patch`
    (`codex-rs/hooks/src/events/post_tool_use.rs`,
    `codex-rs/hooks/src/engine/dispatcher.rs`).
  - MCP tools use `mcp__<server>__<tool>`.
- **Config locations:** `~/.codex/hooks.json`, a `[hooks]` table in
  `~/.codex/config.toml`, or the project's `.codex/` equivalents. Codex plugins
  (`.codex-plugin/plugin.json`) can bundle hooks.
- **Consent:** non-managed hooks don't run until the user has reviewed and
  trusted the exact hook definition.
- **Overlap:** Codex ships built-in, model-generated memories
  (`codex-rs/memories/`). kindling's position is deterministic, local and
  explainable memory shared across tools. That is complementary, but the docs
  should say so explicitly.

**kindling needs:**

- Payload: none; the current parser accepts it.
- Tool names: map `apply_patch` to `file_diff` and format its patch input.
- Failures: detect them in `PostToolUse` (non-zero exit or an error in
  `tool_response`), because there is no failure event.
- Install: `kindling init --codex` writes a hooks file, or kindling ships a
  `.codex-plugin`.

### Grok Build (xAI)

Source: `xai-org/grok-build` at commit `2bdd1d6`, open-sourced under Apache-2.0
in July 2026 (`crates/codegen/xai-grok-hooks/src/{event,discovery,matcher,result}.rs`).

- **Events:**
  - Lifecycle: `SessionStart`, `SessionEnd`, `UserPromptSubmit`, `Stop`,
    `StopFailure`, `Notification`.
  - Tools and permissions: `PreToolUse`, `PostToolUse`, `PostToolUseFailure`,
    `PermissionDenied`.
  - Subagents: `SubagentStart`, `SubagentStop` (with the legacy alias `SubagentEnd`).
  - Compaction: `PreCompact`, `PostCompact`.
  - Grok accepts Claude PascalCase, snake_case and Cursor camelCase event names.
- **Input:** the envelope is camelCase (`hookEventName`, `sessionId`, `cwd`,
  `workspaceRoot`, `timestamp`, `transcriptPath`, `permissionMode`) with the
  event payload flattened into it (`toolName`, `toolInput`, `toolResult`,
  `toolUseId`, `prompt`, `error`, `lastAssistantMessage`, …). Grok **adds
  snake_case aliases** for the main keys: `hook_event_name`, `session_id`,
  `transcript_path`, `permission_mode`, `tool_name`, `tool_input`, `tool_response`
  (from `toolResult`), `tool_use_id`. As a result `HookInput` already
  deserializes the fields kindling uses.
- **Tool names are Grok-native**, for example `run_terminal_command`. Grok's
  _matchers_ expand Claude names (`"Bash"` matches `run_terminal_command`), but
  the _payload_ carries the Grok name. Without a table, every Grok tool call
  falls through to a generic `tool_call`. Grok keeps an alias registry
  (`xai_grok_tools::types::claude_names_for`) that kindling can mirror.
- **Output:** results carry `additional_context`. It's unconfirmed whether
  that field is read at the top level or nested in `hookSpecificOutput`, and
  whether camelCase `additionalContext` is also accepted (open question 2). If
  kindling sends the wrong key, Grok drops the context silently, so the effort
  and confidence ratings above cover capture only, not injection.
- **Config locations:** a directory of hook files at `~/.grok/hooks/*.json`, the
  project's `.grok/hooks/`, and Grok plugins in `.grok/plugins/` or
  `~/.grok/plugins/`.
- **Compatibility caveat:** Grok also reads hooks from `~/.claude/settings.json`,
  `settings.local.json` and `.cursor/hooks.json`. But kindling's Claude Code
  hooks are installed **through a Claude plugin**, not `settings.json`, and in
  the discovery code reviewed Grok doesn't load Claude plugins. So kindling's
  existing Claude hooks do **not** fire under Grok.
- **Consent:** project hooks require `/hooks-trust` (or `--trust`) before they
  run.

**kindling needs:**

- Payload: none beyond confirming the alias behaviour with a fixture.
- Agent detection: recognise a Grok payload regardless of `--agent` (see
  [Agent detection](#agent-detection)), because a kindling hook in
  `~/.claude/settings.json` also fires under Grok.
- Tool names: add a Grok tool-name table.
- Injection: confirm the reply key and nesting (open question 2) before
  claiming SessionStart support.
- Install: `kindling init --grok` writes `~/.grok/hooks/kindling.json`, or
  kindling ships a Grok plugin. It checks for a kindling hook in
  `~/.claude/settings.json` / `settings.local.json` and warns that Grok will run
  both.

### Antigravity (Google)

Source: third-party guides only. Antigravity is closed-source, and its official
docs couldn't be fetched from the research environment (see Open questions).
**Treat every detail here as unverified until it is tested against a real
`agy` binary.**

- **Events (documented):** `PreToolUse`, `PostToolUse`, `PreInvocation`,
  `PostInvocation`, `Stop`. `SessionStart` reportedly works but isn't
  documented. There is no `UserPromptSubmit` or `SessionEnd`.
- **Input:** nested camelCase. `toolCall: { name, args }`, `conversationId`,
  `workspacePaths[]`, `modelName`, `stepIdx`. Argument keys are PascalCase, for
  example `.toolCall.args.CommandLine`.
- **Tool names:**
  - Shell: `run_command`.
  - Files: `write_to_file`, `replace_file_content`, `multi_replace_file_content`,
    `view_file`, `list_dir`, `grep_search`.
  - MCP calls go through `call_mcp_tool`.
- **Output:** a decision (`decision` / `allow_tool`) for gating. Context injection
  is reportedly done by `PreInvocation` returning `injectSteps` with
  `ephemeralMessage`, not `additionalContext`.
- **Config locations:**
  - Workspace: `.agents/hooks.json`.
  - Global: sources disagree. `~/.gemini/antigravity-cli/hooks.json` (CLI) and
    `~/.gemini/config/hooks.json` (IDE) are both reported.
  - Plugins: `~/.gemini/antigravity-cli/plugins/<name>/hooks.json`.
- **Gotchas:** `command` must be an **absolute path**, because relative paths
  resolve against the launch directory. Local hooks only run in a **trusted
  workspace**.

**kindling needs:**

- Payload: a dedicated input normalizer.
- Session lifecycle: infer it. Open the capsule on the first event of a
  `conversationId`, and close it on `Stop`. A conversation that ends without a
  `Stop` needs a separate mechanism. Hooks only run when an event fires, and
  neither the daemon nor the service has an idle sweep for capsules, so "close
  on idle" can't happen by itself. Two options:
  - **(a) Hook-side, preferred for the experimental release:** when a new
    `conversationId` opens in a project, close any capsule still open for an
    earlier Antigravity conversation in the same project. This needs no new
    service behaviour. The last capsule stays open until the next conversation.
  - **(b) Daemon-side idle sweep:** this is new lifecycle policy. It would get
    its own work item and go through discuss-first.
- Tool names: a tool-name table.
- Output: an `injectSteps` formatter for `PreInvocation`.
- Install: an installer that writes absolute paths.
- Release: ship behind `--experimental` until fixtures from a real binary exist.

## Design

### Hook entry point

Add an agent selector to the existing command, defaulting to `claude` so
current plugin installs keep working unchanged:

```
kindling hook [--agent claude|codex|grok|antigravity] <type>
```

Agent-native event names map onto kindling's existing hook types
(`session-start`, `post-tool-use`, `post-tool-use-failure`,
`user-prompt-submit`, `subagent-stop`, `stop`, `pre-compact`). Antigravity also
needs a `pre-invocation` handler, which carries its context injection.

### Agent detection

`--agent` is a hint, not ground truth. One installed hook can be run by more
than one agent: Grok reads `~/.claude/settings.json`, `settings.local.json`
and `.cursor/hooks.json`. So a kindling hook wired there for Claude Code also
fires for Grok sessions. Under `--agent claude` it would tag Grok events
`agentId=claude` and run them through the Claude tool table, where every
Grok-native tool name falls back to a plain `tool_call`.

The normalizer therefore identifies the agent from the payload where it can:

- **Grok:** the envelope carries camelCase-only keys (`hookEventName`,
  `workspaceRoot`) alongside the snake_case aliases. A Claude Code payload
  never has these.
- **Antigravity:** nested `toolCall` and `conversationId`.
- **Codex vs Claude Code:** doesn't need detection today, because Codex only
  reads its own `~/.codex` / `.codex/` config. Codex-only fields such as
  `turn_id` are available if that changes. Confirm them with fixtures.

When the payload signature and `--agent` disagree, the payload wins. The hook
records the mismatch in provenance (`agentHint`) so a misconfigured install
shows up in the data and isn't silently mislabelled.

### Normalization

```
agent stdin JSON ──► normalize(agent) ──► HookEvent (internal)
                                            │  session key, cwd, event,
                                            │  tool {name, canonical, input, result, error},
                                            │  prompt, agent
                                            ▼
                         existing mapping.rs / filter / ObservationInput
                                            │  scopeIds.agentId = <agent>
                                            ▼
                              kindling-client → daemon (masking on append)
```

- Claude Code, Codex and Grok share one serde path, because Grok supplies
  snake_case aliases. Antigravity gets its own normalizer.
- The **session key** comes from `session_id`. For Antigravity it comes from
  `conversationId`.
- **Project-root resolution** is unchanged. It still uses `KINDLING_REPO_ROOT`,
  then the git toplevel of `cwd`. For Antigravity, use the first entry of
  `workspacePaths`.

### Observation ids

Today the hook sends `id: None` for every observation
(`crates/kindling/src/hook/mapping.rs`), so the daemon generates a fresh id
each time. The store's `INSERT OR IGNORE` makes replay of the _same_ id
idempotent (spool replay relies on this). But two hooks capturing the same
event produce two ids and two rows. Nothing is deduplicated across hooks today.

To make a double-wired install safe, tool events get a deterministic id
derived from **`(session key, tool_use_id)`**. The agent is deliberately left
out of the key. In the double-capture case the two copies may disagree on the
agent: a `settings.json` hook run with `--agent claude` and a Grok hook run
with `--agent grok`. Including the agent in the key would keep both rows.

- The id is a stable hash with a fixed prefix, so it can't collide with
  daemon-generated ids. The first write wins, and the second copy is a no-op
  in both `observations` and `capsule_observations`.
- Events with no `tool_use_id` (user prompts, subagent stops) keep
  daemon-generated ids. Hashing their content would merge legitimately
  repeated identical prompts. A double-wired Grok install can still duplicate
  these, and the installer warning is the guard for that case.
- This changes Claude Code observation ids too (Claude Code supplies
  `tool_use_id`), so it is its own work item (AGENT-7). It stays out of
  AGENT-1, whose only intended behaviour change is the `agentId` tag.

### Tool-name tables

Each agent gets a table that maps its tool names to a canonical kindling tool.
The canonical set is `Bash`, `Write`, `Edit`, `Read`, `Glob`, `Grep`, `Task`,
`WebFetch`, `Mcp`. `mapping.rs` then classifies and formats by the canonical
name. Unknown names keep today's behaviour and fall through to `tool_call`,
with the raw name recorded in provenance.

| Canonical | Codex         | Grok Build (to confirm against `claude_names_for`) | Antigravity                                          |
| --------- | ------------- | -------------------------------------------------- | ---------------------------------------------------- |
| Bash      | `Bash`        | `run_terminal_command`                             | `run_command`                                        |
| Edit      | `apply_patch` | Grok edit tool(s)                                  | `replace_file_content`, `multi_replace_file_content` |
| Write     | `apply_patch` | Grok write tool                                    | `write_to_file`                                      |
| Read      | n/a           | `read_file`                                        | `view_file`                                          |
| Grep/Glob | n/a           | Grok search tools                                  | `grep_search`, `list_dir`                            |
| Mcp       | `mcp__*__*`   | `<server>__<tool>`                                 | `call_mcp_tool`                                      |

### Output formatting

| Agent              | session-start / pre-compact reply                                                                                                                                                                                                                                                                                       |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Claude Code, Codex | `{"continue":true,"hookSpecificOutput":{"hookEventName":…,"additionalContext":…}}` (today's)                                                                                                                                                                                                                            |
| Grok Build         | **Unconfirmed.** Grok's results carry snake_case `additional_context`. Whether it is read top-level or from `hookSpecificOutput`, and whether camelCase is also accepted, decides whether today's envelope works or context is silently dropped (open question 2). Don't claim Grok injection until a fixture proves it |
| Antigravity        | `PreInvocation` → `injectSteps` / `ephemeralMessage` (shape to be confirmed)                                                                                                                                                                                                                                            |

The daemon still owns the injection Markdown (`kindling-server/src/inject.rs`).
Only the JSON envelope differs per agent.

### Installers and consent

Each installer is `kindling init --<agent>` and follows the C13 pattern:

- **Opt-in only.** Never triggered by `install.sh` without a prompt, and never
  run by plain `kindling init`.
- **Writes one clearly named file**, for example `~/.codex/hooks.json` entries
  tagged `kindling`, `~/.grok/hooks/kindling.json`, or
  `.agents/hooks.json` entries.
- **Prints exactly what it wrote and where.** It is idempotent and upgrades a
  stale copy.
- **`kindling init --<agent> --uninstall`** removes exactly what it added.
- **Respects the agent's own trust gate.** The installer tells the user they
  will be asked to trust the hook (Codex review, Grok `/hooks-trust`,
  Antigravity trusted workspace). It never bypasses the gate, for example with
  `--trust` or by editing trust stores.
- **Writes a stable absolute path to the `kindling` binary**, which Antigravity
  requires and is harmless elsewhere. Stable means the path the user's install
  channel keeps fixed across upgrades: the `PATH`-resolved entry such as
  `/opt/homebrew/bin/kindling`, `~/.cargo/bin/kindling` or the `install.sh`
  location. It is never `current_exe()` canonicalized, which can resolve to a
  versioned Homebrew Cellar path. Codex trusts the _exact_ hook definition, so a
  path that changes on every `brew upgrade` would make the user re-trust the
  hook after each upgrade. For the same reason the installer rewrites an
  existing entry only when its content actually changes. If no stable path can
  be found, it says so and asks for `--bin <path>`.

Where an agent has a plugin system (Codex, Grok, Antigravity), a plugin
distributed the same way as the Claude Code one is an alternative to the
installer. Prefer the installer at first: it ships in the binary and doesn't
need a marketplace for each agent.

User-facing docs for every agent must say plainly what is recorded:
prompts, tool calls with their inputs and outputs, and errors. Secrets are
masked, and all data stays local.

## Sequencing

1. **Normalization layer + `--agent` flag + agent detection** (Claude Code
   only). This is a refactor with **one** intended behaviour change: Claude
   Code observations gain `agentId: "claude"`, and the `capture_mapping`
   fixtures are updated to match. Everything else in the `ObservationInput`
   and the injection reply stays byte-for-byte the same (`parity.rs`). Earlier
   history keeps `agentId = null` (see [Decision](#decision-proposed)).
2. **Deterministic tool-event ids** (`(session key, tool_use_id)`; see
   [Observation ids](#observation-ids)). Lands before Grok, because Grok is the
   agent that can double-fire.
3. **Codex:** the `apply_patch` mapping, failure detection, `init --codex`,
   fixtures recorded from a real Codex run.
4. **Grok Build:** the tool-name table, the injection envelope confirmed by
   fixture, `init --grok` with the `settings.json` warning, fixtures from a
   real run.
5. **Antigravity (experimental):** verify the payload shape against a real `agy`
   binary first. Then the normalizer, `pre-invocation` formatter, hook-side
   capsule close (option (a) above) and `init --antigravity --experimental`.
6. **Docs:** add each agent to `docs/integrations.md` with an honest "what gets
   recorded" note and uninstall steps. Update `CONTEXT.md` (agent),
   `docs/architecture.md` (hook entry point) and `docs/development.md` (a
   recipe for adding an agent).

## Validation

- **Fixtures:** per-agent payloads captured from real runs go in
  `crates/kindling/tests/fixtures/`, alongside the existing hook payload
  fixtures. `capture_mapping.rs`-style tests assert the resulting
  `ObservationInput` (kind, content, provenance, `agentId`).
- **Agent detection:** a Grok payload delivered to `kindling hook --agent claude`
  is tagged `agentId=grok`, uses the Grok tool table and records the
  `agentHint` mismatch.
- **Double capture:** the same Grok tool event appended through both a
  `--agent claude` and a `--agent grok` hook leaves one observation.
- **Cross-agent injection:** SessionStart for a Codex session returns context
  captured under Claude Code in the same project.
- **Injection:** golden tests for each agent's reply envelope. Claude Code's
  reply stays byte-for-byte unchanged (`parity.rs`).
- **Installer stability:** re-running `init --<agent>` after a binary upgrade
  through the same channel leaves the agent's config byte-identical.
- **End to end, per agent:** a fresh project, the binary only, then
  `kindling init --<agent>`, one session, and `kindling search` finds the
  session's tool calls scoped to that `agentId`.
- **Uninstall:** after `--uninstall` the agent's config matches its pre-install
  state.

## Risks

- **Schema churn.** All three hook systems are less than a year old and have
  renamed events before (Codex's `AfterToolUse` became `PostToolUse`).
  Mitigation: permissive serde, aliases, fixtures pinned to agent versions, and
  failing open, as the Claude Code hooks already do.
- **Antigravity opacity.** It's closed-source, and its documentation is
  inconsistent (config paths, undocumented `SessionStart`). Mitigation: keep it
  experimental and verify against the binary before committing to a design.
- **Double capture.** Grok reads Claude Code's `settings.json` hooks. If a user
  wires kindling into `settings.json` _and_ installs a Grok hook, Grok sessions
  could be captured twice, and the `settings.json` copy would mislabel them as
  Claude Code. Mitigation: payload-based [agent detection](#agent-detection)
  fixes the label. Deterministic tool-event ids from
  `(session key, tool_use_id)` collapse the duplicate rows through the store's
  existing `INSERT OR IGNORE` (see [Observation ids](#observation-ids)). Both
  are new work; today every hook observation gets a fresh daemon-generated id.
  Prompts and subagent stops can still duplicate, so `init --grok` warns when it
  finds a kindling hook in `settings.json`.
- **Overlap with built-in memory** (Codex). Mitigation: positioning and docs,
  not code.
- **Consent drift.** Installers that write into other tools' config are a trust
  boundary. Mitigation: the installer rules above, and no silent installs from
  `install.sh`.

## Open questions

1. Grok's full tool-name list. Mirror `claude_names_for`, or confirm it from a
   live session?
2. Does Grok read `additional_context` from a nested `hookSpecificOutput` for
   settings-style hooks, or only top-level? Does it also accept camelCase
   `additionalContext`? This blocks Grok injection support, not capture.
3. Codex `apply_patch` input. Can the patch text be parsed into per-file diffs
   for richer `file_diff` content?
4. Antigravity: confirm the payload, the `injectSteps` shape, the global config
   path for each surface (IDE vs CLI), and whether `SessionStart` is supported.
5. Ship installers, per-agent plugins, or both?
6. Should anvil sessions run under these agents also tag `agentId`, so that
   anvil can attribute captured activity?
7. The official docs couldn't be fetched from the research environment:
   `developers.openai.com`, `docs.x.ai`, `antigravity.google`, `symposium.dev`
   and `atamel.dev` were outside its network allowlist. Re-check the Codex and
   Grok findings against the official docs, and the Antigravity findings in
   particular, before implementation starts.

## Proposed work items

To be drafted in an APS module, either `08-conversion-surface` or a new module
once this spec is accepted:

| ID (TBD) | Title                                                                                                       | Depends on       |
| -------- | ----------------------------------------------------------------------------------------------------------- | ---------------- |
| AGENT-1  | Hook normalization layer + `kindling hook --agent` + agent detection; Claude Code gains `agentId: "claude"` | n/a              |
| AGENT-2  | Codex capture (`apply_patch`, failure detection, `init --codex`)                                            | AGENT-1          |
| AGENT-3  | Grok Build capture (tool-name table, injection envelope, `init --grok` + `settings.json` warning)           | AGENT-1, AGENT-7 |
| AGENT-4  | Antigravity verification spike against a real `agy`                                                         | n/a              |
| AGENT-5  | Antigravity capture, experimental (incl. hook-side capsule close)                                           | AGENT-1, AGENT-4 |
| AGENT-6  | Integration docs: what gets recorded, uninstall, `agentId` history caveat, `CONTEXT.md`                     | AGENT-2+         |
| AGENT-7  | Deterministic tool-event observation ids (`(session key, tool_use_id)`)                                     | AGENT-1          |

## Sources

### Primary (source code, read directly)

- `openai/codex` at [`de83e76`](https://github.com/openai/codex/tree/de83e7674a69b9ae18544dd1fbf53f31bfe8c640):
  - [hook payload schemas](https://github.com/openai/codex/tree/de83e7674a69b9ae18544dd1fbf53f31bfe8c640/codex-rs/hooks/schema/generated)
  - [hook discovery](https://github.com/openai/codex/blob/de83e7674a69b9ae18544dd1fbf53f31bfe8c640/codex-rs/hooks/src/engine/discovery.rs)
  - [`post_tool_use.rs`](https://github.com/openai/codex/blob/de83e7674a69b9ae18544dd1fbf53f31bfe8c640/codex-rs/hooks/src/events/post_tool_use.rs) (`apply_patch` tool name)
  - [`dispatcher.rs`](https://github.com/openai/codex/blob/de83e7674a69b9ae18544dd1fbf53f31bfe8c640/codex-rs/hooks/src/engine/dispatcher.rs)
  - [plugin manifest](https://github.com/openai/codex/blob/de83e7674a69b9ae18544dd1fbf53f31bfe8c640/codex-rs/core-plugins/src/agent_plugin_manifest.rs)
  - [built-in memories](https://github.com/openai/codex/tree/de83e7674a69b9ae18544dd1fbf53f31bfe8c640/codex-rs/memories)
- `xai-org/grok-build` at [`2bdd1d6`](https://github.com/xai-org/grok-build/tree/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8):
  - [`event.rs`](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-hooks/src/event.rs) (events, envelope, snake_case aliases)
  - [`discovery.rs`](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-hooks/src/discovery.rs) (`.grok/hooks`, `.claude/settings.json`, `.cursor/hooks.json`)
  - [`matcher.rs`](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-hooks/src/matcher.rs) (Claude tool-name aliasing)
  - [`result.rs`](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-hooks/src/result.rs) (`additional_context`)
  - [example hooks](https://github.com/xai-org/grok-build/tree/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-hooks/examples)
- [A Developer's Guide to Agent Hooks in Antigravity CLI (gist, tanaikech)](https://gist.github.com/tanaikech/004f4bcdb530f1a07706c85c13dc8205): payload paths, tool names, config paths, absolute-path requirement

### Official documentation (not reachable from the research environment; to verify)

- [Codex configuration reference](https://developers.openai.com/codex/config-reference)
- [Codex hooks](https://developers.openai.com/codex/hooks)
- [Grok Build hooks](https://docs.x.ai/build/features/hooks) ([Markdown](https://docs.x.ai/build/features/hooks.md))
- [xAI: Grok Build open source](https://x.ai/blog/grok-build-open-source)
- [Antigravity hooks](https://antigravity.google/docs/hooks/)
- [Antigravity plugins](https://antigravity.google/docs/plugins/)
- [Antigravity: build plugins](https://antigravity.google/docs/build-plugins/)
- [Antigravity marketplace](https://antigravity.google/docs/marketplace/)
- [Antigravity feature overview](https://antigravity.google/docs/features/)
- [Antigravity overview](https://antigravity.google/docs/overview/)
- [Getting Started with Antigravity IDE (Google Codelabs)](https://codelabs.developers.google.com/getting-started-agy-ide)

### Secondary (third-party write-ups)

Codex:

- [Codex CLI Hooks: Lifecycle Governance (Daniel Vaughan)](https://codex.danielvaughan.com/2026/05/17/codex-cli-hooks-lifecycle-pretooluse-posttooluse-governance-enforcement/)
- [codex-hooks reference (CodeAlive)](https://github.com/codealive-ai/ai-driven-development/blob/main/skills/hooks-management/references/codex-hooks.md)
- [Codex Hooks Make the Harness Real (Blake Crosley)](https://blakecrosley.com/blog/codex-hooks-make-the-harness-real)
- [Codex CLI Guide 2026 (Blake Crosley)](https://blakecrosley.com/guides/codex)
- [Codex CLI (Symposium)](https://symposium.dev/design/agent-details/codex-cli.html): the "experimental, disabled by default" framing there is outdated
- [Codex (AI agent), Wikipedia](<https://en.wikipedia.org/wiki/OpenAI_Codex_(AI_agent)>)
- Unreliable, as they contradict the source schema: [codegateway.dev guide](https://www.codegateway.dev/en/blog/openai-codex-cli-complete-guide-2026), [neura.market guide](https://www.neura.market/ai-agents/resources/guides/codex-guide)

Grok Build:

- [Grok Build Developer Guide (Developers Digest)](https://www.developersdigest.tech/blog/grok-build-developer-guide-2026)
- [Grok Build CLI (Mervin Praison)](https://mer.vin/2026/05/grok-build-cli-xai-terminal-coding-agent-with-plan-mode-subagents-and-headless-ci/)
- [Grok CLI / Grok Build (innFactory)](https://innfactory.ai/en/ai-harness/grok-cli/)
- [Grok Build skills and connectors guide (Codersera)](https://codersera.com/blog/xai-grok-build-skills-connectors-guide-2026/)
- [Grok Build Explained (Layer3 Labs)](https://www.layer3labs.io/guides/grok-build-explained)
- [Grok Build CLI: install, features, safety (Kingy AI)](https://kingy.ai/news/xai-drops-grok-build-an-agentic-cli-that-wants-to-live-in-your-terminal/)
- [Grok CLI: official build vs community repo](https://grok-cli.github.io/)
- [Grok Build Goes Open Source (Daniel Vaughan)](https://codex.danielvaughan.com/2026/08/01/grok-build-open-source-apache-2-codex-cli-architecture-acp-tool-porting-terminal-agent-wars/)
- [Grok Build open source (Appwrite)](https://appwrite.io/blog/post/grok-build-open-source)
- [Grok Build release notes (Tech Dev Notes)](https://techdevnotes.com/releases/grok-build/1.0.13)
- [Grok Build, Wikipedia](https://en.wikipedia.org/wiki/Grok_Build)

Antigravity:

- [Antigravity Hooks Reference (Agentic Control Plane)](https://agenticcontrolplane.com/blog/antigravity-hooks-reference)
- [agy permissions (Agentic Control Plane)](https://agenticcontrolplane.com/controls/antigravity)
- [Antigravity CLI (Symposium)](https://symposium.dev/design/agent-details/antigravity-cli.html): `SessionStart` undocumented but working; `injectSteps` as the `additionalContext` equivalent
- [Where does Antigravity look for Hooks? (Mete Atamel)](https://atamel.dev/posts/2026/07-16_where_agy_hooks/)
- [A Developer's Guide to Agent Hooks in Antigravity CLI (Medium)](https://medium.com/google-cloud/a-developers-guide-to-agent-hooks-in-antigravity-cli-4c1440febd11)
- [How to Build Your Own Google Antigravity Plugin (DEV Community)](https://dev.to/gde/how-to-build-your-own-google-antigravity-plugin-packaging-skills-mcp-servers-subagents-rules-23p9)
- [Setting Up Google Antigravity: Rules, Skills and MCP (vibecodes)](https://vibecodes.net/blog/setting-up-google-antigravity-rules-skills-mcp)
- [AntiGravity full guide (CodeMeetAI)](https://codemeetai.substack.com/p/antigravity-full-guide-from-install)
- [Mastering Hooks in Coding Agents (X article)](https://x.com/danicat83/article/2067182206934557099)

Reference:

- [Claude Code hooks reference](https://code.claude.com/docs/en/hooks): the baseline format kindling implements today
