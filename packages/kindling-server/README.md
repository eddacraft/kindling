# @eddacraft/kindling-server

> **⚠️ Deprecated** — this package is deprecated and will be removed at v1.0.0.
> Kindling is now a Rust daemon. Use [`@eddacraft/kindling`](https://www.npmjs.com/package/@eddacraft/kindling)
> (the thin HTTP-over-UDS client) or the `kindling` binary instead.
> See <https://github.com/eddacraft/kindling>.

HTTP API server for Kindling - enables multi-agent concurrency and web-based access.

## When to Use

**Use the API server when:**

- Running 5+ concurrent agents
- Agents are in different languages/environments
- Using web-based agents (Claude Code Web, Cursor Web)
- Need centralized write coordination

**Don't use when:**

- Single agent or 2-4 agents with occasional writes (direct SDK is simpler)
- Maximum performance is critical (API adds network overhead)

## Quick Start

### Start the Server

```bash
# Via CLI
kindling serve --port 8080 --db ~/.kindling/project.db

# Or programmatically
import { startServer } from '@eddacraft/kindling-server';
import { openDatabase, SqliteKindlingStore } from '@eddacraft/kindling-store-sqlite';
import { LocalFtsProvider } from '@eddacraft/kindling-provider-local';
import { KindlingService } from '@eddacraft/kindling-core';

const db = openDatabase({ path: './kindling.db' });
const store = new SqliteKindlingStore(db);
const provider = new LocalFtsProvider(db);
const service = new KindlingService({ store, provider });

await startServer({
  service,
  db,
  port: 8080,
  host: '127.0.0.1',
});
```

### Use from Agents (TypeScript)

```typescript
import { KindlingApiClient } from '@eddacraft/kindling-server/client';

const client = new KindlingApiClient('http://localhost:8080');

// Open capsule
const capsule = await client.openCapsule({
  type: 'session',
  intent: 'debug',
  scopeIds: { sessionId: 'agent-1', repoId: 'my-app' },
});

// Append observations
await client.appendObservation(
  {
    id: 'obs-1',
    kind: 'command',
    content: 'npm test',
    provenance: { command: 'npm test', exitCode: 1 },
    ts: Date.now(),
    scopeIds: { sessionId: 'agent-1' },
    redacted: false,
  },
  { capsuleId: capsule.id },
);

// Retrieve context
const results = await client.retrieve({
  query: 'test failure',
  scopeIds: { sessionId: 'agent-1' },
});

// Close capsule
await client.closeCapsule(capsule.id, {
  generateSummary: true,
  summaryContent: 'Fixed test failures in auth module',
  confidence: 0.9,
});
```

### Use from Any Language (HTTP)

```bash
# Retrieve context
curl -X POST http://localhost:8080/api/retrieve \
  -H "Content-Type: application/json" \
  -d '{"query": "authentication error", "scopeIds": {"repoId": "my-app"}}'

# Append observation
curl -X POST http://localhost:8080/api/observations \
  -H "Content-Type: application/json" \
  -d '{
    "observation": {
      "id": "obs-123",
      "kind": "error",
      "content": "JWT verification failed",
      "scopeIds": {"sessionId": "agent-2"},
      "ts": 1768052000000,
      "redacted": false,
      "provenance": {}
    },
    "capsuleId": "capsule-abc"
  }'

# Create pin
curl -X POST http://localhost:8080/api/pins \
  -H "Content-Type: application/json" \
  -d '{
    "targetType": "observation",
    "targetId": "obs-123",
    "note": "Root cause identified",
    "scopeIds": {"repoId": "my-app"}
  }'
```

## API Endpoints

### Health Check

```
GET /health
```

### Retrieve Context

```
POST /api/retrieve
Body: RetrieveOptions
```

### Capsules

```
POST /api/capsules                    # Open capsule
POST /api/capsules/:id/close          # Close capsule
GET  /api/capsules/:id               # Get capsule (not implemented yet)
```

### Observations

```
POST   /api/observations              # Append observation
DELETE /api/observations/:id          # Forget observation
```

### Pins

```
POST   /api/pins                      # Create pin
DELETE /api/pins/:id                  # Remove pin
```

### Export/Import

```
POST /api/export                      # Export bundle
POST /api/import                      # Import bundle
```

## Web Agents (Claude Code Web, Cursor Web)

Web-based agents can't access local filesystem directly. Two options:

### Option 1: Proxy via Extension (Recommended)

Create a browser extension that acts as a bridge:

```
Web Agent → Extension → localhost:8080 → Kindling DB
```

The extension:

- Runs in browser context
- Can make fetch() calls to localhost
- Forwards requests/responses between web agent and API server

### Option 2: MCP Integration (If Supported)

If the web agent supports Model Context Protocol:

```typescript
// MCP server wraps Kindling API
import { Server } from '@modelcontextprotocol/sdk';

const server = new Server(
  {
    name: 'kindling-mcp',
    version: '1.0.0',
  },
  {
    capabilities: {
      resources: {},
      tools: {},
    },
  },
);

// Register Kindling tools
server.tool('kindling_retrieve', async (args) => {
  const client = new KindlingApiClient('http://localhost:8080');
  return await client.retrieve(args);
});

// ... register other tools
```

## Architecture

```
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│   Agent 1    │  │   Agent 2    │  │   Agent N    │
│  (any lang)  │  │  (any lang)  │  │  (any lang)  │
└──────┬───────┘  └──────┬───────┘  └──────┬───────┘
       │                 │                 │
       └─────────────────┼─────────────────┘
                         │ HTTP
                    ┌────▼─────┐
                    │   API    │
                    │  Server  │
                    │  :8080   │
                    └────┬─────┘
                         │
                    ┌────▼─────┐
                    │  SQLite  │
                    │   (WAL)  │
                    └──────────┘
```

**Benefits:**

- Single DB connection (no lock contention)
- Language-agnostic (HTTP)
- Centralized coordination
- Still local-first

## Security

**Important:** The API server binds to `127.0.0.1` by default (localhost only).

- **Don't expose to network** unless you add authentication
- **Don't use in production** without TLS + auth
- **Use for local development only** (multi-agent workflows)

If you need remote access, use SSH tunneling:

```bash
# On remote machine
ssh -L 8080:localhost:8080 user@remote-host

# Now http://localhost:8080 forwards to remote server
```

## License

Apache 2.0
