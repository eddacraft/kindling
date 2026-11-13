# Daemon / Persistent Service Mode Implementation Plan

> **Historical.** This plan targets the retired Node.js/TypeScript engine (pre-D-005).
> It is kept for reference and does not describe the current Rust engine. See
> [docs/architecture.md](../architecture.md) for the current design.

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Eliminate Node.js cold-start overhead (~100-200ms per hook invocation) by running a persistent background service that keeps the SQLite database connection open and handles requests via a Unix domain socket.

**Architecture:** A lightweight HTTP server (using Node's built-in `http` module) listens on a Unix socket. Hook scripts become thin clients that POST JSON to the socket instead of opening their own DB connections. The daemon starts on first use and auto-exits after an idle timeout.

**Tech Stack:** Node.js, Unix domain sockets, HTTP

---

### Task 1: Create the daemon server module

**Files:**

- Create: `plugins/kindling-claude-code/daemon/server.js`
- Create: `plugins/kindling-claude-code/daemon/client.js`

**Step 1: Write the daemon server**

The server should:

- Listen on a Unix socket at `$XDG_RUNTIME_DIR/kindling-<hash>.sock` (or `/tmp/kindling-<hash>.sock`)
- Accept JSON POST requests with `{ action, payload }` shape
- Keep the DB connection open across requests
- Auto-exit after 5 minutes of idle (no requests)
- Write its PID + socket path to a lockfile for client discovery
- Handle `POST /invoke` with body `{ hook, context }` — dispatches to the same logic currently in each hook script
- Handle `GET /health` for liveness checks

```javascript
const http = require('http');
const { init, cleanup } = require('../hooks/lib/init.js');

const IDLE_TIMEOUT_MS = 5 * 60 * 1000;
let idleTimer;
let db, store, service;

function resetIdleTimer() {
  clearTimeout(idleTimer);
  idleTimer = setTimeout(() => {
    cleanup(db);
    try {
      fs.unlinkSync(socketPath);
    } catch {}
    process.exit(0);
  }, IDLE_TIMEOUT_MS);
}

const server = http.createServer((req, res) => {
  resetIdleTimer();
  if (req.method === 'GET' && req.url === '/health') {
    res.writeHead(200);
    res.end('ok');
    return;
  }
  // ... handle POST /invoke
});

server.listen(socketPath);
```

**Step 2: Write the client helper**

A thin module that hooks import instead of `init.js`. It:

1. Checks if daemon is running (lockfile + health check)
2. If yes: POST the hook payload via Unix socket, return response
3. If no: spawn daemon in background, wait for health, then POST

```javascript
const http = require('http');
const { spawn } = require('child_process');

function invoke(hook, context) {
  // Try daemon first, fallback to direct execution
}
```

**Step 3: Manual test**

```bash
# Start daemon manually
node plugins/kindling-claude-code/daemon/server.js &

# Health check
curl --unix-socket /tmp/kindling-*.sock http://localhost/health

# Kill it
kill $(cat /tmp/kindling-*.lock)
```

**Step 4: Commit**

```bash
git add plugins/kindling-claude-code/daemon/
git commit -m "feat(daemon): add persistent service with Unix socket"
```

---

### Task 2: Refactor hooks to use daemon client

**Files:**

- Modify: `plugins/kindling-claude-code/hooks/session-start.js`
- Modify: `plugins/kindling-claude-code/hooks/pre-compact.js`
- Modify: `plugins/kindling-claude-code/hooks/notification.js`

**Step 1: Add daemon client import to each hook**

Each hook's `main()` function gets wrapped:

```javascript
const { tryDaemon } = require('../daemon/client.js');

async function main() {
  const context = await readStdin();

  // Try daemon path first (fast)
  const daemonResult = await tryDaemon('session-start', context);
  if (daemonResult) {
    process.stdout.write(JSON.stringify(daemonResult));
    return;
  }

  // Fallback: direct execution (cold start)
  // ... existing code ...
}
```

**Step 2: Test that hooks still work without daemon**

Run: `echo '{}' | node plugins/kindling-claude-code/hooks/session-start.js`
Expected: Works as before (fallback path)

**Step 3: Test with daemon running**

```bash
node plugins/kindling-claude-code/daemon/server.js &
echo '{}' | node plugins/kindling-claude-code/hooks/session-start.js
# Should be noticeably faster
kill %1
```

**Step 4: Commit**

```bash
git add plugins/kindling-claude-code/hooks/
git commit -m "perf(hooks): use daemon client with cold-start fallback"
```

---

### Task 3: Add daemon lifecycle management

**Files:**

- Create: `plugins/kindling-claude-code/scripts/daemon-start.js`
- Create: `plugins/kindling-claude-code/scripts/daemon-stop.js`
- Create: `plugins/kindling-claude-code/scripts/daemon-status.js`

**Step 1: Write management scripts**

- `daemon-start.js`: Starts daemon if not running, prints socket path
- `daemon-stop.js`: Sends shutdown signal, removes lockfile
- `daemon-status.js`: Reports running/stopped, PID, uptime, request count

**Step 2: Test lifecycle**

```bash
node plugins/kindling-claude-code/scripts/daemon-start.js
node plugins/kindling-claude-code/scripts/daemon-status.js
node plugins/kindling-claude-code/scripts/daemon-stop.js
```

**Step 3: Commit**

```bash
git add plugins/kindling-claude-code/scripts/daemon-*.js
git commit -m "feat(daemon): add start/stop/status management scripts"
```

---

### Task 4: Rebuild bundle and final test

**Step 1: Build and test**

```bash
pnpm run build
pnpm run test
node plugins/kindling-claude-code/scripts/build-bundle.js
```

**Step 2: Commit**

```bash
git add plugins/kindling-claude-code/dist/kindling-bundle.cjs
git commit -m "chore: rebuild plugin bundle with daemon support"
```
