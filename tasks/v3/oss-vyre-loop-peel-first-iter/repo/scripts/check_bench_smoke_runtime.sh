#!/usr/bin/env bash
# CI smoke benchmark wall-clock gate.
#
# The full perf-target manifest carries many hardware-sensitive targets; this
# gate enforces the PR-safe smoke-suite budget so benchmarks cannot silently
# grow into non-running documentation.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
source scripts/lib/cargo_runner.sh
vyre_select_cargo_runner

MANIFEST="contracts/perf_targets.toml"
TARGET="benches-smoke.smoke_runtime"

budget_ms=$(awk '
    /^\[crates\.benches-smoke\.targets\.smoke_runtime\]/ { in_block = 1; next }
    /^\[/ && in_block { in_block = 0 }
    in_block && /^budget *= */ {
        v = $0
        sub(/^budget *= */, "", v)
        print v
        exit
    }
' "$MANIFEST")

if [[ -z "$budget_ms" ]]; then
    echo "bench-smoke gate: missing budget for $TARGET in $MANIFEST" >&2
    exit 1
fi

"$CARGO_RUNNER" build -q -p vyre-bench
target_dir="${CARGO_TARGET_DIR:-target}"
bench_bin="$target_dir/debug/vyre-bench"
if [[ ! -x "$bench_bin" ]]; then
    echo "bench-smoke gate: expected built benchmark binary at $bench_bin" >&2
    echo "Fix: set CARGO_TARGET_DIR to the target dir used by $CARGO_RUNNER or repair the vyre-bench build." >&2
    exit 1
fi

"$bench_bin" list --format json >/dev/null

start_ms=$(date +%s%3N)
"$bench_bin" run \
    --suite smoke \
    --format json \
    --case foundation.elementwise.add.1m \
    --warmup-samples 0 \
    --measured-samples 30 \
    --sample-timeout-secs 30 \
    --determinism-runs 1 >/dev/null
end_ms=$(date +%s%3N)
elapsed_ms=$((end_ms - start_ms))

if (( elapsed_ms > budget_ms )); then
    echo "bench-smoke gate: ${elapsed_ms}ms exceeded ${budget_ms}ms budget." >&2
    echo "Fix: reduce canonical vyre-bench smoke runtime or move heavy cases to release/deep suites." >&2
    exit 1
fi

echo "bench-smoke gate: ${elapsed_ms}ms within ${budget_ms}ms budget."
