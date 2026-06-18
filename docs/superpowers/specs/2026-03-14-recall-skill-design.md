# Recall Skill Design

**Date:** 2026-03-14
**Location:** `plugins/kindling-claude-code/skills/recall/SKILL.md`
**Status:** Design approved, pending implementation

## Overview

A Claude Code skill that teaches agents how to retrieve and use local memory from kindling. The skill is a retrieval protocol — it defines when to check memory, how to query, how to interpret results, and when to pin findings. It targets agents primarily but is usable by anyone.

The skill orchestrates existing `/memory` commands (`search`, `status`, `pin`, `pins`, `unpin`) — no new tooling required. The skill's description must not conflict with existing command descriptions — the commands are tools, the skill is the protocol for using them.

## Approach

Protocol-style skill with embedded examples. Defines rules and principles the agent internalizes, grounded by concrete scenarios that show correct behavior.

## Skill Identity

- **Name:** `recall`
- **Plugin:** `kindling` (the Claude Code plugin at `plugins/kindling-claude-code/`)
- **Invocation:** `/kindling:recall <query>` (explicit) or auto-invoked by triggers
- **Auto-invocation:** Enabled — no `disable-model-invocation`
- **Tool surface:** Existing `/memory` commands (search, status, pin, pins, unpin)

### Frontmatter

```yaml
---
name: recall
description: >
  Memory retrieval protocol. Auto-invoke at session start to review injected context,
  before implementing features or fixes, when encountering repeated errors, when the
  user references past work, or when modifying unfamiliar code areas. Use /kindling:recall
  <query> for targeted deep searches.
---
```

## Context Injection (Passive)

The SessionStart hook already injects prior context (pins + recent observations) into the conversation. This is passive — it happens before the skill is invoked.

The skill teaches agents to **read and act on injected context** rather than ignoring it. At session start, the agent should scan the injected context for relevant pins and recent observations. If the injected context relates to the current task, acknowledge it briefly. No `/memory search` is needed at session start unless the injected context is insufficient for the task at hand.

## Trigger Protocol

### Automatic Triggers

The agent actively searches memory (via `/memory search`) when:

1. **Before implementing** — Agent is about to write code for a feature or fix. Check if prior sessions touched this area.
2. **Repeated errors** — Agent hits an error that feels familiar or has failed the same way twice.
3. **User references past work** — Phrases like "we did this before", "last time", "remember when", "what was that thing".
4. **Unfamiliar code areas** — Agent is about to edit files in a part of the codebase where prior sessions recorded observations (e.g., a module the agent hasn't touched yet this session). When uncertain, fold this into the "before implementing" trigger — search for the area name before editing.

### Explicit Invocation

User or agent types `/kindling:recall <query>` for a targeted deep search.

## Query Strategy

### Step 1: Formulate the Query

- Extract key terms from the current task context (focused terms, not the full user message)
- For "before implementing" triggers: use the feature or area name
- For error triggers: use the error message or pattern
- For past work references: use the user's own words

### Step 2: Run the Search

- `/memory search "<query>"` — returns pins first, then ranked candidates
- Scope is automatic (per-project DB, no flags needed)

### Step 3: Interpret the Tiers

- **Pins** — High-signal, intentionally marked. Always read.
- **Candidates** — Ranked by FTS relevance. Scan the top results, discard noise.
- If results are thin, try alternate terms or broaden the query (e.g., "auth middleware" → "authentication").

### Step 4: Act on Results

- **Relevant context found:** Briefly tell the user what was found and how it affects the approach.
- **Nothing relevant (auto-trigger):** Proceed silently. Don't announce "I checked memory and found nothing."
- **Nothing relevant (explicit `/kindling:recall`):** Tell the user no relevant results were found — they invoked it and expect a response.
- **Never dump raw search output** at the user.

**Key principle:** Transparent when it matters, silent when it doesn't.

## Pin Protocol

### When to Pin

- Root cause of a bug is identified
- An architectural decision is made with rationale
- A non-obvious workaround is found
- A gotcha or footgun is discovered in the codebase
- After an explicit `/kindling:recall` returns useful results — offer to pin if the user is likely to need them again

### When Not to Pin

- Routine code changes (hooks capture those automatically)
- Temporary debugging state
- Things already documented in code comments or READMEs

### How to Pin

- `/memory pin "why this matters"` — pins the most recent observation with a note
- `/memory pin "description" --ttl 7d` — TTL for time-bounded relevance
- **Notes should explain _why_ it matters**, not _what_ it is. The observation content already has the what.

### TTL Heuristic

- **No TTL (permanent):** Architectural decisions, recurring gotchas, root causes of systemic issues.
- **Use TTL when the finding is tied to a specific constraint that will expire:** a library version being upgraded, a temporary workaround, a deadline-driven decision, a known issue with a fix in progress.
- Common TTLs: `7d` (short-term workaround), `30d` (version-specific), `24h` (debugging context for tomorrow).

## Examples

### Example 1: Before Implementing

```
Agent receives task: "Fix the rate limiter in the API server"
→ Trigger: about to implement in an area
→ Runs: /memory search "rate limiter API server"
→ Finds: pinned observation from 3 sessions ago — "rate limiter uses sliding window,
   not fixed window — don't change the algorithm, the bug is in the TTL calculation"
→ Tells user: "Found prior context on the rate limiter — the issue was previously
   identified as a TTL calculation bug, not the algorithm. I'll focus there."
→ Proceeds with that context
```

### Example 2: Discovery → Pin

```
Agent debugging a test failure, discovers the real issue:
→ The SQLite WAL checkpoint was blocking under concurrent writes
→ This isn't obvious from the code and will bite future sessions
→ Runs: /memory pin "WAL checkpoint blocks under concurrent writes —
   use PRAGMA wal_checkpoint(PASSIVE) not FULL" --ttl 30d
→ Tells user: "Pinned that WAL finding for future sessions."
```

### Example 3: Thin Results → Retry

```
Agent looking for context on "webhook validation":
→ Runs: /memory search "webhook validation"
→ Gets 0 candidates
→ Retries: /memory search "webhook"
→ Finds 3 candidates about webhook delivery and retry logic
→ Scans results, finds one relevant observation about payload signing
→ Tells user: "Found a note from a prior session about webhook payload signing
   that may be relevant here."
```

## Decisions

| Decision                    | Choice                  | Rationale                                                          |
| --------------------------- | ----------------------- | ------------------------------------------------------------------ |
| Auto vs explicit invocation | Hybrid                  | Auto on triggers, explicit for deep dives                          |
| Read vs read/write          | Retrieve + Pin          | Hooks handle capture; pinning is intentional                       |
| Query scoping               | Always repo-scoped      | Per-project DB handles this automatically                          |
| Result communication        | Transparent but concise | Mention memory when it influences approach, silent otherwise       |
| Skill approach              | Protocol + examples     | Teaches agents how to think, grounded by concrete scenarios        |
| Session start               | Passive context         | Hook injects context; skill teaches agent to use it, not re-search |

## File Structure

```
plugins/kindling-claude-code/
  skills/
    recall/
      SKILL.md          ← The skill definition
```

Single file. No supporting scripts needed — the skill orchestrates existing `/memory` commands.
