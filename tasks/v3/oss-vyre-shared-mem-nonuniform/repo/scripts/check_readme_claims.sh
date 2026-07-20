#!/usr/bin/env bash
# README + docs claims verified against filesystem reality.
#
# Every numerical claim in user-facing docs becomes a gate. If README
# says "150 ops" and the registry has 211, that's a lie.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

failed=0

# Caching mechanism
CACHE_DIR="target"
CACHE_FILE="$CACHE_DIR/readme_claims_cache.txt"
mkdir -p "$CACHE_DIR"

repo_sha="$(git rev-parse HEAD 2>/dev/null || echo "no-git")"
content_hash="empty"
if [[ -d "vyre-core/src" ]]; then
    content_hash="$(find vyre-core/src -name '*.rs' -type f 2>/dev/null | sort | xargs sha256sum 2>/dev/null | sha256sum | cut -d' ' -f1)"
fi
cache_key="${repo_sha}_${content_hash}"

use_cache=0
if [[ -f "$CACHE_FILE" ]]; then
    read -r stored_key cached_ops cached_files < "$CACHE_FILE"
    if [[ "$stored_key" == "$cache_key" ]]; then
        actual_ops="$cached_ops"
        actual_rs_files="$cached_files"
        use_cache=1
        echo "README claims cache hit. Using cached filesystem metrics."
    fi
fi

if [[ "$use_cache" -eq 0 ]]; then
    actual_ops="$(
        grep -rhE '([^a-z_]|^)id[[:space:]]*[:=][[:space:]]*"[a-z_][a-z0-9_.]*"' \
            --include='*.rs' vyre-core/src 2>/dev/null \
        | grep -oE '"[a-z_][a-z0-9_.]*"' | tr -d '"' \
        | grep -E '^[a-z_][a-z0-9_]*(\.[a-z_][a-z0-9_]*)+$' \
        | sort -u | wc -l | tr -d ' '
    )"
    actual_rs_files="$(find vyre-core/src -name '*.rs' 2>/dev/null | wc -l | tr -d ' ')"
    echo "$cache_key $actual_ops $actual_rs_files" > "$CACHE_FILE"
fi

echo "Filesystem reality:"
echo "  op ids: $actual_ops"
echo "  vyre-core .rs files: $actual_rs_files"

for doc in README.md docs/ARCHITECTURE.md docs/THESIS.md VISION.md; do
    [[ ! -f "$doc" ]] && continue
    # Op-count claims  -  exclude "IEEE 754" and similar standard numbers.
    # Match "150+ ops" but not "IEEE 754 operations".
    claims="$(grep -oE '[0-9]{2,}\+?[[:space:]]*(primitives|ops|operations|operators)' "$doc" 2>/dev/null \
        | grep -v -E '^7(53|54)' \
        | grep -oE '^[0-9]+' | head -3)"
    while IFS= read -r claimed; do
        [[ -z "$claimed" ]] && continue
        diff=$(( claimed > actual_ops ? claimed - actual_ops : actual_ops - claimed ))
        tolerance=$(( actual_ops / 5 ))
        [[ "$tolerance" -lt 20 ]] && tolerance=20
        if [[ "$diff" -gt "$tolerance" ]]; then
            echo "  ✗ $doc claims '${claimed}' ops; filesystem has ${actual_ops} (drift: ${diff})" >&2
            failed=1
        fi
    done <<< "$claims"
done

[[ "$failed" -ne 0 ]] && { echo "Fix: update docs OR implement the claimed functionality." >&2; exit 1; }
echo "README claims verified."
