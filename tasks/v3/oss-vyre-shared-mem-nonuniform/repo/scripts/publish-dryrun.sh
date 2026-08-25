#!/usr/bin/env bash
#
# Deep pre-publish gate for the vyre workspace.
#
# Runs every automated gate we require before a publish can be trusted:
#
#   - formatting, narrow+workspace type checks, warnings-as-errors clippy
#   - every architectural law (A/B/C/D/H + pure-crate dependency invariant)
#   - rebuild_status.sh dashboard
#   - unit + doc + integration test suites per crate
#   - rustdoc with warnings-as-errors (no "missing docs" slips through)
#   - cargo_full publish --dry-run per publishable crate (dependency-ordered) with
#     required metadata checks
#
# The script exits non-zero on the FIRST failure, so the final "READY TO
# PUBLISH" line is trustworthy. Use it as the human's last check before
# actually running `cargo_full publish`.
#
# Usage: bash scripts/publish-dryrun.sh [crate-name ...]
#   - no arguments: run every gate and every publishable crate
#   - crate-name list: limit publish dry-runs to those crates (all other gates
#     still run)

set -euo pipefail

VYRE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$VYRE_ROOT"
source scripts/lib/cargo_runner.sh
vyre_select_cargo_runner

RED=$'\033[31m'
GREEN=$'\033[32m'
YELLOW=$'\033[33m'
RESET=$'\033[0m'

PASS=0
FAIL=0
FAIL_NAMES=()

check() {
    local name="$1"
    shift
    printf '  [%s] %s\n' "…" "$name"
    if "$@" >/tmp/vyre-publish-gate.log 2>&1; then
        printf '\r  [%s%s%s] %s\n' "$GREEN" "✓" "$RESET" "$name"
        PASS=$((PASS+1))
    else
        printf '\r  [%s%s%s] %s\n' "$RED" "✗" "$RESET" "$name"
        FAIL=$((FAIL+1))
        FAIL_NAMES+=("$name")
        echo "    ↳ last 20 lines of output:"
        tail -n 20 /tmp/vyre-publish-gate.log | sed 's/^/      /'
    fi
}

section() {
    printf '\n%s%s%s\n' "$YELLOW" "$1" "$RESET"
}

# ─── Gates ──────────────────────────────────────────────────────────────────

section "Architectural laws"
check "pure-crate dependency invariant"        bash scripts/check_architectural_invariants.sh
check "Law A  -  no closed IR enums"             bash scripts/check_no_closed_ir_enums.sh
check "Law B  -  no string WGSL"                 bash scripts/check_no_string_wgsl.sh
check "Law C  -  capability negotiation"         bash scripts/check_capability_negotiation.sh
check "Law D  -  registry consistency"           bash scripts/check_registry_consistency.sh
check "Law H  -  unsafe SAFETY comments"         bash scripts/check_unsafe_justifications.sh
if [[ -x scripts/check_trait_freeze.sh ]]; then
    check "trait-surface freeze"               bash scripts/check_trait_freeze.sh
fi

section "Code health"
check "cargo_full fmt --check"                 "$CARGO_RUNNER" fmt --check
check "cargo_full check --workspace"           "$CARGO_RUNNER" check --workspace --all-targets
check "cargo_full clippy --workspace"          "$CARGO_RUNNER" clippy --workspace --all-targets -- -D warnings

section "Test suites"
# Every crate whose tests we care about at publish time. Kept explicit so a
# new crate doesn't slip through uncovered.
TEST_CRATES=(
    vyre
    vyre-core
    vyre-spec
    vyre-reference
    vyre-primitives
    vyre-driver-wgpu
    vyre-driver-cuda
    vyre-driver-spirv
    vyre-runtime
    vyre-foundation
    vyre-driver
    vyre-libs
    vyre-aot
    vyre-cc
)
for crate in "${TEST_CRATES[@]}"; do
    if [[ -d "$crate" ]]; then
        check "cargo_full test -p $crate --lib" "$CARGO_RUNNER" test -p "$crate" --lib
    fi
done

section "Rustdoc (warnings denied)"
check "cargo_full doc --workspace"             env RUSTDOCFLAGS="-D warnings" "$CARGO_RUNNER" doc --workspace --no-deps

section "Rebuild status dashboard"
check "scripts/rebuild_status.sh"              bash scripts/rebuild_status.sh

# ─── Publish dry-runs ───────────────────────────────────────────────────────

PACKAGE_READINESS="release/evidence/package/publish-readiness.json"
check "xtask package-readiness" "$CARGO_RUNNER" run -j1 --manifest-path xtask/Cargo.toml --bin xtask -- package-readiness --output "$PACKAGE_READINESS"
if [[ ! -f "$PACKAGE_READINESS" ]]; then
    printf '%sNOT READY: missing %s%s\n' "$RED" "$PACKAGE_READINESS" "$RESET"
    exit 1
fi
mapfile -t PUBLISH_ENTRIES < <(jq -r '.publish_order[] | [.package, .manifest] | @tsv' "$PACKAGE_READINESS")
if [[ "${#PUBLISH_ENTRIES[@]}" -eq 0 ]]; then
    printf '%sNOT READY: publish_order is empty in %s%s\n' "$RED" "$PACKAGE_READINESS" "$RESET"
    exit 1
fi

# If the caller passed crate names, filter the order to just those.
if [[ $# -gt 0 ]]; then
    REQUESTED=("$@")
    FILTERED=()
    for entry in "${PUBLISH_ENTRIES[@]}"; do
        crate="${entry%%$'\t'*}"
        for req in "${REQUESTED[@]}"; do
            if [[ "$crate" == "$req" ]]; then
                FILTERED+=("$entry")
            fi
        done
    done
    PUBLISH_ENTRIES=("${FILTERED[@]}")
fi

section "Publishable-crate metadata"
for entry in "${PUBLISH_ENTRIES[@]}"; do
    crate="${entry%%$'\t'*}"
    manifest="${entry#*$'\t'}"
    dir="$(dirname "$manifest")"
    if [[ ! -d "$dir" ]]; then
        printf '  %s[skip]%s %s (directory missing)\n' "$YELLOW" "$RESET" "$dir"
        continue
    fi
    check "$dir: README.md present"            test -f "$dir/README.md"
    check "$dir: LICENSE-MIT present"          test -f "$dir/LICENSE-MIT"
    check "$dir: LICENSE-APACHE present"       test -f "$dir/LICENSE-APACHE"
    check "$dir: description declared"         grep -Eq '^description[[:space:]]*=' "$dir/Cargo.toml"
    check "$dir: keywords declared"            grep -Eq '^keywords[[:space:]]*=' "$dir/Cargo.toml"
    check "$dir: categories declared"          grep -Eq '^categories[[:space:]]*=' "$dir/Cargo.toml"
    check "$dir: readme declared"              grep -Eq '^readme[[:space:]]*=' "$dir/Cargo.toml"
    check "$dir: license declared"             grep -Eq '^license[[:space:]]*=' "$dir/Cargo.toml"
    check "$dir: repository declared"          grep -Eq '^repository[[:space:]]*=' "$dir/Cargo.toml"
done

section "Publish dry-run (dependency-ordered)"
for entry in "${PUBLISH_ENTRIES[@]}"; do
    crate="${entry%%$'\t'*}"
    manifest="${entry#*$'\t'}"
    check "cargo_full publish --dry-run --manifest-path $manifest ($crate)" "$CARGO_RUNNER" publish --dry-run --allow-dirty --manifest-path "$manifest"
done

# ─── Summary ────────────────────────────────────────────────────────────────

printf '\n'
if [[ "$FAIL" -eq 0 ]]; then
    printf '%sREADY TO PUBLISH%s (%d gates passed)\n' "$GREEN" "$RESET" "$PASS"
    printf '\nSuggested publish command (dependency-ordered):\n'
    for entry in "${PUBLISH_ENTRIES[@]}"; do
        crate="${entry%%$'\t'*}"
        manifest="${entry#*$'\t'}"
        printf '  %s publish --manifest-path %s # %s\n' "$CARGO_RUNNER" "$manifest" "$crate"
    done
    exit 0
else
    printf '%sNOT READY: %d gate(s) failed%s (%d passed, %d failed)\n' \
        "$RED" "$FAIL" "$RESET" "$PASS" "$FAIL"
    printf '\nFailing gates:\n'
    for name in "${FAIL_NAMES[@]}"; do
        printf '  - %s\n' "$name"
    done
    printf '\nRerun individual checks from scripts/ to iterate.\n'
    exit 1
fi
