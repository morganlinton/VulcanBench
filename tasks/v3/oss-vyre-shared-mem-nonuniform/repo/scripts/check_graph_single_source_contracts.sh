#!/usr/bin/env bash
# Enforce graph-domain single-sourcing between vyre-primitives and
# vyre-self-substrate.
#
# Graph algorithms and validation belong in vyre-primitives. Self-substrate may
# add dispatch scratch, batching, resident scheduling, and backend wiring, but a
# release build must fail before wrappers re-grow forked algorithm bodies.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
source scripts/lib/cargo_runner.sh
vyre_select_cargo_runner

CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" "$CARGO_RUNNER" test -q -p vyre-self-substrate --test graph_single_source_contracts
