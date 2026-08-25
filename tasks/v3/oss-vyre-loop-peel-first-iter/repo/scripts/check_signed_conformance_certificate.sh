#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
export RUSTC_WRAPPER=""

source scripts/lib/cargo_runner.sh
vyre_select_cargo_runner

export VYRE_RELEASE_BACKEND="${VYRE_RELEASE_BACKEND:-all}"
export VYRE_RELEASE_SHARDS="${VYRE_RELEASE_SHARDS:-64}"
export VYRE_RELEASE_FEATURES="${VYRE_RELEASE_FEATURES:-gpu}"
export VYRE_RELEASE_CERT_DIR="${VYRE_RELEASE_CERT_DIR:-.internals/certs/signoff-shards}"

merged_certificate="$(scripts/prove-release-shards.sh)"
if [[ ! -s "$merged_certificate" ]]; then
    printf 'Fix: signed conformance gate did not produce a merged certificate: %s\n' "$merged_certificate" >&2
    exit 1
fi

printf 'signed-conformance-certificate gate: sharded %s-backend certificate verified at %s.\n' "$VYRE_RELEASE_BACKEND" "$merged_certificate"
