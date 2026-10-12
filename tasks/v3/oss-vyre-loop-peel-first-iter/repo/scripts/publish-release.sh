#!/usr/bin/env bash
#
# Guarded crates.io publish launcher for the configured Vyre / Weir release train.
#
# This script intentionally refuses to run unless the maintainer sets:
#   VYRE_RELEASE_APPROVED=<token derived by scripts/lib/release_train.sh>
#
# It derives the publish order from release/evidence/package/publish-readiness.json
# so final publish cannot drift from the audited dependency order.

set -euo pipefail

PREFLIGHT=0
if [[ "${1:-}" == "--preflight" ]]; then
    PREFLIGHT=1
    shift
fi
if [[ "$#" -ne 0 ]]; then
    printf 'Fix: unknown publish-release argument(s): %s\n' "$*" >&2
    exit 2
fi

VYRE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$VYRE_ROOT"

source scripts/lib/release_train.sh
vyre_load_release_train

APPROVAL_TOKEN="$VYRE_RELEASE_PUBLISH_APPROVAL_TOKEN"
if [[ "$PREFLIGHT" != "1" && "${VYRE_RELEASE_APPROVED:-}" != "$APPROVAL_TOKEN" ]]; then
    printf 'Fix: refusing to publish without explicit approval.\n' >&2
    printf 'Set VYRE_RELEASE_APPROVED=%s only after the maintainer approves crates.io publish.\n' "$APPROVAL_TOKEN" >&2
    exit 2
fi

source scripts/lib/cargo_runner.sh
vyre_select_cargo_runner
PACKAGE_READINESS="release/evidence/package/publish-readiness.json"

if ! command -v jq >/dev/null 2>&1; then
    printf 'Fix: jq is required to read %s.\n' "$PACKAGE_READINESS" >&2
    exit 2
fi

"$CARGO_RUNNER" run -j1 --manifest-path xtask/Cargo.toml --bin xtask -- package-readiness --output "$PACKAGE_READINESS"

blocker_count="$(jq '.blockers | length' "$PACKAGE_READINESS")"
if [[ "$blocker_count" != "0" ]]; then
    printf 'Fix: package readiness has %s blocker(s); refusing publish.\n' "$blocker_count" >&2
    jq -r '.blockers[]' "$PACKAGE_READINESS" >&2
    exit 1
fi

mapfile -t PUBLISH_ENTRIES < <(jq -r '.publish_order[] | [.package, .version, .manifest] | @tsv' "$PACKAGE_READINESS")
if [[ "${#PUBLISH_ENTRIES[@]}" -eq 0 ]]; then
    printf 'Fix: publish_order is empty in %s.\n' "$PACKAGE_READINESS" >&2
    exit 1
fi

if [[ "$PREFLIGHT" == "1" ]]; then
    printf 'publish-release preflight passed for %s audited crate(s); no crates.io publish performed.\n' "${#PUBLISH_ENTRIES[@]}"
    exit 0
fi

crate_version_visible() {
    local package="$1"
    local version="$2"
    local output
    if output="$("$CARGO_RUNNER" search "$package" --limit 1 2>/dev/null)" \
        && printf '%s\n' "$output" | grep -F "${package} = \"${version}\"" >/dev/null; then
        return 0
    fi
    return 1
}

for entry in "${PUBLISH_ENTRIES[@]}"; do
    package="${entry%%$'\t'*}"
    rest="${entry#*$'\t'}"
    version="${rest%%$'\t'*}"
    manifest="${rest#*$'\t'}"
    if crate_version_visible "$package" "$version"; then
        printf 'already visible on crates.io: %s %s\n' "$package" "$version"
        if [[ "${VYRE_RELEASE_SKIP_INDEX_WAIT:-}" != "1" ]]; then
            bash scripts/wait-crates-index.sh "$package" "$version"
        fi
        continue
    fi
    printf 'publishing %s %s from %s\n' "$package" "$version" "$manifest"
    "$CARGO_RUNNER" publish --manifest-path "$manifest"
    if [[ "${VYRE_RELEASE_SKIP_INDEX_WAIT:-}" != "1" ]]; then
        bash scripts/wait-crates-index.sh "$package" "$version"
    fi
done

printf 'crates.io publish completed for audited Vyre/Weir publish order.\n'
printf 'Remaining launch actions: make repositories public, then push release branch and tags after explicit approval.\n'
