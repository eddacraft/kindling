# CONTEXT

The shared language of kindling. Use these terms, with these meanings, in code,
docs, issues, and PRs. When a term here disagrees with the code, the code wins.
Fix this file in the same PR.

For how the pieces fit together, see [docs/architecture.md](docs/architecture.md).
For field-level types, see [docs/data-model.md](docs/data-model.md).

## What kindling is

kindling is a **local memory engine** for AI-assisted development. It records
what happened during a session as **observations**, groups them into
**capsules**, and returns them through **retrieval** with deterministic,
explainable ranking. Everything lives in one SQLite database per project on the
user's machine.

kindling is **mechanism, not policy**. It captures and returns context. It does
not decide what is true, important, approved, or allowed. Promotion, curation,
governance, and token budgeting belong to **downstream** systems (anvil, Edda).
If a proposed feature makes kindling decide something on the user's behalf, it
probably belongs downstream.

## Core domain

**Observation**
: One captured event. It is the atomic unit of memory: append-only, immutable
except for **forget**. It has a `kind`, `content`, `provenance` (free-form JSON
about where it came from), a timestamp `ts`, and `scopeIds`.
_Code:_ `kindling_types::Observation`, table `observations`.

**Observation kind**
: The closed set of observation types. Capture kinds are `tool_call`, `command`,
`file_diff`, `error`, and `message`. Workflow kinds are `node_start`,
`node_output`, `node_error`, and `node_end`. Adding a kind changes the schema
contract (there is a `CHECK` constraint) and the ts-rs bindings.
_Code:_ `ObservationKind`, `ObservationKind::ALL`.

**Capsule**
: A bounded group of observations that share one intent: a coding session, or a
single PocketFlow node run. A capsule has a `type`, an `intent` (a short
human-readable purpose), a status, and scope. Observations attach to the open
capsule for their session.
_Code:_ `Capsule`, tables `capsules` and `capsule_observations`.

**Capsule type**
: `session` (one agent or editor session) or `pocketflow_node` (one workflow
node execution).

**Capsule status**
: `open` or `closed`. A capsule opens once and closes once. It never reopens.
Closing can attach a **summary**.

**Summary**
: A condensed description of a capsule. It has `content`, a `confidence` in
`[0, 1]`, and `evidenceRefs` (the observation ids it was derived from). kindling
stores summaries. It does not judge them.
_Code:_ `Summary`, tables `summaries` and `summaries_fts`.

**Pin**
: A user-controlled marker that makes an observation or a summary always appear
in retrieval, ahead of ranked results. Pins are **non-evictable**. A pin may
carry a `reason` and an optional `expiresAt`.
_Code:_ `Pin`, `PinTargetType` (`observation` | `summary`), table `pins`.

**Scope** (`ScopeIds`)
: Five optional identifiers attached to observations, capsules, and pins:
`sessionId`, `repoId`, `agentId`, `userId`, and `taskId`. The first four are
stored in their own columns and you can filter retrieval by them. `taskId` is
**provenance only**. It is stored, but retrieval cannot filter by it.

**Project**
: The unit of storage isolation. One project has one SQLite database at
`~/.kindling/projects/<hash>/kindling.db`. The hash comes from the **project
root**. A project is not a scope: scopes filter rows inside a project database,
and projects separate databases.

**Project root**
: The directory that identifies a project. Resolution order: `KINDLING_REPO_ROOT`
(only when the working directory is inside it), then `git rev-parse
--show-toplevel`, then the canonical working directory. Daemon requests carry it
in the `X-Kindling-Project` header.

## Retrieval

**Retrieval** (`retrieve`)
: A ranked, top-K query. It returns three **tiers** in a fixed order:

1. **Pins**: every active pin in scope, always included.
2. **Current summary**: the latest summary for the session's capsule, if one
   exists. Always included.
3. **Candidates** (also called **provider hits**): FTS matches, ranked and capped
   at `maxCandidates`.

Every result carries **provenance** that explains why it was returned.

**Provider**
: The component that finds and scores candidates. The only provider is
`local-fts`: FTS5 BM25 relevance normalised to `[0, 1]`, blended with recency
(`0.7 * relevance + 0.3 * recency`, where recency covers a 30-day window).
_Code:_ `kindling-provider`.

**Orchestrator**
: Combines pins, the current summary, and provider candidates into one
`RetrieveResult`. It lives in `kindling-provider` and the service calls it.

**Deterministic**
: The same database, query, scope, and `retrieve_at` time always produce the same
result in the same order. Ties break on stable keys. This is a contract. See
[docs/retrieval-contract.md](docs/retrieval-contract.md).

**List** (`listObservations`)
: An exhaustive, paginated read, as opposed to ranked retrieval. It returns every
observation that matches a kinds, scope, and `[since, until)` filter, in
`(ts ASC, id ASC)` order, using a keyset **cursor**. It does no aggregation:
counting and grouping are downstream policy.
_Route:_ `POST /v1/observations/list`.

**Token budget**
: Deprecated. kindling bounds results by `maxCandidates` only. Fitting results
into a model's context window is the consumer's job.

## Privacy

**Masking**
: Automatic secret redaction that runs on every append, at the **service
boundary**, before anything reaches storage. Masking happens in the service, not
in the client, so no client (Rust, TypeScript, or hand-written HTTP) can skip it.
_Code:_ `kindling-service/src/filter/`.

**Redaction evidence**
: Returned with each append. It gives the number of masked matches and the secret
classes that matched (for example `bearerToken`), never the matched text.

**Forget**
: Removes an observation's content after it was captured. The content becomes
`[redacted]`, the row leaves the FTS index, and `redacted = true`. The id and the
capsule membership stay, so provenance still resolves. Forget is not delete.

**Redacted**
: An observation that has been forgotten. Retrieval and list exclude redacted
observations unless you pass `includeRedacted`.

## Runtime and processes

**Daemon**
: The long-running `kindling serve` process (`kindling-server`). It speaks
HTTP/1 over a Unix domain socket (`~/.kindling/kindling.sock`, mode `0600`). On
Windows it uses loopback TCP with a bearer token. It keeps one service per
project and shuts down after 30 idle minutes by default. All data goes through
the daemon so that several tools can write at once safely.

**Embedded**
: Running `kindling-service` in-process, with no daemon and no IPC. Use it only
when a single process owns the database.

**Client**
: Anything that talks to the daemon. `kindling-client` is the Rust client and
`@eddacraft/kindling` is the TypeScript client. When the socket does not answer,
a client **auto-spawns** the daemon. It checks the daemon's schema version when
it connects.

**Runtime**
: `kindling-runtime`, a facade crate for Rust downstreams (anvil first). It
bundles the client, the spool, and an optional in-process daemon. Its
**attach-or-start** behaviour connects to an existing daemon on the socket, or
starts one on a tokio task if none is running.

**Spool**
: An opt-in, append-only NDJSON buffer used by `SpooledClient`. When the daemon
is unreachable, appends go to the spool, and they are sent to the daemon on the
next successful append or on `flush()`. The spool is never a second source of
truth: SQLite is always authoritative. Delivery is "exactly-once-ish on id",
because the id is assigned before the first attempt and the daemon **dedups** on
id.

**Dedup**
: An append whose id already exists does not fail. It returns the existing row
with `deduplicated: true`.

**Capability handshake**
: `GET /v1/health` and `kindling status --json` report the daemon version, the
schema version, and a **kind registry** (each observation kind with its required
fields). Clients use it to detect version skew.

## Integration surface

**Producer**
: Anything that appends observations: hooks, adapters, the CLI, or downstream
apps.

**Hook**
: A Claude Code lifecycle entry point that the binary implements as `kindling hook
<type>`. The types are `session-start`, `post-tool-use`, `post-tool-use-failure`,
`user-prompt-submit`, `subagent-stop`, `stop`, and `pre-compact`. Some hooks
capture observations. Others request **injection context**.

**Injection context**
: Markdown the daemon builds for the `session-start` and `pre-compact` hooks so
that Claude Code can re-read recent memory. The service puts the data together
and the server formats it into Markdown. The output must match the original Node
hooks byte for byte.

**Adapter**
: An integration that maps a host tool's events onto observations and capsules.
The OpenCode, PocketFlow, and VS Code adapters live under `packages/` and use the
thin TypeScript client. Claude Code has no adapter package: its integration is
built into the binary.

**Plugin**
: The Claude Code plugin in `plugins/kindling-claude-code/`. It wires the hooks
and provides slash commands such as `/memory`.

## Contracts

**Schema contract**
: `schema/schema.sql` and `schema/version.json`. The store implements this
canonical database layout, and other tools can rely on it. The current version is
**6**, and `PRAGMA user_version` must match it. The store crate keeps a vendored
copy, and CI fails if the copy drifts. See [schema/README.md](schema/README.md).

**Bindings**
: TypeScript types generated from `kindling-types` by `ts-rs` into
`crates/kindling-types/bindings/`. The thin client uses them. CI fails if they
drift.

**Export bundle**
: A JSON snapshot of a project, or of one scope in it, with deterministic
ordering and a `bundleVersion`. `export → import → export` round-trips. Import
handles conflicts with one of `skip`, `overwrite`, or `error`.

**v1 API**
: The daemon's HTTP routes under `/v1/`. They use camelCase JSON, and every data
route requires `X-Kindling-Project`. Changing it is a public API change. Discuss
it first (see CONTRIBUTING).

## Words to avoid

| Avoid                          | Say instead          | Why                                                                   |
| ------------------------------ | -------------------- | --------------------------------------------------------------------- |
| event, log entry, memory item  | observation          | One noun for the atomic unit.                                         |
| session (for the record)       | capsule              | A session is the activity. The capsule is the stored record of it.    |
| delete, purge (observations)   | forget               | Forget keeps the row and its provenance.                              |
| search (for the ranked API)    | retrieve             | `kindling search` is the CLI verb. The engine operation is retrieval. |
| query / enumerate              | list                 | When you mean the exhaustive, paginated read.                         |
| server (for the process)       | daemon               | `kindling-server` is the crate. The daemon is the running process.    |
| repo (for storage isolation)   | project              | `repoId` is a scope value. A project is a database.                   |
| truth, knowledge, facts        | observations, memory | kindling records what happened. It does not assert truth.             |
| npm package (for distribution) | binary, crate        | kindling no longer publishes to npm (D-010).                          |

## Downstream names you will see

- **anvil**: EddaCraft's governed planning tool. It is the main Rust consumer,
  through `kindling-runtime`, and the source of the `KINTEG-*` work items.
- **Edda**: the downstream system for organisational memory and governance. The
  architecture doc's non-goals point here.
- **APS**: Anvil Plan Spec, the planning format under `plans/`. `D-0xx` are
  decisions recorded in `plans/index.aps.md`.
