#!/usr/bin/env bash
# P-DELETE-8  -  no hardcoded threshold constants in production code.
#
# A "threshold constant" is an integer or float `const` whose name
# matches `*_THRESHOLD`, `*_LIMIT`, `*_MAX`, `*_MIN`, `*_CAP`,
# `*_BUDGET`, `*_FLOOR`, `*_CEILING`  -  values an operator might want
# to tune at runtime through Tier-A config (`scripts/check_tier_config.sh`).
# Structural constants (`*_WORDS`, `*_BYTES`, `*_OFFSET`) are excluded
# because they describe the wire format, not a runtime knob.
#
# The gate ratchets the count of threshold-shaped consts in production
# code under `vyre-foundation/src/optimizer`, `vyre-runtime/src/megakernel`,
# `vyre-driver-wgpu/src/runtime`. Adding a new one is a regression;
# moving an existing one out into Tier-A config decreases the floor.
#
# Usage:
#   scripts/check_no_hardcoded_thresholds.sh           # enforce
#   scripts/check_no_hardcoded_thresholds.sh --report  # list current threshold sites

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

THRESHOLD_RE='const [A-Z_]+_(THRESHOLD|LIMIT|MAX|MIN|CAP|BUDGET|FLOOR|CEILING|TIMEOUT|DEADLINE|RETRY|BACKOFF):.*= [0-9]'

SCAN_ROOTS=(
    vyre-foundation/src/optimizer
    vyre-runtime/src/megakernel
    vyre-driver-wgpu/src/runtime
    vyre-driver-wgpu/src/buffer
)
POLICY="docs/optimization/THRESHOLD_POLICY.toml"

mode="${1:-enforce}"

count=0
hits=$(grep -rnE "$THRESHOLD_RE" --include='*.rs' "${SCAN_ROOTS[@]}" 2>/dev/null \
    | grep -vE '/tests/|test_fixtures|_tests\.rs:' || true)
if [[ -n "$hits" ]]; then
    count=$(printf '%s\n' "$hits" | wc -l | tr -d ' ')
fi

if [[ "$mode" == "--report" ]]; then
    echo "no-hardcoded-thresholds: $count sites; policy=$POLICY"
    if (( count > 0 )); then
        printf '%s\n' "$hits"
    fi
    exit 0
fi

# Floor at policy-authoring; per-threshold unit, provenance, and Tier
# placement live in docs/optimization/THRESHOLD_POLICY.toml.
FLOOR=14

if (( count > FLOOR )); then
    echo "no-hardcoded-thresholds gate: $count threshold sites (floor=$FLOOR)." >&2
    printf '%s\n' "$hits" >&2
    echo >&2
    echo "Fix: route the new threshold through Tier-A config where it is" >&2
    echo "operator-tunable, or mark it structural in $POLICY with unit," >&2
    echo "provenance, override path, evidence link, and VX-475 ownership." >&2
    exit 1
fi

if (( count < FLOOR )); then
    echo "no-hardcoded-thresholds: $count below floor $FLOOR  -  tighten FLOOR to $count." >&2
    exit 1
fi

echo "no-hardcoded-thresholds gate: $count sites at floor."
exit 0
