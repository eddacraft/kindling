# Public Direction — Delivered Work

| ID     | Owner  | Status   |
| ------ | ------ | -------- |
| PUBDIR | @aneki | Complete |

**Reconciled:** 2026-11-13.

## Purpose

Completed work is indexed in [completed.aps.md](../../completed.aps.md).
Release receipts and the full inventory are in the
[reconciliation report](../../reconciliation/2026-11-13-aps-cleanup.md).
Original outcomes, validation commands, implementation notes and earlier status
receipts below are historical. Removed TypeScript packages and superseded crate
layouts are not current setup instructions. Completion records delivery at the
stated boundary; it does not close later defects or claim fresh native tests.

## Work Items

### PUBDIR-001: Establish the public vision and direction contract

- **Status:** Complete
- **Delivery evidence (2026-11-13):** PR #151 (70cf1f52) is an ancestor of v0.4.0; VISION.md, README links and CONTRIBUTING.md are live on main.
- **Historical status:** Merged — PR #151 (merged 2026-10-21) adds `VISION.md`, README
  pointers, and corrects the `docs/use-cases.md` adapter table. The "time to
  first recall" measure is kept as an outcome target although `install.sh`
  depends on the v0.4.0 Release (ISS-003).
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

- **Status:** Complete
- **Delivery evidence (2026-11-13):** PR #151 (70cf1f52) is an ancestor of v0.4.0; VISION.md, README links and CONTRIBUTING.md are live on main.
- **Historical status:** Merged — PR #151 replaces the `CONTRIBUTING.md` scope guardrails
  with the three lanes, maintainer-led areas, and proposal evidence. States a
  single maintainer with no guaranteed response time; security reports are the
  exception and keep `SECURITY.md`'s acknowledgement aims (3 business days,
  14-day updates; acknowledgement, not resolution).
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
