#!/usr/bin/env bash
# P2 inventory #127  -  machine-readable perf targets gate.
#
# Validates the structure of `contracts/perf_targets.toml` and verifies
# every named bench file exists. Does NOT run benches itself  -  that's
# the job of `scripts/check_bench_baselines.sh` and the deep-bench
# nightly. The gate's purpose is to keep the perf-target manifest
# honest: every entry references a real bench, every bench has a budget.
#
# Usage:
#   scripts/check_perf_targets.sh

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

MANIFEST="contracts/perf_targets.toml"
if [[ ! -f "$MANIFEST" ]]; then
    echo "perf-targets gate: $MANIFEST missing." >&2
    exit 1
fi

VALID_METRICS_RE='^(time_ns|time_us|time_ms|throughput_gibps|throughput_kops|allocs|bytes|hit_rate_pct)$'
VALID_DIRECTION_RE='^(max|min)$'

# Walk each [crates.X.targets.Y] block via awk and emit one row per target.
rows=$(awk '
    /^\[crates\./ {
        # Reset any previous block.
        if (in_block) emit()
        in_block = 1
        # Parse the table id: [crates.<crate>.targets.<id>]
        line = $0
        sub(/^\[crates\./, "", line)
        sub(/\]$/, "", line)
        n = split(line, parts, ".")
        crate = parts[1]
        id = parts[n]
        bench = ""
        metric = ""
        budget = ""
        direction = ""
        next
    }
    /^\[/ && in_block {
        emit()
        in_block = 0
    }
    in_block && /^bench *= */ {
        v = $0; sub(/^bench *= *"/, "", v); sub(/"$/, "", v); bench = v
    }
    in_block && /^metric *= */ {
        v = $0; sub(/^metric *= *"/, "", v); sub(/"$/, "", v); metric = v
    }
    in_block && /^budget *= */ {
        v = $0; sub(/^budget *= */, "", v); budget = v
    }
    in_block && /^direction *= */ {
        v = $0; sub(/^direction *= *"/, "", v); sub(/"$/, "", v); direction = v
    }
    END { if (in_block) emit() }
    function emit() {
        print crate "|" id "|" bench "|" metric "|" budget "|" direction
    }
' "$MANIFEST")

errors=()
count=0

while IFS= read -r row; do
    [[ -z "$row" ]] && continue
    IFS='|' read -r crate id bench metric budget direction <<< "$row"
    count=$((count + 1))
    if [[ -z "$bench" ]]; then
        errors+=("$crate.$id: missing 'bench' field")
        continue
    fi
    if [[ ! -f "$bench" ]]; then
        errors+=("$crate.$id: bench file '$bench' not found")
    fi
    if [[ ! "$metric" =~ $VALID_METRICS_RE ]]; then
        errors+=("$crate.$id: invalid metric '$metric'")
    fi
    if [[ -z "$budget" ]]; then
        errors+=("$crate.$id: missing 'budget' field")
    fi
    if [[ ! "$direction" =~ $VALID_DIRECTION_RE ]]; then
        errors+=("$crate.$id: invalid direction '$direction' (must be max or min)")
    fi
done <<< "$rows"

if (( count == 0 )); then
    errors+=("perf-targets manifest contains zero entries (parser bug or empty file)")
fi

if (( ${#errors[@]} > 0 )); then
    echo "perf-targets gate: ${#errors[@]} violations across $count targets." >&2
    for e in "${errors[@]}"; do
        echo "  $e" >&2
    done
    echo >&2
    echo "Fix: align the manifest with reality  -  either add the missing bench" >&2
    echo "file or remove the target. Every entry MUST point at a real bench." >&2
    exit 1
fi

echo "perf-targets gate: $count targets validated."
exit 0
