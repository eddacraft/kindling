#!/usr/bin/env bash
# APS Completion Checker
# Blocks session end when a work item or action plan went In Progress after
# the session started and that status is not committed.
#
# These are not flagged:
#   - items already In Progress in the session's baseline commit (e.g. a claim
#     made on the branch before this session)
#   - In Progress statuses that are committed. On a feature branch the claim
#     commit sets the item to In Progress on purpose and it stays there until
#     the work merges.
#
# The baseline is the HEAD commit recorded by init-session.sh in
# .claude/.aps-session-baseline. Without a valid baseline, any uncommitted
# In Progress change is flagged. The baseline is per checkout, and uncommitted
# edits cannot be attributed to a session, so in a checkout shared by several
# sessions another session's uncommitted edits are flagged too.
#
# Items are matched by ID (e.g. PORT-014), not by file, so moving an item to
# another module file does not make it look new.
#
# Usage: ./.aps/scripts/check-complete.sh [plans-dir]
#
# Exit codes:
#   0 — nothing this session left In Progress and uncommitted
#   2 — uncommitted In Progress items from this session (blocks Claude from stopping)

set -euo pipefail

PLANS_DIR="${1:-plans}"
PLANS_DIR="${PLANS_DIR%/}"
BASELINE_FILE=".claude/.aps-session-baseline"

# If no plans directory, nothing to check
if [ ! -d "$PLANS_DIR" ]; then
  exit 0
fi

# Not a git repo: there is no session or commit boundary to compare against.
if ! git rev-parse --is-inside-work-tree &>/dev/null; then
  exit 0
fi

BASELINE=""
if [ -f "$BASELINE_FILE" ]; then
  CANDIDATE=$(tr -d '[:space:]' < "$BASELINE_FILE")
  if [ -n "$CANDIDATE" ] && git rev-parse --verify --quiet "${CANDIDATE}^{commit}" >/dev/null; then
    BASELINE="$CANDIDATE"
  fi
fi

HAS_HEAD=0
if git rev-parse --verify --quiet HEAD >/dev/null; then
  HAS_HEAD=1
fi

# A work-item spec file: plans/*.aps.md or plans/modules/*.aps.md, excluding
# index files and dotfiles (the set the original checker scanned).
is_spec_path() {
  local rel="${1#"$PLANS_DIR"/}"
  [ "$rel" != "$1" ] || return 1
  case "$rel" in modules/*) rel="${rel#modules/}" ;; esac
  case "$rel" in */*) return 1 ;; esac
  case "$rel" in
    .* | index*) return 1 ;;
    *.aps.md) return 0 ;;
    *) return 1 ;;
  esac
}

# Read spec markdown on stdin; print the ID of every work item whose first
# Complete / In Progress status line says In Progress.
in_progress_ids() {
  awk '
    /^### [A-Z]+-[0-9]+:/ {
      id = $2; sub(/:$/, "", id); open = 1; next
    }
    open {
      l = tolower($0)
      if (l ~ /\*\*status:\*\* *in progress/ || l ~ /status: *in progress/) {
        print id; open = 0
      } else if (l ~ /\*\*status:\*\* *complete/ || l ~ /status: *complete/) {
        open = 0
      }
    }
  '
}

# In Progress IDs across every spec file in a commit.
ids_at_rev() {
  local rev="$1" path
  git ls-tree -r --name-only "$rev" -- "$PLANS_DIR" 2>/dev/null | while IFS= read -r path; do
    is_spec_path "$path" || continue
    git show "$rev:$path" 2>/dev/null | in_progress_ids
  done | sort -u
}

# In Progress IDs across every spec file in the working tree.
ids_in_worktree() {
  local f
  for f in "$PLANS_DIR"/modules/*.aps.md "$PLANS_DIR"/*.aps.md; do
    [ -f "$f" ] || continue
    is_spec_path "$f" || continue
    in_progress_ids < "$f"
  done | sort -u
}

NOW_IDS=$(ids_in_worktree)
HEAD_IDS=""
if [ "$HAS_HEAD" -eq 1 ]; then HEAD_IDS=$(ids_at_rev HEAD); fi
BASE_IDS=""
if [ -n "$BASELINE" ]; then BASE_IDS=$(ids_at_rev "$BASELINE"); fi

# Flag: In Progress now, not committed that way at HEAD, not pre-existing.
FLAGGED=$(comm -23 \
  <(printf '%s\n' "$NOW_IDS" | sed '/^$/d' | sort -u) \
  <(printf '%s\n%s\n' "$HEAD_IDS" "$BASE_IDS" | sed '/^$/d' | sort -u))

INCOMPLETE=0
REPORT=""

while IFS= read -r id; do
  [ -n "$id" ] || continue
  REPORT+="  - $id: set In Progress since the session started, not committed"$'\n'
  INCOMPLETE=$((INCOMPLETE + 1))
done <<< "$FLAGGED"

# Action plans: flag an In Progress plan with unchecked boxes only when the
# file has uncommitted changes and was not already In Progress at the
# baseline.
plan_in_progress() {
  grep -qiE '^\| *Status *\|.*(In Progress|In-Progress)'
}

if [ -d "$PLANS_DIR/execution" ]; then
  for f in "$PLANS_DIR/execution/"*.actions.md; do
    [ -f "$f" ] || continue
    plan_in_progress < "$f" || continue

    UNCHECKED=$(grep -c '^ *- \[ \]' "$f" 2>/dev/null || true)
    [ "${UNCHECKED:-0}" -gt 0 ] || continue

    # Committed and unchanged: an open plan on a branch is fine.
    if [ "$HAS_HEAD" -eq 1 ] && git ls-files --error-unmatch -- "$f" &>/dev/null \
      && git diff --quiet HEAD -- "$f"; then
      continue
    fi

    # Already In Progress when the session started: not this session's.
    if [ -n "$BASELINE" ] && git cat-file -e "$BASELINE:$f" 2>/dev/null \
      && git show "$BASELINE:$f" | plan_in_progress; then
      continue
    fi

    REPORT+="  - $(basename "$f"): $UNCHECKED unchecked item(s), In Progress since the session started, not committed"$'\n'
    INCOMPLETE=$((INCOMPLETE + 1))
  done
fi

if [ "$INCOMPLETE" -gt 0 ]; then
  # Exit 2 blocks Claude from stopping. stderr is fed back to Claude.
  {
    echo "Session incomplete. $INCOMPLETE item(s) went In Progress after this session started and are not committed:"
    printf '%s' "$REPORT"
    echo ""
    echo "Before ending this session, either:"
    echo "  1. Complete the items or explicitly mark them Blocked, or"
    echo "  2. Commit the In Progress status (e.g. the claim commit on a feature branch)"
    echo "Add any discovered work as Draft items."
    echo ""
    echo "The baseline is per checkout, so in a checkout shared with another session"
    echo "these edits may be theirs. Leave them alone and work in your own worktree."
  } >&2
  exit 2
fi

exit 0
