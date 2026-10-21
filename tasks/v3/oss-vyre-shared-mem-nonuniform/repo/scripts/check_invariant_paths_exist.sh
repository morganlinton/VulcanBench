#!/usr/bin/env bash
# Verify every test-file path referenced by vyre-spec/src/invariants.rs
# exists on disk. A broken pointer means a conform test was renamed or
# deleted without updating the invariant descriptor that cites it; either
# restore the test or delete the invariant entry. Documenting a broken
# pointer as a "known missing" path is forbidden (LAW 9).

set -euo pipefail

cd "$(dirname "$0")/.."

missing=0
while IFS= read -r path; do
    case "$path" in
        "conform/tests/<file>.rs")
            # Doc comment example, not a real path.
            continue
            ;;
    esac
    if [ ! -f "$path" ]; then
        printf 'MISSING: %s\n' "$path"
        missing=$((missing + 1))
    fi
done < <(grep -oE 'conform/[^:"]+\.rs' vyre-spec/src/invariants.rs | sort -u)

if [ "$missing" -ne 0 ]; then
    printf '\nFix: either restore the missing conform test file or delete the invariant entry that references it.\n' >&2
    printf 'Total missing paths: %d\n' "$missing" >&2
    exit 1
fi

echo "All invariant test-file paths resolve."
