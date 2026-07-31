#!/usr/bin/env bash
# Mutation-surrogate: detect tests with no assertion primitives.
# Tests that pass by doing nothing give false confidence.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

BASELINE_FILE="scripts/baselines/unfailing_tests.txt"
mkdir -p "$(dirname "$BASELINE_FILE")"

# Crude heuristic: count #[test] fns whose body has no assert/panic/expect/unwrap/?.
count=0
while IFS= read -r test_file; do
    [[ -z "$test_file" ]] && continue
    fn_count=$(grep -c "#\[test\]" "$test_file" 2>/dev/null || true)
    assertion_count=$(grep -cE "assert|panic!|expect\(|unwrap\(|\?;|\?[[:space:]]*$|todo!|unimplemented!" "$test_file" 2>/dev/null || true)
    fn_count=${fn_count:-0}
    assertion_count=${assertion_count:-0}
    if [[ "$fn_count" -gt 0 && "$assertion_count" -lt "$fn_count" ]]; then
        suspect=$((fn_count - assertion_count))
        count=$((count + suspect))
    fi
done < <(find vyre-core/tests vyre-reference/tests vyre-driver-wgpu/tests vyre-runtime/tests vyre-foundation/tests -name '*.rs' 2>/dev/null)

baseline=$(cat "$BASELINE_FILE" 2>/dev/null || echo 0)

if [[ "${1:-}" == "--refresh" ]]; then
    echo "$count" > "$BASELINE_FILE"
    echo "refreshed baseline: $count"
    exit 0
fi

echo "Unfailing-test surrogate: $count (baseline $baseline)."

if [[ "$count" -gt "$baseline" ]]; then
    echo "FAIL: count exceeded baseline. Fix: add assertions to the new tests." >&2
    exit 1
fi

if [[ "$count" -lt "$baseline" ]]; then
    echo "$count" > "$BASELINE_FILE"
fi
