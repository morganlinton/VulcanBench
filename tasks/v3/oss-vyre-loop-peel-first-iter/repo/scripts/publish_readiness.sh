#!/usr/bin/env bash
# P2 inventory #126  -  one-command publish readiness dry run.
#
# Composite gate that runs every check a maintainer needs to clear
# before tagging a release. Includes:
#
#   1. release signoff composite (every architectural invariant).
#   2. doc + features matrix smoke (`./cargo_full doc --workspace`).
#   3. feature-MSRV matrix (`scripts/check_feature_msrv.sh --ci`).
#   4. publish dry-run (`scripts/publish-dryrun.sh`).
#   5. GPU + VYRE smoke (`scripts/vyre_smoke.sh`).
#
# Each step prints its own status; the script aborts on the first
# failure with a labeled exit code so CI can fan out to the right
# follow-up.
#
# Usage:
#   scripts/publish_readiness.sh

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
source scripts/lib/cargo_runner.sh
vyre_select_cargo_runner

step() {
    echo
    echo "═══════════════════════════════════════════════════════════════"
    echo "▶ $*"
    echo "═══════════════════════════════════════════════════════════════"
}

fail() {
    local step_name="$1"
    local exit_code="$2"
    echo
    echo "publish-readiness: $step_name FAILED. Aborting with exit code $exit_code." >&2
    exit "$exit_code"
}

step "1/6 release signoff (architectural invariants)"
if ! bash scripts/check_release_signoff.sh; then
    fail "release signoff" 11
fi

step "2/6 cargo_full doc --workspace --all-features"
if ! "$CARGO_RUNNER" doc --workspace --all-features --no-deps --quiet 2>&1 | tail -10; then
    fail "cargo_full doc" 12
fi

step "3/6 feature-MSRV matrix"
if ! bash scripts/check_feature_msrv.sh --ci; then
    fail "feature-MSRV matrix" 13
fi

step "4/6 publish dry-run"
if ! bash scripts/publish-dryrun.sh; then
    fail "publish dry-run" 14
fi

step "5/5 GPU + VYRE smoke"
if ! bash scripts/vyre_smoke.sh; then
    fail "vyre_smoke" 16
fi

echo
echo "═══════════════════════════════════════════════════════════════"
echo "publish-readiness: ALL 5 STEPS GREEN."
echo "═══════════════════════════════════════════════════════════════"
echo
echo "Manual checklist before tagging:"
echo "  [ ] CHANGELOG.md updated for the release."
echo "  [ ] Version bumps applied to every workspace crate that ships."
echo "  [ ] OWNERSHIP.md and tier_config_manifest.toml reflect the publish set."
echo "  [ ] external_ir_extension example builds against the candidate."
echo "  [ ] Three-substrate parity certificate generated."
echo "  [ ] vyre-bench meta-harness implementation tracked against docs/VYRE_BENCH_META_HARNESS_PRD.md."
exit 0
