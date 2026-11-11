#!/usr/bin/env bash
# Capture release-prep verification evidence to {SCRATCH}.
# Uses set -x transcripts so silent successes (cargo fmt) and each pnpm phase
# are visible in the mandated artifact files.
set -euo pipefail
set +x

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRATCH="${SCRATCH:-/tmp/grok-goal-bcd8e438c318/implementer}"
cd "$ROOT"
mkdir -p "$SCRATCH"

# Only filenames mandated by plans/.../goal/plan.md Verification plan.
MANDATED=(
  aps-format.txt
  pnpm-install-full.txt
  pnpm-clean.txt
  packages-list.txt
  post-merge-cli.txt
  preflight-rust.txt
  preflight-npm.txt
  version-align.txt
)

# Remove stray scratch artifacts from prior capture attempts.
shopt -s nullglob
for f in "$SCRATCH"/*; do
  base="$(basename "$f")"
  keep=0
  for m in "${MANDATED[@]}"; do
    if [[ "$base" == "$m" ]]; then
      keep=1
      break
    fi
  done
  if [[ "$keep" -eq 0 ]]; then
    rm -f "$f"
  fi
done
shopt -u nullglob

step1_aps_format() {
  grep -E 'Purpose|Work Items|Last reviewed|ID.*Status' \
    plans/modules/06-downstream-integration-surface.aps.md \
    plans/modules/08-conversion-surface.aps.md \
    >"$SCRATCH/aps-format.txt"
}

step2_package_cleanup() {
  rm -rf packages/kindling-{core,store-sqlite,store-sqljs,provider-local,server,cli,adapter-claude-code} 2>&1 | cat
  node -e "
const fs=require('fs'); const base=fs.readFileSync('tsconfig.base.json','utf8');
if (base.includes('kindling-core') || base.includes('kindling-server/src')) {
  console.error('stale paths remain'); process.exit(1);
}
console.log('tsconfig clean');
"
  # Redirect stdout first so stderr joins the same transcript (`2>&1 >file`
  # leaves stderr on the terminal and drops install diagnostics).
  pnpm install --frozen-lockfile >"$SCRATCH/pnpm-install-full.txt" 2>&1
  tail -3 "$SCRATCH/pnpm-install-full.txt" >"$SCRATCH/pnpm-clean.txt"
  grep -F 'Scope: all 7 workspace projects' "$SCRATCH/pnpm-install-full.txt" >/dev/null
  ls packages/ | tee "$SCRATCH/packages-list.txt" >/dev/null
}

step3_merge_state() {
  git log --oneline -1 main | cat
  git log --oneline -1 --grep='conversion surface' main | cat
  cargo test -p eddacraft-kindling --test cli -- demo browse 2>&1 | tail -5 \
    >"$SCRATCH/post-merge-cli.txt"
}

# Status of the most recent run_logged invocation.
last_status=0

# Run one command, append its set -x transcript and a status marker to $1.
# A group or function used as an operand of `||` has errexit disabled, so a
# later success would otherwise mask an earlier failure. Each check is a
# single command whose status is captured with `||` on that command alone.
run_logged() {
  local log="$1" label="$2"
  shift 2
  last_status=0
  {
    set -x
    "$@"
  } >>"$log" 2>&1 || last_status=$?
  set +x
  echo "[$label exit=$last_status]" >>"$log"
}

step5_preflight_rust() {
  local log="$SCRATCH/preflight-rust.txt" failed=0
  : >"$log"

  run_logged "$log" fmt cargo fmt --all -- --check
  [ "$last_status" -eq 0 ] || failed=1

  run_logged "$log" clippy cargo clippy --all-targets --all-features -- -D warnings
  [ "$last_status" -eq 0 ] || failed=1

  run_logged "$log" test cargo test --workspace
  [ "$last_status" -eq 0 ] || failed=1

  echo "[exit=$failed]" >>"$log"
  return "$failed"
}

step5_preflight_npm() {
  local tmp failed=0
  tmp="$(mktemp)"
  : >"$tmp"

  run_logged "$tmp" install pnpm install --frozen-lockfile
  [ "$last_status" -eq 0 ] || failed=1
  run_logged "$tmp" build pnpm run build
  [ "$last_status" -eq 0 ] || failed=1
  run_logged "$tmp" type_check pnpm run type-check
  [ "$last_status" -eq 0 ] || failed=1
  run_logged "$tmp" lint pnpm run lint
  [ "$last_status" -eq 0 ] || failed=1
  run_logged "$tmp" test pnpm run test
  [ "$last_status" -eq 0 ] || failed=1

  echo "[exit=$failed]" >>"$tmp"
  # Phase traces from the same run (set -x) plus the full transcript so a
  # failed install, build, or test keeps its diagnostics.
  {
    grep '^+ pnpm' "$tmp" || true
    echo '--- output ---'
    cat "$tmp"
  } >"$SCRATCH/preflight-npm.txt"
  rm -f "$tmp"
  return "$failed"
}

step5_version_and_schema() {
  head -30 Cargo.toml package.json CHANGELOG.md >"$SCRATCH/version-align.txt"
  scripts/sync-vendored-schema.sh 2>&1 | cat
}

mechanical_checks() {
  local failed=0

  if ! grep -q '+ cargo fmt --all -- --check' "$SCRATCH/preflight-rust.txt"; then
    echo "CHECK FAIL: preflight-rust.txt missing '+ cargo fmt'" >&2
    failed=1
  fi
  if ! grep -q '+ pnpm run build' "$SCRATCH/preflight-npm.txt"; then
    echo "CHECK FAIL: preflight-npm.txt missing '+ pnpm run build'" >&2
    failed=1
  fi
  if ! grep -q '+ pnpm run lint' "$SCRATCH/preflight-npm.txt"; then
    echo "CHECK FAIL: preflight-npm.txt missing '+ pnpm run lint'" >&2
    failed=1
  fi
  if ! grep -F 'Scope: all 7 workspace projects' "$SCRATCH/pnpm-install-full.txt" >/dev/null; then
    echo "CHECK FAIL: pnpm-install-full.txt missing scope line" >&2
    failed=1
  fi
  if [[ "$(wc -l <"$SCRATCH/pnpm-clean.txt")" -ne 3 ]]; then
    echo "CHECK FAIL: pnpm-clean.txt is not 3 lines (got $(wc -l <"$SCRATCH/pnpm-clean.txt"))" >&2
    failed=1
  fi
  if ! grep -q 'test result: ok' "$SCRATCH/post-merge-cli.txt"; then
    echo "CHECK FAIL: post-merge-cli.txt missing passing tests" >&2
    failed=1
  fi

  return "$failed"
}

main() {
  set +x
  echo "Capturing release evidence to $SCRATCH"
  step1_aps_format
  step2_package_cleanup
  step3_merge_state

  local rust_exit=0 npm_exit=0
  step5_preflight_rust || rust_exit=$?
  step5_preflight_npm || npm_exit=$?
  step5_version_and_schema

  echo "--- aps-format.txt ---"
  cat "$SCRATCH/aps-format.txt"
  echo "--- pnpm-clean.txt ---"
  cat "$SCRATCH/pnpm-clean.txt"
  echo "--- post-merge-cli.txt ---"
  cat "$SCRATCH/post-merge-cli.txt"
  echo "--- preflight-rust.txt (head) ---"
  head -8 "$SCRATCH/preflight-rust.txt"
  echo "--- preflight-npm.txt ---"
  cat "$SCRATCH/preflight-npm.txt"
  echo "--- version-align.txt (head) ---"
  head -10 "$SCRATCH/version-align.txt"

  if ! mechanical_checks; then
    echo "Mechanical evidence checks FAILED" >&2
    exit 1
  fi
  if [[ "$rust_exit" -ne 0 || "$npm_exit" -ne 0 ]]; then
    echo "Preflight commands failed (rust=$rust_exit npm=$npm_exit)" >&2
    exit 1
  fi
  echo "All mechanical evidence checks passed."
}

main "$@"