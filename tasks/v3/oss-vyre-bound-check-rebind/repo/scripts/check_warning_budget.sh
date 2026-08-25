#!/usr/bin/env bash
# Cap the workspace warning count at the baseline; regressions fail CI.
# Lower the baseline file whenever the count drops to ratchet the
# contract tighter.

set -euo pipefail
cd "$(dirname "$0")/.."

BUDGET_FILE=".internals/baselines/warning_budget.txt"
BUDGET="$(cat "$BUDGET_FILE" | tr -d '[:space:]')"
source scripts/lib/cargo_runner.sh
vyre_select_cargo_runner
CURRENT="$("$CARGO_RUNNER" build --workspace 2>&1 | grep -c '^warning:' || true)"

if [ "$CURRENT" -gt "$BUDGET" ]; then
    printf 'warnings regressed: %s > budget %s. Fix the new warnings or justify in the PR description.\n' "$CURRENT" "$BUDGET" >&2
    exit 1
fi
if [ "$CURRENT" -lt "$BUDGET" ]; then
    printf 'warnings progress: %s < budget %s. Lower %s to ratchet.\n' "$CURRENT" "$BUDGET" "$BUDGET_FILE"
fi
printf 'warnings: %s / budget %s\n' "$CURRENT" "$BUDGET"
