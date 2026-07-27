# Public Direction and Contribution Surface

| ID     | Owner  | Status |
| ------ | ------ | ------ |
| PUBDIR | @aneki | Draft  |

**Last reviewed:** 2026-07-19 — created ahead of expected external attention to
make kindling's direction, contribution boundaries, and available work legible
without exposing the internal APS tree as a public task dump.

## Purpose

Give prospective users and contributors a stable account of what kindling is
trying to become, which changes fit that direction, and which bounded
opportunities are safe to pick up. APS remains the planning source of truth;
GitHub Issues becomes the curated public discussion and pickup surface.

## North Star

> kindling makes useful development context survive the boundaries where AI
> tools forget, locally, explainably, and under the developer's control.

The public direction should make these principles concrete:

- Capture once; recall across tools and sessions.
- Local-first means no account, cloud dependency, or hidden egress.
- Retrieval is deterministic and explainable before it becomes clever.
- Trust matters more than capture volume: users can inspect, redact, forget,
  export, and diagnose retained memory.
- Rust is the canonical engine; clients and adapters stay thin.
- kindling provides memory mechanisms, not workflow governance or product
  policy.
- Small inspection experiences support the memory loop; a hosted application
  or general-purpose dashboard is not the product.

## In Scope

- A public vision and durable product-direction statement.
- Contributor acceptance lanes and maintainer checkpoints.
- A curated public roadmap derived from, but smaller than, the APS plan tree.
- GitHub issue forms, labels, issue hygiene, and contributor-ready task
  contracts.
- Explicit APS metadata for work that may be projected publicly.
- A reviewed, one-way APS-to-GitHub-Issues projection once upstream APS support
  is suitable.

## Out of Scope

- Publishing every APS module or work item as a GitHub issue.
- Making GitHub Issues or Projects a second planning source of truth.
- Bidirectional automation that edits APS markdown without review.
- Open-governance, response-time, or roadmap-delivery commitments that a
  single-maintainer project cannot sustain.
- Changing kindling's core product architecture within this module.
- Community growth, marketing campaigns, or hosted collaboration services.

## Contribution Lanes

The public contribution contract will distinguish:

1. **Welcome:** confirmed bugs, tests, documentation truth, portability,
   benchmarks, diagnostics, and adapter-contract compliance.
2. **Discuss first:** schema or API changes, observation kinds, retrieval
   ranking, retention and redaction behaviour, new adapters, and substantial
   inspection UX.
3. **Not the direction:** hosted or multi-tenant memory, governance workflows,
   opaque model-only ranking, a parallel non-Rust engine, and unrelated agent
   frameworks.

Security boundaries, persistent-data migrations, public contracts, and
cross-repository changes remain maintainer-led even when implementation help is
invited.

## Interfaces

**Depends on:**

- Current README, architecture, integration, retrieval, privacy, and lifecycle
  behaviour — public direction must describe shipped truth.
- `anvil-plan-spec` JSON export and any future public-projection metadata —
  PUBDIR-007 consumes APS rather than reparsing markdown privately. Earlier
  public-direction work does not wait on this capability.

**Coordinates with:**

- KINTEG and PORT — public opportunities must respect live compatibility,
  security, migration, and release constraints.
- [08-conversion-surface](./08-conversion-surface.aps.md) (CONV) — owns install
  and adoption conversion; PUBDIR owns direction and contribution legibility.
  C9's why-kindling material must not tell a competing story.
- Anvil KFIT — downstream governance integration remains distinct from
  standalone kindling's public contribution contract.

**Exposes:**

- A stable north-star and product-principles document.
- A three-lane contribution acceptance policy.
- A small public roadmap and labelled contributor-opportunity queue.
- An APS-to-Issue projection contract with explicit source identity and drift
  behaviour.

## Ready Checklist

- [ ] Operator accepts the proposed north star and contribution lanes.
- [ ] Initial maintainer-only and external-ready boundaries are agreed.
- [ ] Three to five suitable initial public opportunities are selected.
- [ ] Kindling's APS compatibility floor and upgrade path are verified.

## Work Items

### PUBDIR-001: Establish the public vision and direction contract

- **Status:** Draft
- **Intent:** Give users and contributors a durable basis for judging whether
  proposed work moves kindling towards its intended product.
- **Expected Outcome:** A concise public vision defines the north star, product
  principles, intended users, concrete success measures, non-goals, and the
  boundary between standalone kindling and downstream products such as anvil.
  README and public overview copy point to it without duplicating or weakening
  the contract.
- **Validation:** Public direction is consistent with `README.md`,
  `docs/use-cases.md`, `docs/architecture.md`, `docs/integrations.md`, and live
  supported journeys; `git diff --check` passes.
- **Files:** `VISION.md`, `README.md`, selected public overview documents.
- **Confidence:** high

### PUBDIR-002: Define contribution acceptance lanes and checkpoints

- **Status:** Draft
- **Intent:** Let contributors identify welcome work and avoid investing in
  changes that conflict with kindling's boundaries or require a prior decision.
- **Expected Outcome:** The contributor guide distinguishes welcome,
  discuss-first, and not-the-direction work; explains maintainer-led security,
  migration, API, and cross-repository changes; and gives proposal authors the
  evidence expected before implementation begins.
- **Validation:** `CONTRIBUTING.md` maps representative proposals unambiguously
  to one lane and remains consistent with the public vision; `git diff --check`
  passes.
- **Dependencies:** PUBDIR-001
- **Files:** `CONTRIBUTING.md`, `SECURITY.md` where reporting boundaries need
  clarification.
- **Confidence:** high

### PUBDIR-003: Publish a curated public roadmap

- **Status:** Draft
- **Intent:** Show external readers where kindling is heading without requiring
  them to interpret migration history, design gates, or internal APS mechanics.
- **Expected Outcome:** A short public roadmap presents current, next, and later
  product outcomes, links only to deliberately public opportunities, names
  non-goals, and identifies APS as the internal canonical plan rather than
  publishing the full plan tree as a promise.
- **Validation:** Every roadmap claim traces to current APS or shipped product
  evidence; no Draft or maintainer-only work is presented as available;
  `aps lint plans && git diff --check` passes.
- **Dependencies:** PUBDIR-001
- **Files:** `ROADMAP.md`, `README.md`.
- **Confidence:** high

### PUBDIR-004: Create the contributor issue and label contract

- **Status:** Draft
- **Intent:** Turn GitHub Issues into a clear public discussion and pickup
  surface rather than an unstructured mirror of internal plans.
- **Expected Outcome:** Issue forms cover confirmed bugs and proposals; labels
  distinguish contributor-ready, good-first, help-wanted, design-needed,
  blocked, security-sensitive, and relevant component areas; stale completed
  handoffs are closed; each advertised opportunity contains problem, outcome,
  scope, acceptance, validation, dependencies, and maintainer checkpoints.
- **Validation:** Repository issue forms validate, the live label set matches
  the documented contract, and `gh issue list --state open --json number,title,labels`
  shows no completed handoff presented as active work.
- **Dependencies:** PUBDIR-002
- **Files:** `.github/ISSUE_TEMPLATE/`, `CONTRIBUTING.md`.
- **Confidence:** high

### PUBDIR-005: Define APS public-projection metadata

- **Status:** Draft
- **Intent:** Make public publication an explicit maintainer decision encoded in
  APS rather than inferred from status or file location.
- **Expected Outcome:** A reviewed contract defines contribution audience,
  difficulty, skills, maintainer checkpoints, public issue identity, and
  publication eligibility. Only explicitly external `Ready` items qualify;
  Draft, Blocked, security-sensitive, cross-repository design-gate, and
  maintainer-only work is excluded by default. Required export additions are
  proposed upstream to `anvil-plan-spec` rather than implemented by a private
  kindling parser.
- **Validation:** Example eligible and ineligible items produce an unambiguous
  expected projection; `aps lint plans` passes; the upstream APS proposal links
  back to this consumer contract.
- **Dependencies:** PUBDIR-002, PUBDIR-003
- **Files:** This module, APS project context or an accepted design record,
  upstream `anvil-plan-spec` plan/issue.
- **Confidence:** medium

### PUBDIR-006: Publish the first curated contributor opportunities manually

- **Status:** Draft
- **Intent:** Validate the contributor-task contract with real issues before
  investing in synchronisation automation.
- **Expected Outcome:** Three to five bounded opportunities are published with
  the approved labels and complete pickup context. Security-sensitive and
  unresolved contract choices remain maintainer-led. Contributor feedback is
  captured as evidence for the projection design.
- **Validation:** Each selected issue satisfies the PUBDIR-004 contract and
  links to one canonical APS identity; none requires unpublished design work to
  begin.
- **Dependencies:** PUBDIR-003, PUBDIR-004, PUBDIR-005
- **Confidence:** high

### PUBDIR-007: Adopt a reviewed one-way APS-to-Issues projection

- **Status:** Draft
- **Intent:** Remove repetitive public-task bookkeeping without creating a
  second writable plan.
- **Expected Outcome:** A dry-run-first command or CI workflow creates and
  updates only explicitly publishable issues, preserves discussion outside a
  managed body section, records a stable APS identity marker, and reports drift.
  Issue assignment or comments never mutate APS automatically; APS status
  changes remain reviewed repository commits.
- **Validation:** Fixture and sandbox-repository tests prove idempotent create,
  update, no-op, exclusion, rename, completion, and human-edited-body behaviour;
  a dry run against kindling reports only the deliberately published set.
- **Dependencies:** PUBDIR-005, PUBDIR-006, upstream APS projection capability.
- **Confidence:** medium

## Risks

| Risk                                                   | Likelihood | Impact | Mitigation                                                                 |
| ------------------------------------------------------ | ---------- | ------ | -------------------------------------------------------------------------- |
| Public roadmap becomes a delivery promise              | Medium     | High   | Publish outcomes and horizons, not dates; label uncertainty                |
| Every internal item leaks into Issues                  | Medium     | High   | Explicit external metadata and default exclusion                           |
| GitHub and APS disagree                                | High       | Medium | APS remains canonical; dry-run and drift reporting; no silent reverse sync |
| Contributor-ready tasks still require hidden decisions | Medium     | High   | Require complete acceptance and maintainer checkpoints before publication  |
| Community expectations exceed maintainer capacity      | Medium     | High   | State review boundaries and avoid response-time commitments                |
| Vision prose overstates unfinished journeys            | Medium     | High   | Reconcile against executable integration truth before publication          |

## Decisions

1. **APS is canonical; Issues is a projection.** Public collaboration should
   not introduce a second planning authority.
2. **Publication is opt-in.** `Ready` alone does not make a work item suitable
   for external contribution.
3. **Prove the issue contract manually first.** Automation follows evidence
   from a small curated set.
4. **Vision is human-owned.** The north star is reviewed prose, not generated
   from implementation work items.

## Open Questions

1. Should the public roadmap be fully curated prose or partly generated from
   explicitly public APS metadata?
2. Which contribution metadata belongs in core APS versus a kindling-local
   extension?
3. Should completed APS work close a projected issue automatically or only
   propose a reviewed close action?
4. What minimum maintainer response expectations, if any, can be stated
   sustainably?
