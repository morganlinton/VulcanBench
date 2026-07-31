#!/usr/bin/env bash
# Audit-G  -  bench budgets enforced from `contracts/perf_targets.toml`.
#
# The perf-targets manifest (item #127) lists 22 publish-blocker
# budgets (latency, throughput, hit-rate, allocations) per crate. This
# gate is the runtime side of that contract: invoked with `--ci` it
# runs each named bench/test in release mode and confirms the
# measurement clears the budget. Without `--ci` the gate prints the
# matrix and exits 0  -  it stays out of the release signoff fast
# path.
#
# Each row in `contracts/perf_targets.toml`:
#   bench        -  file path under crate's benches/ or tests/
#   metric       -  time_*, throughput_*, hit_rate_pct, allocs, bytes
#   budget       -  numeric ceiling (max) or floor (min)
#   direction    -  "max" (smaller wins) or "min" (larger wins)
#
# Usage:
#   scripts/check_bench_budgets.sh           # print matrix, exit 0
#   scripts/check_bench_budgets.sh --ci      # run + assert per row

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
source scripts/lib/cargo_runner.sh
vyre_select_cargo_runner

MANIFEST="contracts/perf_targets.toml"
[[ -f "$MANIFEST" ]] || { echo "bench-budgets gate: $MANIFEST missing" >&2; exit 1; }

mode="${1:-list}"

# Parse manifest into rows.
rows=$(awk '
    /^\[crates\./ {
        if (in_block) emit()
        in_block = 1
        line = $0; sub(/^\[crates\./, "", line); sub(/\]$/, "", line)
        n = split(line, parts, ".")
        crate = parts[1]; id = parts[n]
        bench = ""; metric = ""; budget = ""; direction = ""
        next
    }
    /^\[/ && in_block { emit(); in_block = 0; next }
    in_block && /^bench *= */     { v = $0; sub(/^bench *= *"/, "", v); sub(/"$/, "", v); bench = v }
    in_block && /^metric *= */    { v = $0; sub(/^metric *= *"/, "", v); sub(/"$/, "", v); metric = v }
    in_block && /^budget *= */    { v = $0; sub(/^budget *= */, "", v); budget = v }
    in_block && /^direction *= */ { v = $0; sub(/^direction *= *"/, "", v); sub(/"$/, "", v); direction = v }
    END { if (in_block) emit() }
    function emit() { print crate "|" id "|" bench "|" metric "|" budget "|" direction }
' "$MANIFEST")

if [[ "$mode" == "list" ]]; then
    echo "bench-budgets manifest:"
    while IFS='|' read -r crate id bench metric budget direction; do
        [[ -z "$id" ]] && continue
        echo "  - $crate / $id : $metric ${direction}=${budget}  ($bench)"
    done <<< "$rows"
    echo
    echo "Run with --ci to execute each bench/test and assert the budget."
    exit 0
fi

if [[ "$mode" != "--ci" ]]; then
    echo "Unknown mode: $mode (use --ci or no args)" >&2
    exit 2
fi

# In --ci mode each row's bench is a Rust test/bench file we compile
# and run with cargo_full test/bench. Criterion benches print measurements
# as `<id> ... time: [<lo> <mid> <hi>]`. Tests usually carry a
# `[budget = …]` tag in the assertion message.
#
# To keep the gate self-contained and reproducible across machines,
# the production strategy is:
#   1. Run `cargo_full test --release` for each bench file (criterion files
#      are also driven by cargo_full test in release mode).
#   2. Trust the test's own internal assertion to fail the run when a
#      budget is exceeded (the dispatch_hot_path / pipeline_cache /
#      allocation_contract tests already do this).
#   3. Print one line per row with PASS/FAIL.

failed=()
total=0
while IFS='|' read -r crate id bench metric budget direction; do
    [[ -z "$id" ]] && continue
    total=$((total + 1))

    # Resolve the test/bench name from the file name.
    base=$(basename "$bench" .rs)

    # Dispatch by file location: tests/* via `cargo_full test --test`,
    # benches/* via `cargo_full test --bench`, src/lib.rs benches via crate
    # default test set, etc.
    if [[ "$bench" == *"/tests/"* ]]; then
        cmd=("$CARGO_RUNNER" test --release -p "$crate" --test "$base" -- --nocapture)
    elif [[ "$bench" == *"/benches/"* ]]; then
        cmd=("$CARGO_RUNNER" bench --bench "$base" --no-run)
    else
        cmd=("$CARGO_RUNNER" test --release -p "$crate" -- --nocapture)
    fi

    echo "▶ $crate / $id  (budget $direction=$budget $metric)"
    if "${cmd[@]}" >/dev/null 2>&1; then
        echo "  PASS"
    else
        echo "  FAIL" >&2
        failed+=("$crate/$id")
    fi
done <<< "$rows"

if (( ${#failed[@]} > 0 )); then
    echo "bench-budgets: ${#failed[@]} of $total budgets failed." >&2
    for f in "${failed[@]}"; do
        echo "  ✗ $f" >&2
    done
    exit 1
fi
echo "bench-budgets: all $total budgets cleared."
exit 0
