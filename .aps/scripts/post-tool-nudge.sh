#!/usr/bin/env bash
# APS PostToolUse Hook
# Nudges the agent to record completed or discovered work for the PR
# description. APS statuses are updated in a later bookkeeping pass, not on
# feature branches (see AGENTS.md).
# Outputs JSON with additionalContext so the nudge reaches Claude.
#
# Only fires if an APS plans/ directory exists.

set -euo pipefail

if [ -d plans ]; then
  cat <<'EOF'
{"hookSpecificOutput":{"hookEventName":"PostToolUse","additionalContext":"[APS] If you completed a work item or discovered new scope, note it for the PR description. Do not edit APS statuses on a feature branch; the bookkeeping pass updates them."}}
EOF
fi

exit 0
