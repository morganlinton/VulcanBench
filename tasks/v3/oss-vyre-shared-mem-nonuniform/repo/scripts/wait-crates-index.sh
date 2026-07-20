#!/usr/bin/env bash
set -euo pipefail

crate="${1:-}"
version="${2:-}"
max_attempts="${VYRE_CRATES_INDEX_MAX_ATTEMPTS:-60}"
interval_seconds="${VYRE_CRATES_INDEX_INTERVAL_SECONDS:-5}"

if [[ -z "$crate" || -z "$version" ]]; then
    printf 'Fix: usage is bash scripts/wait-crates-index.sh <crate> <version>\n' >&2
    exit 2
fi

if ! [[ "$max_attempts" =~ ^[0-9]+$ && "$max_attempts" -gt 0 ]]; then
    printf 'Fix: VYRE_CRATES_INDEX_MAX_ATTEMPTS must be a positive integer.\n' >&2
    exit 2
fi

if ! [[ "$interval_seconds" =~ ^[0-9]+$ && "$interval_seconds" -gt 0 ]]; then
    printf 'Fix: VYRE_CRATES_INDEX_INTERVAL_SECONDS must be a positive integer.\n' >&2
    exit 2
fi

if ! command -v cargo_full >/dev/null 2>&1; then
    if [[ -x ./cargo_full ]]; then
        cargo_full_cmd=(./cargo_full)
    else
        printf 'Fix: cargo_full is not on PATH and ./cargo_full is not executable from this directory.\n' >&2
        exit 2
    fi
else
    cargo_full_cmd=(cargo_full)
fi

for ((attempt = 1; attempt <= max_attempts; attempt += 1)); do
    if output="$("${cargo_full_cmd[@]}" search "$crate" --limit 1 2>/dev/null)" \
        && printf '%s\n' "$output" | grep -F "${crate} = \"${version}\"" >/dev/null; then
        printf 'crates.io index contains %s %s\n' "$crate" "$version"
        exit 0
    fi
    printf 'waiting for crates.io index: %s %s (%s/%s)\n' "$crate" "$version" "$attempt" "$max_attempts" >&2
    sleep "$interval_seconds"
done

printf 'Fix: crates.io index did not expose %s %s after %s attempts.\n' "$crate" "$version" "$max_attempts" >&2
exit 1
