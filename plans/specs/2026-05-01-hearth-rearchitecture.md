# Hearth Rearchitecture Plan

| Field      | Value                                                                              |
| ---------- | ---------------------------------------------------------------------------------- |
| Status     | Brainstorm / proposal — pre-ADR                                                    |
| Owner      | @joshuaboys                                                                        |
| Created    | 2026-05-01                                                                         |
| Branch     | `claude/rearchitect-hearth-daemon-RbDYw`                                           |
| Scope      | EddaCraft stack across `eddacraft/anvil-001` + `eddacraft/kindling`                |
| Mirror of  | `eddacraft/anvil-001` `plans/brainstorms/2026-05-01-hearth-rearchitecture.md`      |

**Locked-in decisions** (do not relitigate):

1. Introduce a fifth named primitive — **Hearth** — the always-on local Rust daemon that hosts the Anvil kernel, Kindling log, Ember proposal queue, and Edda memory store in one process, sharing one semantic substrate (witchcraft).
2. **Kindling positioning is hybrid**: Rust is canonical, the crate lives in the Anvil monorepo, but Kindling still ships independently as both a Rust crate and `@eddacraft/kindling` npm projection generated via `ts-rs`.
3. **Witchcraft powers Ember, Edda, and the Anvil kernel — not Kindling.** Ember does hybrid BM25 + semantic candidate detection; Edda does semantic-with-confidence retrieval over curated decisions; the Anvil kernel embeds symbols/functions for code-similarity. Kindling stays FTS-only — witchcraft never reads from or writes to kindling.db.

---

## 1. Motivating signal from prior decisions

The rearchitecture is mostly *acknowledging substrate that already exists* in the codebase rather than building from scratch:

- **ADR-015 (Intercept Loop Enforcement)** already specifies a Rust daemon with **JSON-RPC 2.0** IPC over **Unix sockets / Windows named pipes**, process-group control, and hook-based session registration. The daemon binary is `crates/anvil-intercept/`.
- **ADR-030 (Surface Drivers on Daemon)** already declares the daemon authoritative and surfaces (VS Code, MCP, shell, web) as *thin drivers* attaching over IPC.
- **ADR-033** archived the in-process VS Code extension and TS scanner, leaving `archive/anvil-vscode-extension/` and the TS antipattern scanner pending re-introduction via the daemon-driver path (`DRVR-003/-004`).
- **D-003** (kindling repo, `2026-04-15-rust-port-design.md`) already locks in dual-maintain Rust + TS with **Rust as the source of truth via ts-rs** — the 9-crate workspace (`kindling-types`, `-store`, `-provider`, `-service`, `-filter`, `-hook`, `-server`, `-cli`, umbrella) is *Ready* but not yet executed.
- **`anvil-kernel-types`** is already a zero-dep root crate suitable for ts-rs derivation.
- **`docs/vision/aspirational-ultimate-feature.md`** already names the four end-user capabilities — invariant streaming, structural drift modelling, plan-aware watching, behavioural diff — that this rearchitecture unlocks.

**Reading:** Hearth is a **rename + scope-expansion of `anvil-intercept`** plus an **import of the planned Kindling Rust crates** plus a **new `forge-retrieval` substrate crate** wrapping witchcraft. The novel work is (a) the retrieval crate, (b) Ember/Edda Rust modules, (c) symbol-embedding in the kernel, and (d) cleanup of the TS projection layer. The IPC, daemon lifecycle, session registry, surface-driver protocol, and Rust ports are already designed or in flight.

---

## 2. The Hearth daemon

### 2.1 Process model

- **Single binary**: `hearth` (replaces today's `anvil-intercept` binary entry; the `anvil intercept start` subcommand stays as a backwards-compatible launcher for one release).
- **One long-lived OS process per user-session.** Started lazily on first client request (CLI auto-spawns) or persistently via OS service unit (`launchd` / `systemd --user` / Windows service).
- **Single tokio runtime** orchestrating: file watcher, kernel parser+graph, Kindling write path, Ember candidate detector, Edda promotion arbiter, retrieval substrate, IPC listener, MCP server, HTTP server.
- **Per-workspace state directory**: `~/.hearth/<workspace-fingerprint>/` containing `kindling.db`, `ember.db`, `edda/` (git-backed YAML), `forge.db` (witchcraft index), `socket`, `pid`, `logs/`.
- **Single writer, many readers** for every SQLite DB (WAL mode). Readers can be inside Hearth (other modules) or, for read-only tooling, external processes. The daemon owns the writer lock.
- **Symbol-graph state** stays in-memory (current kernel design); witchcraft state is persisted but rebuilt on demand.

### 2.2 IPC surface (four lanes, one process)

All four lanes converge on the same `HearthService` trait inside the daemon — they are presentation skins, not separate code paths.

| Lane | Transport | Schema | Primary client | Notes |
|------|-----------|--------|---------------|-------|
| **Local socket** | Unix socket (`socket`) / Windows named pipe | JSON-RPC 2.0 (`hearth-proto` — see §3.1; renamed from today's `anvil-intercept-proto`) | CLI, TUI, drivers | Default, lowest-latency. Auth via filesystem ACL + per-session token. |
| **HTTP** | `axum` on `127.0.0.1:<random>` (port written to state dir) | JSON-RPC 2.0 over HTTP POST + SSE for streams | Web dashboard, agents in containers | Loopback-only by default. Bearer token in state dir. |
| **MCP** | stdio JSON-RPC | Model Context Protocol | Claude Code, Cursor, OpenAI agents | Existing `anvil mcp serve --stdio` (RMCP) becomes a thin shim that forwards to the local socket. |
| **stdio hooks** | stdin/stdout JSON, one observation per process | Kindling hook contract (D-003 immutable) | Claude Code hook, OpenCode adapter, PocketFlow | The `kindling-hook` binary ships independently and proxies into Hearth's local socket; <10 ms round-trip target preserved. |

**Service shape** (load-bearing methods, illustrative):

```rust
// Anvil enforcement
fn validate_buffer(req: ValidateBufferRequest) -> Result<Vec<Diagnostic>>;
fn watch_workspace(req: WatchRequest) -> Stream<EngineEvent>;

// Kindling (write-only / read-deterministic)
fn open_capsule(req: OpenCapsuleRequest) -> Capsule;
fn append_observation(req: AppendObservationRequest) -> Ack;
fn close_capsule(req: CloseCapsuleRequest) -> Ack;
fn retrieve(req: RetrieveRequest) -> RetrieveResult;        // FTS-only, deterministic

// Ember (proposes, never decides)
fn propose(req: ProposeRequest) -> ProposalId;              // hybrid retrieval over Ember corpus
fn list_candidates(req: ListCandidatesRequest) -> Vec<Proposal>;
fn dismiss(req: DismissRequest) -> Ack;

// Edda (curated, human-gated)
fn query_edda(req: EddaQueryRequest) -> EddaQueryResult;    // hybrid + confidence rerank
fn submit_for_review(req: SubmitForReviewRequest) -> ReviewId;
fn promote(req: PromoteRequest) -> EddaEntryId;             // requires human-approval token

// Cross-cutting
fn subscribe(req: SubscribeRequest) -> Stream<HearthEvent>; // unified event firehose
fn health() -> HealthReport;
```

The session-registry, process-group enforcement, and IPC plumbing already in `anvil-intercept` are reused unchanged.

### 2.3 Lifecycle

- **Startup**: <500 ms cold target. Steps: open SQLite DBs in WAL mode → mmap kernel symbol graph → bind socket → register OS signal handlers → start watcher → emit `Ready` event. Witchcraft index is opened lazily on first retrieval call so the daemon doesn't pay embed-model load cost on every restart.
- **Auto-spawn**: CLI commands check for the socket; if absent, fork-detach `hearth` and wait up to 1 s for `Ready`.
- **Service mode**: `hearth install --user` writes a `systemd --user` / `launchd` / Windows service unit that starts on login.
- **Reload**: `hearth reload` re-reads `.anvil/architecture.yaml`, policy bundles, workspace config without dropping the socket. Schema migrations require restart.
- **Shutdown**: SIGTERM → drain IPC queue (5 s) → close DBs → release socket. SIGKILL leaves a stale socket; startup detects via PID file and reclaims.
- **Crash containment**: per-file parse errors are isolated by the kernel (existing); a witchcraft panic is caught and degrades retrieval to FTS-only; Ember and Edda failures degrade gracefully (Hearth reports `Degraded` health but stays up for Anvil enforcement).

### 2.4 Existing CLIs / hooks / IDE extensions become clients

| Today | After |
|-------|-------|
| `anvil` CLI does its own scan inline (or via embedded `anvil-checks`) | Thin client; sends `validate_buffer` to Hearth socket. Falls back to embedded scanner if no daemon. |
| `anvil watch` runs a foreground watcher | Subcommand becomes alias for `hearth watch`; daemon owns the watcher, CLI subscribes to event stream. |
| `anvil mcp serve --stdio` is a launch shim with embedded fallback (RMCP) | Forwards every MCP request to Hearth socket; embedded fallback retained for offline use. |
| Claude Code hook (`kindling-hook` binary) writes directly to a SQLite file | Connects to Hearth socket → daemon writes Kindling. Hook stays a small, statically-linked binary so it works on machines without `hearth` if needed (back-channel direct DB write retained as fallback per D-003 contract). **Single-writer invariant preserved by socket-first probe:** the hook always checks for the Hearth socket and sends over IPC if present; only when the socket is absent (i.e. daemon not running, writer lock free) does it open the SQLite file directly. The hook never opens the DB while the daemon is alive. |
| `archive/anvil-vscode-extension/` (paused) | Re-introduced as `crates/anvil-vscode-driver/` LSP-style client; pure presentation, all logic in Hearth. (DRVR-003.) |
| TUI (`anvil-tui`) is in-process | Becomes a thin client using the existing `EngineEvent` stream; `anvil-tui` crate stays but its surfaces consume the daemon stream rather than embedding the kernel. |

**Compatibility:** every CLI subcommand gains a `--no-daemon` flag forcing the embedded fallback path. This preserves CI use cases and provides a recovery hatch.

---

## 3. Rust workspace shape

### 3.1 Target crate map (post-rearchitecture)

Crates marked **NEW** are introduced; **RENAMED** preserves git history via `git mv`; **ABSORBED** crates are merged into a parent and deleted.

#### Layer 0 — types (zero deps)

| Crate | Origin | Purpose | ts-rs export? |
|-------|--------|---------|---------------|
| `anvil-kernel-types` | existing | Diagnostics, EngineEvent, SymbolNode, EdgeType, TrustLevel | yes → `packages/anvil/contracts` |
| `kindling-types` | **NEW** (per D-003) | Observation, Capsule, Pin, Summary, ScopeIds | yes → `@eddacraft/kindling` npm |
| `ember-types` | **NEW** | Proposal, ProposalKind, Confidence, DecayPolicy | yes → `@eddacraft/edda-stack` |
| `edda-types` | **NEW** | EddaEntry, MemoryObjectKind (decision/pattern/constraint/warning/doctrine/lesson), ReviewEnvelope | yes → `@eddacraft/edda-stack` |
| `hearth-proto` | **RENAMED** from `anvil-intercept-proto` | JSON-RPC 2.0 envelopes, all IPC method schemas | yes (drivers may be TS) |

#### Layer 1 — substrates

| Crate | Origin | Purpose |
|-------|--------|---------|
| `forge-retrieval` | **NEW** | Sole consumer of witchcraft. Per-consumer query policies (Ember hybrid, Edda hybrid+rerank, kernel symbol-similarity). One DB file per index; multiple indexes supported. |
| `kindling-store` | **NEW** | SQLite + FTS5 store implementing the cross-language `schema/schema.sql` contract. Single writer, many readers, WAL. |
| `kindling-filter` | **NEW** | Content redaction, path filters, scope policies |
| `kindling-provider` | **NEW** | LocalFtsProvider (BM25 + recency); pluggable trait for future providers but **never** semantic. |
| `ember-store` | **NEW** | SQLite-backed proposal queue with TTL/decay |
| `edda-store` | **NEW** | Git-backed YAML repository, signed commits, lock file for atomic writes |
| `anvil-symbol-index` | **NEW** | Maintains per-workspace symbol embeddings via forge-retrieval; consumed by kernel for similarity queries (anti-duplication, pattern conformance). |

#### Layer 2 — services

| Crate | Origin | Purpose |
|-------|--------|---------|
| `anvil-kernel` | existing | Watcher, parser, graph, policy. **No new dependencies on retrieval — uses `anvil-symbol-index` only via a narrow query trait** so the enforcement path remains AI-free in spirit. |
| `anvil-checks` | existing | Antipattern, secret, command-safety. Authoritative scanner per ADR-026/029. |
| `anvil-architecture` | existing | Boundary rules, baseline, drift. |
| `anvil-policy` | existing | OPA/Rego evaluation. |
| `anvil-observability` | existing | W3C traceparent, redaction (TRACE-001/-003). |
| `kindling-service` | **NEW** | Orchestrates store + filter + provider behind D-003's `KindlingService` trait. |
| `ember-curator` | **NEW** | Reads observations from Kindling (via service trait), runs candidate-proposal detection using `forge-retrieval`'s hybrid mode. Writes proposals to `ember-store`. **One-way data flow enforced at type level.** |
| `edda-promotion` | **NEW** | Human-review state machine. Promotion requires explicit `--approved-by <human>` token plus a Kindling-recorded approval observation. Auto-promotion is structurally impossible. |
| `edda-query` | **NEW** | Hybrid + confidence-rerank query path; consumed by Anvil kernel for "find decisions relevant to this code" and by MCP for agent contexts. |

#### Layer 3 — daemon

| Crate | Origin | Purpose |
|-------|--------|---------|
| `hearth-session` | **RENAMED** from session-registry parts of `anvil-intercept` | Session lifecycle, auth tokens, ACL on socket file. |
| `hearth-rules` | **RENAMED** from `anvil-intercept-rules` | InterceptRule trait + concrete rules (now sourced from `anvil-checks` + Edda decisions). |
| `hearth-win32` | **RENAMED** from `anvil-intercept-win32` | Windows Job Objects, named-pipe ACLs. |
| `hearth-daemon` | **RENAMED + EXPANDED** from `anvil-intercept` library | Composes: kernel · checks · architecture · policy · kindling-service · ember-curator · edda-store · edda-promotion · edda-query · forge-retrieval · symbol-index · observability. Owns the IPC listener and the unified event bus. |
| `hearth` | **NEW binary** (replaces `anvil-intercept` binary) | Entry point. `hearth start`, `hearth status`, `hearth install`, `hearth reload`, `hearth shutdown`. |

#### Layer 4 — surfaces

| Crate | Origin | Purpose |
|-------|--------|---------|
| `anvil-cli` | existing, **trimmed** | All non-daemon subcommands become Hearth clients. `--no-daemon` flag forces embedded fallback. |
| `anvil-tui` | existing | Reuses kernel-types `EngineEvent`s subscribed via socket. |
| `kindling-cli` | **NEW** (per D-003 PORT-011) | Independent binary. Connects to Hearth if running, else opens the SQLite directly (Kindling's contract guarantees this works because Kindling is FTS-only and the schema is the cross-language contract). |
| `kindling-hook` | **NEW** (per D-003 PORT-007) | Independent binary. Same dual-mode: prefers Hearth socket, falls back to direct DB write. <10 ms target preserved. |
| `anvil-vscode-driver` | **NEW** (returns ADR-033 archived) | Pure LSP-style client. (DRVR-003.) |
| `anvil-bench` | existing | Criterion harness, gains Hearth round-trip benchmarks. |
| `workspace-hack` | existing | Hakari unifier. |

**Crates deleted outright:** `anvil-checks-napi` (no consumer post-archive), `anvil-intercept-proto` / `anvil-intercept` / `anvil-intercept-rules` / `anvil-intercept-win32` (renamed via `git mv`, contents preserved).

### 3.2 Dependency graph (text-rendered)

```
                                         [anvil-kernel-types]
                                                  ↑
                ┌───────────────────────┬─────────┼─────────┬───────────────────────┐
                │                       │         │         │                       │
        [kindling-types]          [ember-types]   │  [edda-types]              [hearth-proto]
                ↑                       ↑         │         ↑                       ↑
                │                       │         │         │                       │
        [kindling-store]          [ember-store]   │  [edda-store]               (consumed by all
        [kindling-filter]              ↑          │         ↑                    drivers + CLIs)
        [kindling-provider]            │          │         │
                ↑                      │          │         │
                │                      │          │         │
        [kindling-service] ───────→ [ember-curator]  ───→  [edda-promotion]
                                       ↑                    ↑
                                       │                    │
                                  [forge-retrieval]  ←──[edda-query]
                                       ↑
                                       │
                                [anvil-symbol-index]
                                       ↑
                                       │
                              [anvil-kernel]   [anvil-checks]   [anvil-architecture]   [anvil-policy]
                                       ↑              ↑                ↑                    ↑
                                       └──────────────┴────────┬───────┴────────────────────┘
                                                               │
                                                       [hearth-daemon]
                                                          ↑   ↑   ↑
                                  [hearth-session] ───────┘   │   └─── [hearth-rules]
                                  [hearth-win32]──────────────┘
                                                               │
                                          ┌────────────────────┼────────────────────┐
                                          ↓                    ↓                    ↓
                                   [hearth bin]          [anvil-cli]          [kindling-cli]
                                                              ↑                    ↑
                                                       [anvil-tui]           [kindling-hook]
                                                       [anvil-vscode-driver]
```

**Invariants of the graph:**

- `kindling-*` crates have **no path** to `forge-retrieval` (preserves "Kindling is FTS-only" — checked structurally by `cargo deny`/Hakari rules).
- `ember-curator` reads from `kindling-service` via a *narrow query trait* defined in `kindling-types`; it **does not** depend on `kindling-store` directly.
- `edda-promotion` cannot bypass `edda-store`'s human-approval gate (the "approve" method is gated by `ApprovalToken` constructed only by reading a confirmed observation from Kindling).
- `anvil-kernel` imports `anvil-symbol-index` behind a `feature = "symbol-embeddings"` flag that is on by default but switchable off in CI / no-daemon mode.

---

## 4. The `forge-retrieval` crate

A single Rust crate wrapping witchcraft, exposing per-consumer policies. Why one crate: witchcraft has compile-time embedder selection (T5-quantized vs. OpenVINO), a single SQLite-per-index storage model, and a single-writer concurrency assumption. Centralising the wrapper makes those constraints solvable in one place.

### 4.1 Public API shape

```rust
pub struct ForgeRetrieval { /* witchcraft DB handles + caches */ }

pub enum IndexKind {
    Ember,        // hybrid mode, balanced alpha
    Edda,         // hybrid + confidence rerank, accuracy-weighted
    SymbolGraph,  // semantic-only over symbol identifiers + docstrings
}

impl ForgeRetrieval {
    /// Writer handle. Sealed crate-internally so only `hearth-daemon` can construct it
    /// (see §4.5 for the rationale — preserves witchcraft's single-writer constraint).
    pub(crate) fn open_writer(state_dir: &Path) -> Result<Self>;

    /// Reader handle. Open to external tools, dry-run paths, and tests.
    pub fn open_reader(state_dir: &Path) -> Result<Self>;

    pub fn upsert(&self, kind: IndexKind, doc: Document) -> Result<()>;       // writer only
    pub fn delete(&self, kind: IndexKind, id: DocumentId) -> Result<()>;      // writer only
    pub fn query(&self, kind: IndexKind, q: Query, policy: QueryPolicy) -> Result<Hits>;
    pub fn rebuild(&self, kind: IndexKind) -> Result<()>;                     // writer only
}

pub struct QueryPolicy {
    pub mode: RetrievalMode,            // Hybrid | SemanticOnly | BM25Only
    pub alpha: f32,                     // BM25 vs. semantic weight, 0..=1
    pub max_results: usize,
    pub rerank: Option<RerankPolicy>,   // Edda uses ConfidenceRerank
    pub explain: bool,                  // emit provenance trace
}
```

### 4.2 Per-consumer policies

| Consumer | IndexKind | Mode | Alpha | Rerank | Notes |
|----------|-----------|------|-------|--------|-------|
| **Ember candidate detector** | `Ember` | Hybrid | 0.5 | none | Recall-weighted; we *want* loose proposals because Edda gates promotion. |
| **Edda agent-context query** | `Edda` | Hybrid | 0.4 (semantic-leaning) | `ConfidenceRerank` | Precision-weighted. Confidence rerank uses Edda's curator-assigned weight + age decay. |
| **Anvil kernel — anti-duplication** | `SymbolGraph` | SemanticOnly | n/a | none | Embeddings of symbol name + signature + extracted comment. Threshold-gated for the enforcement decision. |
| **Anvil kernel — pattern conformance** | `SymbolGraph` | SemanticOnly | n/a | none | Embeds signatures of "approved" patterns from Edda; new symbols are checked for cosine similarity. |
| **MCP `query_edda` tool** | `Edda` | Hybrid | 0.4 | `ConfidenceRerank` | Same as agent-context. Provenance trace always on (transparency for AI Act Art. 13). |

### 4.3 Storage layout

- `~/.hearth/<workspace-fingerprint>/forge.db` — single SQLite file with **three logical indexes** (Ember, Edda, SymbolGraph) keyed by `IndexKind` in document metadata. Witchcraft's schema permits this via the `metadata` JSON column. (`<workspace-fingerprint>` is the same per-workspace state-directory key used in §2.1.)
- Backed up by `hearth backup` (forge.db is rebuildable from Kindling + Edda + the symbol graph, but rebuild is slow so we ship a backup tool).
- **Witchcraft schema is opinionated** (UUID, ts, metadata JSON, hash, body; 128-d 4-bit-quantized residual embeddings). We accept that constraint — our payload shape is small enough to fit. The crate hides the witchcraft schema from upstream consumers behind `Document` and `Hits`.

### 4.4 Embedder selection

Compile-time feature flag, passed through to witchcraft:

- `forge-retrieval/t5-quantized` (default, all platforms)
- `forge-retrieval/t5-openvino` (Linux x86_64 perf path)
- `forge-retrieval/metal` (macOS ARM64, GPU)
- `forge-retrieval/cuda` (Linux x86_64 with NVIDIA)

Cargo features cascade into Hearth's release matrix; user-facing distributions pick the right binary at install time. Document the constraint loudly: "you cannot switch embedder backend at runtime."

### 4.5 Concurrency

Witchcraft is single-writer. We enforce that **only `hearth-daemon` writes** by giving `forge-retrieval` two constructors:

- `open_writer(path)` — used only by daemon code (sealed crate-internally to enforce).
- `open_reader(path)` — used by external tools, dry-run paths, and tests.

The daemon's tokio runtime serialises writes through a single mpsc channel into the witchcraft writer, so even concurrent ingest from kernel/Ember/Edda is safely linearised.

### 4.6 Honest constraints

From the witchcraft survey (cite: GitHub repo, README, Cargo.toml as of 2026-04-28):

- T5-only encoder, fixed 128-d. Cannot bring custom embeddings.
- Hybrid alpha tuning is internal to witchcraft's CLI; library exposes `reciprocal_rank_fusion` but not a clean alpha knob. **Action:** open an issue with the witchcraft project asking for a programmatic alpha; in the meantime, fork-patch in our forge-retrieval crate.
- ~21 ms p95 on M2 Max for hybrid query at NFCorpus scale. On modest workspaces (<100k symbols, <100k Ember proposals, <10k Edda entries) we should comfortably stay well under that.
- Apache 2.0 license — compatible with our distribution model.
- Maturity: 446 stars, last commit 2026-04-28, active. Vendor-pinned via Cargo lockfile and a periodic `cargo outdated` check; if the project stalls, our `forge-retrieval` shim makes vendoring or replacement tractable.

---

## 5. Migration map (TS → Rust + projection)

### 5.1 `eddacraft/anvil-001`

| Path | Action | Rationale |
|------|--------|-----------|
| `packages/anvil/contracts/` | **CONVERT** to ts-rs projection generated from `anvil-kernel-types` | ADR-014 + ts-rs unblocks single source of truth. |
| `packages/anvil/ports/` | **DELETE** | Rust traits replace these interfaces; no TS consumer post-archive. |
| `packages/anvil/core/` | **SPLIT**: archive scanner/suppression-parser per ADR-029 (already authored Rust); delete drift/explain TS duplicates; **keep** any pure utility that has no Rust equivalent. | Reduces to ~15% of current LOC. |
| `packages/anvil/runtime/` | **DELETE** | Replaced by `hearth-daemon`. |
| `packages/anvil/policy/` | **KEEP** as TS orchestration layer | Hybrid Rego path remains. Reassess when ADR-014 thresholds tripped. |
| `packages/anvil/checks-napi/` | **DELETE** | Zero consumers post-ADR-033. |
| `packages/edda-stack/` | **CONVERT** to ts-rs projection generated from `kindling-types` + `ember-types` + `edda-types`. Keep TS surface (npm) for adapters. | Three-layer philosophy stays; the *implementation* moves to Rust. |
| `packages/kindling-integration/` | **DELETE** | Direct Rust consumption via Hearth socket replaces this bridge. The 11-observation-kind contract migrates verbatim into `kindling-types`. |
| `packages/aps/` | **KEEP** | OSS surface per ADR-018; TS-canonical for now. |
| `archive/anvil-vscode-extension/` | **DELETE** (after `anvil-vscode-driver` reaches feature parity) | New driver replaces it. |
| `crates/anvil-checks-napi/` | **DELETE** | Same reason. |
| `crates/anvil-intercept*` | **RENAME** via `git mv` to `crates/hearth-*` | History preserved. |

### 5.2 `eddacraft/kindling`

This repo becomes a **release surface** for the Kindling crate(s) that physically live in `eddacraft/anvil-001`. Two viable mechanics, recommendation in **bold**:

- **(Recommended) Subtree split + monorepo home**: `eddacraft/kindling` mirrors the `crates/kindling-*` and `packages/kindling-*` subtrees from anvil-001 via `git subtree split` on each release, plus its own README/CHANGELOG/LICENSE. CI in anvil-001 publishes; CI in kindling repo verifies the split. This keeps Kindling adoptable cross-project (`cargo install kindling-cli`, `npm install @eddacraft/kindling`) without requiring consumers to clone the whole stack.
- *Alternative*: keep both repos with kindling repo as a true secondary clone, syncing via a release script. Higher drift risk; not recommended.

Either way, the kindling repo's `plans/05-rust-port` module retires (its work is now done in anvil-001). The `D-003` decision text moves verbatim into anvil-001's `plans/decisions/` so the rationale is co-located with the code.

### 5.3 What stays TS

- **Adapter packages** (`kindling-adapter-claude-code`, `-opencode`, `-pocketflow`): TS for adapter ergonomics; consume Hearth via `kindling-hook` binary or socket. Rewriting to Rust offers no benefit.
- **Browser store** (`kindling-store-sqljs`): TS-only by definition (sql.js is browser-bound).
- **APS planning spec** (`packages/aps`): OSS, TS-canonical, slow-evolving.
- **Anvil website** (under `apps/website/`): unchanged.

### 5.4 What we delete (consolidated)

- `packages/anvil/ports/`, `packages/anvil/runtime/`, `packages/anvil/checks-napi/` — TS replaced by Rust.
- `packages/kindling-integration/` — bridge no longer needed.
- TS scanner + suppression parser + scanner-parity harness in `packages/anvil/core/` — already archived per ADR-033, now removed entirely.
- `crates/anvil-checks-napi/` — no consumer.
- `archive/anvil-vscode-extension/` — once `anvil-vscode-driver` ships.
- `archive/anvil-mcp-*` (if present) — RMCP shim is the canonical MCP entry.
- The standalone `eddacraft/kindling` server/CLI implementations once subtree-split is wired (their code lives in anvil-001).

Estimated deletion: ~40–60% of `packages/`. Most of `archive/` rotates from "cold storage" to "removed entirely" since the new daemon-driver path makes those implementations obsolete rather than dormant.

---

## 6. New end-user capabilities (against the August 2026 EU AI Act deadline)

The four capabilities below are the minimum that the substrate makes viable and that strengthen "agentic engineering governance" positioning. Each maps to specific EU AI Act articles for high-risk systems (Annex III obligations).

### 6.1 Plan-aware watching (PAW)

**What it does:** The daemon correlates each save against (a) the active APS plan, (b) Edda's structural-law constraints relevant to the touched files, and (c) Ember's open proposals. Real-time in-editor warnings surface "this change steps outside plan scope" and "this change touches an Edda-marked invariant."

**Why now:** Substrate readiness — needs Edda hybrid retrieval (forge-retrieval/Edda mode), kernel symbol graph, daemon event bus. All three converge in Hearth.

**EU AI Act mapping:**
- **Art. 9 (risk management)** — PAW is a continuous risk-control measure for code generated by AI assistants.
- **Art. 12 (record-keeping)** — every PAW signal is a Kindling observation, immutable and auditable.
- **Art. 14 (human oversight)** — PAW surfaces decisions to humans rather than blocking; honours warnings-over-blocks (ADR-002).

### 6.2 Behavioural diff review (BDR)

**What it does:** On each commit (or save), the kernel computes a *semantic* diff between previous and new symbol embeddings. Output: "Function `processOrder` shifted toward async I/O (cosine 0.72 → 0.41)" or "Public surface expanded by 3 symbols semantically resembling persistence operations." Surfaces in CLI, TUI, and the VS Code driver.

**Why now:** Witchcraft makes per-symbol embedding cheap enough to maintain incrementally. Without it, this required a vector DB and an API key.

**EU AI Act mapping:**
- **Art. 13 (transparency)** — BDR makes intent shifts in AI-generated code legible to reviewers.
- **Art. 15 (accuracy/robustness)** — provides a behavioural-correctness signal that text diffs miss.

### 6.3 Anti-duplication & pattern conformance (ADP)

**What it does:** Before a new function is accepted (at save time), the kernel queries `forge-retrieval/SymbolGraph` for semantically similar existing symbols. If similarity > threshold, raises an Anvil warning citing the existing symbol. Conversely, if a symbol matches a pattern Edda has marked "preferred," the warning becomes positive: "matches approved pattern `repository-with-cache`."

**Why now:** Embedding the symbol graph is the single highest-leverage application of witchcraft for the Anvil kernel — addresses the "AI agents reinvent the wheel" complaint head-on. Listed as a capability in `aspirational-ultimate-feature.md` but never previously feasible without a vector DB.

**EU AI Act mapping:**
- **Art. 11 (technical documentation)** — ADP creates a structured trail of "this code matches/diverges from pattern X."
- **Art. 26 (record-keeping for high-risk systems)** — every ADP decision is logged.

### 6.4 Institutional-memory retrieval in agent contexts (IMR)

**What it does:** The Hearth MCP server exposes `query_edda` (hybrid + confidence rerank). Every agent (Claude Code, Cursor, your custom agents) can ask "what decisions/patterns/constraints are relevant to this code?" and get back ranked Edda entries with confidence scores and provenance.

**Why now:** This is the unblocker for the "agents don't know our decisions" problem. Without semantic retrieval over Edda, we can only do tag/keyword matching and agents miss everything not lexically named.

**EU AI Act mapping:**
- **Art. 14 (human oversight)** — humans curate Edda; agents read from it; the boundary is clear.
- **Art. 15 (accuracy)** — agents stop hallucinating "best practices" when they have a curated, retrievable source.

**Cross-project pattern library (deferred):** The user's brief listed this as a candidate. I'd defer to v2 — it requires multi-workspace federation of Edda/forge.db, which is non-trivial and adds operational surface (sync, conflict resolution). The four capabilities above are Hearth-v1; cross-project federation is Hearth-v2 once single-workspace stability is proven.

---

## 7. Verification

### 7.1 Benchmarks to beat (criterion harness in `anvil-bench`)

| Operation | Target p95 | Source of target |
|-----------|------------|------------------|
| Hearth cold start | < 500 ms | Lifecycle requirement (CLI auto-spawn). |
| IPC round-trip (`validate_buffer` empty payload) | < 5 ms | Existing intercept p99 measurements. |
| Kindling write (single observation) | < 10 ms | D-003 success criterion. |
| Kindling FTS retrieve (default budget) | < 100 ms | `retrieval-contract.md`. |
| Witchcraft hybrid query (Ember corpus, 100k docs) | < 30 ms | Witchcraft README claim 21 ms + 9 ms IPC budget. |
| Edda query w/ confidence rerank | < 100 ms | Hybrid + rerank budget. |
| Symbol-graph cold build (100k LOC TS) | < 3 s | Existing kernel target. |
| Symbol embedding update (single file change) | < 50 ms | New requirement; sets ADP latency ceiling. |
| Memory ceiling (medium repo, 100k LOC) | < 700 MB | Kernel 500 MB + forge ~200 MB. |

CI gates fail the build if regression > 25 % vs. baseline (mirrors ADR-014 promotion thresholds).

### 7.2 Invariants to prove

Encoded as `cargo deny` rules + `cargo test` cases + `clippy` lints + structural CI checks:

1. **`forge-retrieval` has no path to `kindling-store`, `kindling-service`, or `kindling-provider`.** Enforced by `cargo-deny` dependency rule. *Justification:* Kindling stays FTS-only; witchcraft never touches it.
2. **`anvil-kernel` enforcement decisions never await on `forge-retrieval`'s witchcraft path beyond a configurable timeout, and similarity scores are deterministic given a stable index** (test: build index → query twice → assert byte-equal hit list). *Justification:* prevention-over-detection + deterministic-not-probabilistic.
3. **Kindling observations are immutable** — only `redacted` flag mutates; SQL trigger rejects updates to other columns. *Justification:* D-003 + write-emit contract.
4. **Edda promotion requires a human-attested approval token**, constructed only by reading a Kindling observation of kind `human_approval` whose payload is signed by a registered approver. Property test: forge a Proposal → assert `promote()` rejects without an approval. *Justification:* "If you can't explain why it's in Edda, it doesn't belong."
5. **Truth flow is one-way: Kindling → Ember → Edda.** No `kindling-*` crate has a path to `ember-*` or `edda-*`; `ember-*` cannot import `edda-*`. *Justification:* core principle.
6. **Provenance is mandatory.** Every `EngineEvent` carries a `provenance` field; serde-test ensures encoding/decoding never drops it. *Justification:* `anvil-vision.md` quote.
7. **Schema version compatibility** — Hearth refuses to start if `kindling.db` schema version is outside its compatible range. *Justification:* D-003 cross-language contract; see `schema/version.json`.

### 7.3 Smoke test (Hearth round-trip end-to-end)

Single integration test (`tests/round_trip.rs` in `hearth-daemon`) that proves the whole substrate works:

```
1. Start a fresh Hearth in a temp workspace.
2. Connect a mock VS Code driver client over the socket; subscribe to events.
3. Emit an observation via `kindling-hook`:
     POST {kind: "tool_call", content: "edited foo.ts", scopeIds:{...}}
   Assert: Kindling row exists; FTS picks it up; subscriber sees ObservationEvent.
4. Trigger Ember candidate detection (it should run automatically on observation
   threshold; here we force it via a debug RPC).
   Assert: Ember proposal exists in ember.db with hybrid rationale; confidence
   score; subscriber sees ProposalCreated.
5. Submit the proposal for review, then promote with an approval token.
   Assert: edda/decisions/<id>.yaml on disk; git commit signed; subscriber sees
   EddaEntryPromoted.
6. Modify foo.ts in a way that touches the new Edda entry's symbol pattern.
   Assert: Anvil policy decision references the Edda entry by id and emits a
   warning whose `provenance` includes both the symbol-graph hit and the Edda
   query trace; mock VS Code driver receives a Diagnostic on the open buffer.
7. Shutdown Hearth gracefully; restart; query Edda; assert state survives.
```

This test is the single canonical proof that the substrate is healthy. CI runs it on every push to `main` / release branches; failure blocks merges.

### 7.4 Go/no-go gates for the rearchitecture itself

Before declaring Hearth v1 ready:

- **G1**: All bench targets in §7.1 hit on developer workstation + CI Linux runner.
- **G2**: All invariants in §7.2 enforced and proven by tests.
- **G3**: Smoke test §7.3 passes on macOS, Linux, Windows.
- **G4**: ts-rs projection round-trip — generate TS types, compile a sample TS adapter against them, assert no breaking changes vs. last published `@eddacraft/kindling`.
- **G5**: 30-day dogfood window — Anvil team runs Hearth as primary on their own work; bug rate trending < 1 sev-1 per week before public alpha.

---

## 8. Sequencing (suggested release horizons)

This is **a sketch**, not a contract — use the existing APS planning surface to break into modules.

| Horizon | Headline | Crates new/touched |
|---------|----------|--------------------|
| **H2** (next release, slate already drafting) | Rename intercept → hearth; add forge-retrieval skeleton; ship Kindling Rust crates; preserve current behaviour. **No new user-facing features.** | hearth-* renames, kindling-types/store/provider/service/filter/hook, forge-retrieval (no consumers yet). |
| **H3** | Wire forge-retrieval into Ember + Edda Rust modules; ts-rs projections; delete `packages/kindling-integration` and `packages/anvil/runtime`. | ember-*, edda-*, ts-rs export pipeline. |
| **H4** (the substrate-unlock release) | Anvil kernel symbol embeddings + ADP + BDR + PAW + IMR ship. EU AI Act-aligned audit trail surfaces. | anvil-symbol-index, kernel ADP path, MCP `query_edda`, VS Code driver re-introduction (DRVR-003). |
| **H5** | Cross-project pattern library (Hearth-v2 federation); web dashboard. | (deferred from this rearchitecture) |

H2 + H3 are intentionally invisible to users — we are repaving the runway. H4 is the headline release that the EU AI Act window is positioned for.

---

## 9. Risks & open questions

1. **Witchcraft API gaps** — programmatic alpha tuning isn't exposed in the library. Mitigation: vendor a fork-patch in `forge-retrieval` and upstream a PR. Status: needs a concrete reproduction issue filed.
2. **T5 model loading** — first witchcraft query incurs model-load cost (~hundreds of ms). Mitigation: lazy-load on first retrieval call; pre-warm in `hearth start --pre-warm` for service-mode installations.
3. **Embedder backend selection at install time** — distribution matrix grows (per-OS, per-CPU/GPU). Mitigation: ship a single `t5-quantized` default that runs everywhere; offer optimised variants behind opt-in installer flags.
4. **Edda Git-backed YAML + signed commits** — requires a stable signing setup (GPG or sigstore). Open question: which signing primitive do we standardise on? Suggest sigstore (`cosign`) for GPG-free workflows. **(Non-blocking decision; can default to GPG if signing key already exists.)**
5. **Kindling subtree-split mechanics** — `git subtree split` is fragile across long-lived branches. Mitigation: a single canonical `tools/release-kindling.sh` script in anvil-001 that generates the kindling-repo tree on each release; CI verifies determinism.
6. **`anvil-kernel` symbol embeddings vs. "deterministic over probabilistic"** — using cosine similarity for ADP introduces a probabilistic signal into the enforcement adjacent path. Mitigation: ADP emits warnings (not blocks) by default; the deterministic Rust kernel still owns enforcement decisions; the embedding query is *advisory input*, not authoritative output. Document this tension explicitly in a follow-up ADR.
7. **MCP-only agents that can't run Hearth** — for sandboxed or remote agents that only speak MCP, ensure Hearth's MCP surface is reachable over the existing `anvil mcp serve --stdio` shim and via HTTP-MCP for cloud agents. Loopback HTTP guard plus bearer-token auth keeps it safe.

---

## 10. What this plan deliberately does *not* decide

- The **exact Cargo workspace layout** (one workspace vs. multi-workspace) — recommend single workspace under `eddacraft/anvil-001/` to keep refactor velocity high; separate workspaces are a v2 concern.
- The **hearth IPC versioning policy** — recommend semver on `hearth-proto` with backwards-compatibility for one minor version; defer detail to a follow-up ADR.
- **Whether `hearth` should run as a system service vs. user service by default** — recommend `--user` for v1 (lower privilege blast radius) and revisit for fleet deployments.
- The **MCP tool naming** — needs separate review with the agent framework integrators; not load-bearing for the rearchitecture itself.

These should each become small, focused ADRs after this plan is accepted in principle.

---

## Appendix A — Reference crate inventory pre-rearchitecture (snapshot)

For context, the current `crates/` directory in `eddacraft/anvil-001` holds: `anvil-kernel`, `anvil-kernel-types`, `anvil-checks`, `anvil-checks-napi`, `anvil-architecture`, `anvil-policy`, `anvil-observability`, `anvil-intercept`, `anvil-intercept-proto`, `anvil-intercept-rules`, `anvil-intercept-win32`, `anvil-tui`, `anvil-cli`, `anvil-bench`, `workspace-hack`. The `eddacraft/kindling` repo holds its 9 planned `crates/` (per D-003) but has not yet begun executing PORT-001..017.

## Appendix B — Source citations driving this plan

- **`docs/vision/anvil-vision.md`** — invariants, "constitutional engineering" thesis.
- **`docs/vision/aspirational-ultimate-feature.md`** — PAW, BDR, ADP, drift modelling.
- **`docs/architecture/rust-kernel-spec.md`** — kernel modules, performance targets, no-AI-in-enforcement.
- **`docs/architecture/overview.md`** — Edda Stack three-layer model, governing rules, truth flow.
- **`plans/decisions/`** — ADR-011a (Rust core), ADR-014 (language allocation), ADR-015 (intercept daemon), ADR-018 (product/IP), ADR-026/029 (scanner/suppression authority), ADR-030/033 (surface drivers, archive).
- **`plans/brainstorms/missing-features-analysis.md`** — auto-fix, MCP, web dashboard gaps.
- **`eddacraft/kindling` `plans/specs/2026-04-15-rust-port-design.md`** — D-003 dual-maintain Rust + TS.
- **`eddacraft/kindling` `docs/architecture.md`, `docs/data-model.md`, `docs/retrieval-contract.md`** — write-emit contract, FTS-only retrieval.
- **<https://github.com/dropbox/witchcraft>** (read 2026-05-01) — library API, schema, embedder options, perf claims, license.
