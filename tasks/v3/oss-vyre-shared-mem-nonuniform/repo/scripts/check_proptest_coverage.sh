#!/usr/bin/env bash
# Audit-H  -  proptest coverage ratchet.
#
# Counts the number of `*.rs` files under workspace src/ + tests/ that
# import `proptest` and ratchets monotonically upward. Adding a new
# property test increases the count; deleting one fails the gate. The
# floor begins at 42 (the count at gate-authoring time, 2026-04-28)
# and rises to the SQLite/NASA/Linux bar of 200+ over future patches.
#
# Doctrine: proptest is the cheapest way to expose IR / wire-format /
# optimizer invariants at scale. Property tests are first-class
# regression coverage and must not silently shrink.
#
# Usage:
#   scripts/check_proptest_coverage.sh           # enforce
#   scripts/check_proptest_coverage.sh --report  # print current count

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Ratcheted floor  -  bump upward as new property tests land.
FLOOR=123
# Stretch target tracked for the 0.7 release.
TARGET=200

mode="${1:-enforce}"

count=$(grep -rlE 'proptest!|use proptest|proptest::|extern crate proptest' --include='*.rs' . 2>/dev/null \
    | grep -vE '/target/|/target-' \
    | wc -l | tr -d ' ')

if [[ "$mode" == "--report" ]]; then
    echo "proptest-coverage: $count files import proptest (floor=$FLOOR, target=$TARGET)"
    exit 0
fi

if (( count < FLOOR )); then
    echo "proptest-coverage gate: $count files (floor=$FLOOR)." >&2
    echo "Fix: a property test was deleted. Either restore it or" >&2
    echo "lower the FLOOR in scripts/check_proptest_coverage.sh with" >&2
    echo "an explicit reviewer rationale." >&2
    exit 1
fi

if (( count > FLOOR )); then
    # New property tests landed  -  bump the floor to lock the gain.
    echo "proptest-coverage: $count files (floor=$FLOOR, target=$TARGET)" >&2
    echo "Fix: bump FLOOR in scripts/check_proptest_coverage.sh to" >&2
    echo "$count to lock the new property-test coverage." >&2
    exit 1
fi

echo "proptest-coverage gate: $count files at the floor (target=$TARGET)."
exit 0
