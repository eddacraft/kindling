---
name: recall
description: >
  Memory retrieval protocol. Auto-invoke at session start to review injected context,
  before implementing features or fixes, when encountering repeated errors, when the
  user references past work, or when modifying unfamiliar code areas. Use /kindling:recall
  <query> for targeted deep searches.
---

# Memory Retrieval Protocol

You have access to kindling — a local memory engine that captures what happens across sessions (tool calls, diffs, commands, errors). This protocol teaches you when and how to retrieve that memory.

## Injected Context

At session start, kindling automatically injects prior context (pins and recent observations) into the conversation. **Read it.** If anything relates to the current task, acknowledge it briefly and let it inform your approach.

You do not need to run `/memory search` at session start unless the injected context is insufficient for the task at hand.

## When to Search

Run `/memory search` proactively when any of these triggers apply:

1. **Before implementing** — You are about to write code for a feature or fix. Search for prior work in that area before starting.
2. **Repeated errors** — You hit an error that feels familiar or you have failed the same way twice. Search for prior solutions.
3. **User references past work** — The user says things like "we did this before", "last time", "remember when", "what was that thing". Search using their words.
4. **Unfamiliar code area** — You are about to edit files in a part of the codebase you haven't touched yet this session. Search for the module or area name. When uncertain whether this applies, fold it into trigger 1 — search for the area name before editing.

## How to Search

### Formulate the query

Use focused key terms, not full sentences:

- Before implementing: the feature or area name (e.g., `"rate limiter"`, `"auth middleware"`)
- Error triggers: the error message or pattern (e.g., `"SQLITE_BUSY"`, `"JWT expired"`)
- Past work references: the user's own words (e.g., `"webhook retry logic"`)

### Run the search

```
/memory search "<query>"
```

Scope is automatic — each project has its own database.

### Interpret results

Results come in tiers:

- **Pins** — High-signal. Someone intentionally marked these. Always read them.
- **Candidates** — Ranked by FTS relevance. Scan the top results, discard noise.

If results are thin, try alternate terms (e.g., `"auth middleware"` → `"authentication"`) or drop qualifiers to broaden:

```
/memory search "webhook validation"   → 0 results
/memory search "webhook"              → 3 candidates about delivery and signing
```

### Act on results

- **Relevant context found:** Briefly tell the user what you found and how it affects your approach. Example: "Found prior context on the rate limiter — a previous session identified the bug as a TTL calculation issue, not the algorithm. I'll focus there."
- **Nothing relevant (auto-trigger):** Proceed silently. Do not announce that you checked memory and found nothing.
- **Nothing relevant (explicit `/kindling:recall`):** Tell the user no relevant results were found — they invoked it and expect a response.
- **Never dump raw search output** at the user. Summarize what matters.

**Principle:** Transparent when it matters, silent when it doesn't.

## When to Pin

When you discover something important that will help future sessions, pin it:

**Pin these:**

- Root cause of a bug
- Architectural decisions with rationale
- Non-obvious workarounds
- Gotchas or footguns in the codebase

**Don't pin these:**

- Routine code changes (hooks capture those automatically)
- Temporary debugging state
- Things already documented in code comments or READMEs

After an explicit `/kindling:recall` returns useful results, offer to pin if the user is likely to need them again.

### How to pin

```
/memory pin "why this matters"
/memory pin "workaround: use PASSIVE checkpoint, not FULL" --ttl 30d
```

Notes should explain **why** it matters, not what it is. The observation content already has the what.

### TTL heuristic

- **No TTL (permanent):** Architectural decisions, recurring gotchas, root causes of systemic issues.
- **Use TTL** when the finding is tied to something that will expire: a library version being upgraded, a temporary workaround, a deadline-driven decision, a known issue with a fix in progress.
- Common TTLs: `24h` (debugging context for tomorrow), `7d` (short-term workaround), `30d` (version-specific finding).

## Other commands

- `/memory status` — Check database stats (observation count, sessions, pins)
- `/memory pins` — List all active pins
- `/memory unpin <id>` — Remove a pin that's no longer relevant

## Examples

### Before implementing

```
Task: "Fix the rate limiter in the API server"
→ Search: /memory search "rate limiter API server"
→ Found: pin from 3 sessions ago — "rate limiter uses sliding window, not fixed window —
  the bug is in the TTL calculation, not the algorithm"
→ Response: "Found prior context on the rate limiter — a previous session identified the
  bug as a TTL calculation issue. I'll focus there."
```

### Discovery → Pin

```
Debugging a test failure, you discover the real issue:
→ SQLite WAL checkpoint blocks under concurrent writes
→ This isn't obvious from the code and will affect future work
→ Pin: /memory pin "WAL checkpoint blocks under concurrent writes —
  use PRAGMA wal_checkpoint(PASSIVE) not FULL" --ttl 30d
→ Response: "Pinned that WAL finding for future sessions."
```

### Thin results → Retry

```
Looking for context on webhook validation:
→ Search: /memory search "webhook validation" → 0 results
→ Retry: /memory search "webhook" → 3 candidates
→ One candidate mentions payload signing from a prior session
→ Response: "Found a note from a prior session about webhook payload signing
  that may be relevant here."
```
