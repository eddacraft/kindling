# @eddacraft/kindling-cli

> **⚠️ Deprecated** — this package is deprecated and will be removed at v1.0.0.
> kindling is now a Rust daemon. Use [`@eddacraft/kindling`](https://www.npmjs.com/package/@eddacraft/kindling)
> (the thin HTTP-over-UDS client) or the `kindling` binary instead.
> See <https://github.com/eddacraft/kindling>.

Command-line interface for kindling - inspect, search, and manage your local AI memory.

[![npm version](https://img.shields.io/npm/v/@eddacraft/kindling-cli.svg)](https://www.npmjs.com/package/@eddacraft/kindling-cli)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](../../LICENSE)

## Installation

```bash
# Install globally
npm install -g @eddacraft/kindling-cli

# Or run via npx
npx @eddacraft/kindling-cli status
```

## Commands

### Status

Show database status and statistics:

```bash
kindling status
kindling status --db ./custom-path.db
```

Output:

```
Database: /home/user/.kindling/memory.db
Size: 2.4 MB

Observations: 1,247
Capsules: 23 (5 open, 18 closed)
Pins: 8 (3 expired)
Summaries: 18
```

### Search

Search for context across observations and summaries:

```bash
# Basic search
kindling search "authentication error"

# Filter by session
kindling search "auth" --session session-123

# Filter by repository
kindling search "auth" --repo /home/user/my-project

# Limit results
kindling search "auth" --limit 20

# Include redacted observations
kindling search "auth" --include-redacted
```

### List

List entities in the database:

```bash
# List capsules
kindling list capsules
kindling list capsules --status open
kindling list capsules --type session

# List pins
kindling list pins
kindling list pins --active  # Only non-expired

# List observations
kindling list observations
kindling list observations --kind error
kindling list observations --capsule cap-123
```

### Pin

Pin important observations or summaries:

```bash
# Pin an observation
kindling pin observation obs_abc123 --note "Root cause identified"

# Pin with TTL (expires in 7 days)
kindling pin observation obs_abc123 --ttl 7d

# Pin a summary
kindling pin summary sum_xyz789 --note "Key architecture decision"
```

### Unpin

Remove a pin:

```bash
kindling unpin pin_xyz789
```

### Inspect

View details of an entity:

```bash
kindling inspect observation obs_abc123
kindling inspect capsule cap_xyz789
kindling inspect summary sum_123
```

### Export

Export data for backup or transfer:

```bash
# Export all data
kindling export ./backup.json

# Export scoped data
kindling export ./backup.json --repo /home/user/my-project
kindling export ./backup.json --session session-123
```

### Import

Import data from a backup:

```bash
kindling import ./backup.json
```

## Configuration

The CLI uses these default paths:

| Item     | Default Path              |
| -------- | ------------------------- |
| Database | `~/.kindling/memory.db`   |
| Config   | `~/.kindling/config.json` |

Override with environment variables:

```bash
export KINDLING_DB_PATH=/custom/path/memory.db
kindling status
```

Or use the `--db` flag:

```bash
kindling status --db ./my-memory.db
```

## Output Formats

```bash
# Default (human-readable)
kindling list capsules

# JSON output
kindling list capsules --json

# Quiet (IDs only)
kindling list capsules --quiet
```

## Related Packages

- [`@eddacraft/kindling-core`](../kindling-core) - Domain types
- [`@eddacraft/kindling-store-sqlite`](../kindling-store-sqlite) - SQLite persistence

## License

Apache-2.0
