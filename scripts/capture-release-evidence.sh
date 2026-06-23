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
  port018-check.txt
  port018-pipeline.txt
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
  pnpm install --frozen-lockfile 2>&1 >"$SCRATCH/pnpm-install-full.txt"
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

step4_port018() {
  node packages/kindling/scripts/build-platform-packages.mjs --check \
    >"$SCRATCH/port018-check.txt" 2>&1
  grep -E 'platform|optionalDependencies|cross-build' \
    .github/workflows/publish.yml .github/workflows/release.yml \
    | cat >"$SCRATCH/port018-pipeline.txt"
}

step5_preflight_rust() {
  local exit_code=0
  {
    set -x
    cargo fmt --all -- --check
    cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -20
    cargo test --workspace 2>&1 | tail -10
  } >"$SCRATCH/preflight-rust.txt" 2>&1 || exit_code=$?
  echo "[exit=$exit_code]" >>"$SCRATCH/preflight-rust.txt"
  return "$exit_code"
}

step5_preflight_npm() {
  local tmp exit_code=0
  tmp="$(mktemp)"
  {
    set -x
    pnpm install --frozen-lockfile
    pnpm run build
    pnpm run type-check
    pnpm run lint
    pnpm run test
  } >"$tmp" 2>&1 || exit_code=$?
  echo "[exit=$exit_code]" >>"$tmp"
  # Phase traces from the same run (set -x) plus plan-mandated tail -20 of output.
  {
    grep '^+ pnpm' "$tmp" || true
    echo '--- tail -20 ---'
    tail -20 "$tmp"
  } >"$SCRATCH/preflight-npm.txt"
  rm -f "$tmp"
  return "$exit_code"
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
  if ! grep -Fx 'OK: no committed optionalDependencies (7 targets are injected at publish).' \
    "$SCRATCH/port018-check.txt" >/dev/null; then
    echo "CHECK FAIL: port018-check.txt missing OK line" >&2
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
  step4_port018

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
  echo "--- port018-check.txt ---"
  cat "$SCRATCH/port018-check.txt"
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