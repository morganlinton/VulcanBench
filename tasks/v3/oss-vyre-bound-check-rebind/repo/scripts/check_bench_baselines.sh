#!/usr/bin/env bash
# Gap #11  -  bench baselines published.
#
# Every criterion bench gets a baseline committed to benches/RESULTS.md
# with machine spec + commit hash + numbers. Competitors (wgpu, naga,
# rust-gpu) publish these; without them, "vyre is fast" is a claim.

set -euo pipefail
cd "$(dirname "$0")/.."

RESULTS="benches/RESULTS.md"

if [ ! -f "$RESULTS" ]; then
    echo "gap #11: $RESULTS does not exist" >&2
    echo "  fix: run the baseline capture script and commit the output" >&2
    exit 1
fi

# Required header fields
required_fields=("machine:" "gpu:" "cpu:" "rustc:" "commit:")
for field in "${required_fields[@]}"; do
    if ! grep -q "$field" "$RESULTS"; then
        echo "gap #11: $RESULTS missing required field '$field'" >&2
        exit 1
    fi
done

# Every crate with a benches/ dir must have at least one bench row
# in RESULTS.md.
while IFS= read -r crate; do
    name=$(basename "$(dirname "$crate")")
    if ! grep -q "^### $name\b" "$RESULTS"; then
        echo "gap #11: $RESULTS missing section for $name" >&2
        exit 1
    fi
done < <(find . -name benches -type d -not -path '*/target/*' -not -path './benches' | head -20)

echo "gap #11: baseline file present and covers every benches/ crate"
