#!/usr/bin/env bash
# Asserts that every job listed in `.github/CI_REQUIRED.md` is actually
# declared in the corresponding workflow YAML. Closes the gap where a
# job is removed from CI but stays in the required-jobs doc (advisory
# is worse than absent).
#
# Scope: the three new lego-quality gates added 2026-05-02. Extending to
# the full required list is straightforward  -  add each gate name below.

set -euo pipefail
cd "$(dirname "$0")/.."

WF=".github/workflows/architectural-invariants.yml"
DOC=".github/CI_REQUIRED.md"

if [ ! -f "$WF" ] || [ ! -f "$DOC" ]; then
    echo "missing $WF or $DOC" >&2
    exit 1
fi

REQUIRED_JOBS=(
    "vyre-lints-raw-ir"
    "vyre-lints-allowlist-drift"
    "op-matrix-coverage"
    "lego-audit"
)

FAIL=0
for job in "${REQUIRED_JOBS[@]}"; do
    if ! grep -qE "^[[:space:]]+${job}:[[:space:]]*$" "$WF"; then
        echo "FAIL: required job '$job' not declared in $WF" >&2
        FAIL=1
    fi
    if ! grep -q "\`$job\`" "$DOC"; then
        echo "FAIL: required job '$job' not listed in $DOC" >&2
        FAIL=1
    fi
done

if [ "$FAIL" -eq 0 ]; then
    echo "required CI jobs ✓ (${#REQUIRED_JOBS[@]} checked)"
fi
exit "$FAIL"
