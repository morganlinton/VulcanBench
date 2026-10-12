#!/usr/bin/env bash
# P2 inventory #124  -  examples-without-internal-imports gate.
#
# Every example under `examples/` consumes vyre through the public
# facade only. Reaching for `vyre_core::*`, `vyre_foundation::*`,
# `vyre_driver::*`, `vyre_primitives::*`, or any backend crate from an
# example file is a smell  -  it implies the public surface is missing a
# re-export and should be widened, not the example's import.
#
# Exceptions:
#   - `examples/external_ir_extension` deliberately demonstrates how to
#     register an opaque payload with the dialect registry; it imports
#     `vyre_core::dialect::*` and `vyre_core::backend::*` because that
#     IS the demonstration.
#
# Usage:
#   scripts/check_examples_public_facade.sh

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Fully-qualified internal-crate prefixes that examples must NOT import.
FORBIDDEN_RE='\b(vyre_core|vyre_foundation|vyre_driver|vyre_driver_wgpu|vyre_driver_spirv|vyre_driver_cuda|vyre_runtime|vyre_libs|vyre_primitives|vyre_intrinsics|vyre_reference|vyre_spec|vyre_macros|vyre_aot|vyre_cc|vyre_harness|vyre_test_harness)::'

# Examples that document an explicit exception. Adding a new entry
# requires a one-line justification in the example's README.
EXCEPTIONS=(
    # Demonstrates dialect/backend extension  -  internal imports ARE the demo.
    "examples/external_ir_extension"
    # Cat-A library author template  -  community libs DO depend on
    # vyre-libs / vyre-reference / vyre-primitives. The template is not
    # a "consumer" example, it's the starter pack for a new published
    # extension crate.
    "examples/libs-template"
)

errors=()

while IFS= read -r src; do
    [[ -z "$src" ]] && continue
    rel="${src#./}"
    skip=0
    for excp in "${EXCEPTIONS[@]}"; do
        if [[ "$rel" == "$excp"/* ]]; then
            skip=1
            break
        fi
    done
    (( skip )) && continue
    # Skip pure comment / doc-comment lines so the gate doesn't flag the
    # very examples that *describe* the rule.
    hits=$(grep -nE "$FORBIDDEN_RE" "$src" 2>/dev/null \
        | grep -vE '^[0-9]+:[[:space:]]*(//|//!|//#)' \
        || true)
    if [[ -n "$hits" ]]; then
        while IFS= read -r line; do
            [[ -z "$line" ]] && continue
            errors+=("$rel:$line")
        done <<< "$hits"
    fi
done < <(find examples -type f -name '*.rs' 2>/dev/null)

if (( ${#errors[@]} > 0 )); then
    echo "examples-public-facade gate: ${#errors[@]} forbidden imports." >&2
    for e in "${errors[@]}"; do
        echo "  $e" >&2
    done
    echo >&2
    echo "Fix: examples consume vyre via the \`vyre\` crate only." >&2
    echo "If a needed type is not re-exported, widen the public facade" >&2
    echo "(item #121)  -  do not reach into internal crates from examples." >&2
    exit 1
fi

count=$(find examples -type f -name '*.rs' 2>/dev/null | wc -l | tr -d ' ')
echo "examples-public-facade gate: $count example .rs files clean."
exit 0
