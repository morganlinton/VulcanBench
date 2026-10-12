#!/usr/bin/env bash
# Gap #15  -  release engineering.
#
# A reader who sees 0.4.1 on crates.io must be able to:
#   1. `cargo_full install vyre` (from the local path in CI, crates.io
#      elsewhere) without cloning the repo.
#   2. Run the installed `vyre` binary and see a non-trivial demo.
#
# Today: vyre-core is lib-only; `cargo_full install --path vyre-core`
# fails because there is no bin target. Closing the gap means adding
# a minimal CLI to vyre-core (or a dedicated vyre-cli crate) that
# demonstrates dispatching a tiny Program on the local GPU.

set -euo pipefail
cd "$(dirname "$0")/.."

INSTALL_ROOT=$(mktemp -d)
trap 'rm -rf "$INSTALL_ROOT"' EXIT
source scripts/lib/cargo_runner.sh
vyre_select_cargo_runner

if ! "$CARGO_RUNNER" install --path vyre-driver-wgpu --root "$INSTALL_ROOT" --locked 2>&1 | tail -20; then
    echo "gap #15: 'cargo_full install --path vyre-driver-wgpu' failed" >&2
    echo "  fix: the vyre CLI lives in vyre-driver-wgpu (breaks cycle with vyre-core). Keep the [[bin]] target there." >&2
    exit 1
fi

BIN="$INSTALL_ROOT/bin/vyre-wgpu"
if [ ! -x "$BIN" ]; then
    echo "gap #15: installed binary $BIN does not exist" >&2
    exit 1
fi

# Sanity: --version must succeed
if ! "$BIN" --version; then
    echo "gap #15: '$BIN --version' failed" >&2
    exit 1
fi

# The demo must run to completion (timeout 30s so a hung GPU does
# not hold CI).
if ! timeout 30 "$BIN" demo 2>&1 | tail -5; then
    echo "gap #15: '$BIN demo' failed or timed out" >&2
    exit 1
fi

echo "gap #15: release-ready  -  cargo_full install + demo succeed"
