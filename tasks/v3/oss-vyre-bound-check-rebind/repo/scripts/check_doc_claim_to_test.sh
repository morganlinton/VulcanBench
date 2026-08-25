#!/usr/bin/env bash
# P1 inventory #111  -  doc-claim-to-test gate.
#
# Walks `contracts/doc_claims_manifest.toml`. For each claim row:
#   - The `doc` file must exist and must contain the literal `phrase`.
#   - The `test` file must exist.
#
# This makes "for every claim, there is a test" a hard CI gate. New
# claims land alongside their proving test; old claims either keep
# their test green or get removed from the manifest *and* the doc.
#
# Doctrine: claim drift is the number-one way docs lie. Pinning each
# claim to a file path forces the doc edit to ride alongside the test
# edit (and vice versa).
#
# Usage:
#   scripts/check_doc_claim_to_test.sh

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

MANIFEST="contracts/doc_claims_manifest.toml"
if [[ ! -f "$MANIFEST" ]]; then
    echo "doc-claim-to-test gate: $MANIFEST missing." >&2
    exit 1
fi

errors=()
count=0

# Parse [[claim]] blocks.
rows=$(awk '
    /^\[\[claim\]\]/ {
        if (in_block) emit()
        in_block = 1
        id = ""; doc = ""; phrase = ""; test = ""
        next
    }
    /^\[/ && in_block { emit(); in_block = 0; next }
    in_block && /^id *= */         { v = $0; sub(/^id *= *"/, "", v); sub(/"$/, "", v); id = v }
    in_block && /^doc *= */        { v = $0; sub(/^doc *= *"/, "", v); sub(/"$/, "", v); doc = v }
    in_block && /^phrase *= */     { v = $0; sub(/^phrase *= *"/, "", v); sub(/"$/, "", v); phrase = v }
    in_block && /^test *= */       { v = $0; sub(/^test *= *"/, "", v); sub(/"$/, "", v); test = v }
    END { if (in_block) emit() }
    function emit() { print id "|" doc "|" phrase "|" test }
' "$MANIFEST")

while IFS= read -r row; do
    [[ -z "$row" ]] && continue
    IFS='|' read -r id doc phrase test <<< "$row"
    count=$((count + 1))
    if [[ -z "$id" || -z "$doc" || -z "$phrase" || -z "$test" ]]; then
        errors+=("$id: missing one of id/doc/phrase/test fields")
        continue
    fi
    if [[ ! -f "$doc" ]]; then
        errors+=("$id: doc file '$doc' not found")
        continue
    fi
    if ! grep -qF "$phrase" "$doc"; then
        errors+=("$id: phrase '$phrase' not found in '$doc'")
    fi
    # The test path may be a directory or file; either is acceptable.
    if [[ ! -e "$test" ]]; then
        errors+=("$id: test path '$test' not found")
    fi
done <<< "$rows"

if (( count == 0 )); then
    errors+=("doc-claim-to-test gate: zero claims parsed (parser bug or empty file)")
fi

if (( ${#errors[@]} > 0 )); then
    echo "doc-claim-to-test gate: ${#errors[@]} violations across $count claims." >&2
    for e in "${errors[@]}"; do
        echo "  $e" >&2
    done
    echo >&2
    echo "Fix: every doc claim has a proving test. Update the manifest" >&2
    echo "AND the doc/test together. Removing a claim removes both rows." >&2
    exit 1
fi

echo "doc-claim-to-test gate: $count claims validated."
exit 0
