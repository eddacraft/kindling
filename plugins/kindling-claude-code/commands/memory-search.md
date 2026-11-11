---
description: Search your session memory for past work. Use when the user wants to find something from a previous session.
---

Search through past Claude Code sessions to find relevant context.

Run this command to search:

```bash
node "${CLAUDE_PLUGIN_ROOT}/scripts/memory-search.js" "$ARGUMENTS"
```

Show the results to the user in a readable format. Each result includes the observation ID; pass that ID to `/memory forget` to redact it.
