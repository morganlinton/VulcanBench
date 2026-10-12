#!/usr/bin/env bash
# Every stable error code emitted by vyre source must appear in
# docs/error-codes.md. Prose drifts; codes don't. Catching a code that's
# been added to source without a doc entry lets tooling keep up.
#
# Scans for V### (3-digit), E-*, W-*, B-*, C-* tokens inside Rust string
# literals and verifies each appears in the doc.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

DOC="docs/error-codes.md"
if [[ ! -f "$DOC" ]]; then
    echo "Missing catalog: $DOC. Fix: create the registry before adding new codes." >&2
    exit 1
fi

# Extract codes from source (inside "..." strings only  -  grepping the
# full file would false-positive on module names like `V013.rs`).
SEARCH_DIRS=(
    vyre-foundation/src
    vyre-spec/src
    vyre-driver/src
    vyre-reference/src
    vyre-driver-wgpu/src
    vyre-driver-spirv/src
)

codes_in_source="$(
    grep -rEho '"(V[0-9]{3}|[E|W|B|C]-[A-Z\-]+)[^"]*' \
        --include='*.rs' \
        "${SEARCH_DIRS[@]}" \
        2>/dev/null \
    | grep -oE '(V[0-9]{3}|[E|W|B|C]-[A-Z\-]+)' \
    | sort -u
)"

missing=0
while IFS= read -r code; do
    if [[ -z "$code" ]]; then continue; fi
    if ! grep -Fq "\`${code}\`" "$DOC"; then
        echo "Uncataloged error code: ${code}. Fix: add a row to ${DOC}." >&2
        missing=1
    fi
done <<< "$codes_in_source"

if [[ "$missing" -ne 0 ]]; then exit 1; fi
count="$(echo "$codes_in_source" | grep -c . || true)"
echo "Error codes cataloged: ${count} codes verified against ${DOC}."
