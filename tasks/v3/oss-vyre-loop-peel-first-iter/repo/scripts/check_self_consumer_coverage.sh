#!/usr/bin/env bash
# E3 / F3 / F5 - recursion thesis: every registered executable Tier-2.5
# primitive has a named self-consumer wrapper.
#
# The old version counted top-level `pub fn` symbols and searched a removed
# `vyre-driver/src/self_substrate/` tree. That confused helpers, CPU oracles,
# validators, and `try_*` adapters with executable primitive contracts.
#
# This gate measures the architectural boundary directly: primitive harness
# registrations under `vyre-primitives` must be represented by the current
# self-consumer catalog surface.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

mode="${1:-enforce}"
case "$mode" in
    enforce|--report) ;;
    *)
        echo "Usage: scripts/check_self_consumer_coverage.sh [--report]" >&2
        exit 2
        ;;
esac

python3 scripts/check_self_consumer_coverage.py "$mode"
