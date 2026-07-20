#!/usr/bin/env bash
# Backend extension contract: a new backend is one crate plus inventory submits.
#
# The core driver must expose inventory collections and frozen registry views;
# concrete backend crates must own their implementation, depend on
# `vyre-driver`, and submit BackendRegistration, BackendPrecedence, and
# BackendCapability from their own crate. The core registry must not contain a
# hand-maintained list of concrete backend ids.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

failures=0

fail() {
    echo "backend-extension-contract: $1" >&2
    failures=$((failures + 1))
}

require_grep() {
    local pattern="$1"
    local path="$2"
    local message="$3"
    if ! grep -Eq "$pattern" "$path"; then
        fail "$message"
    fi
}

require_rg() {
    local pattern="$1"
    local path="$2"
    local message="$3"
    if ! rg -q "$pattern" "$path"; then
        fail "$message"
    fi
}

inventory_file="vyre-driver/src/backend/registry/inventory_streams.rs"
acquire_file="vyre-driver/src/backend/registry/acquire.rs"

require_grep 'inventory::collect!\(BackendRegistration\);' "$inventory_file" \
    "BackendRegistration must be an inventory collection in $inventory_file"
require_grep 'inventory::collect!\(BackendPrecedence\);' "$inventory_file" \
    "BackendPrecedence must be an inventory collection in $inventory_file"
require_grep 'inventory::collect!\(BackendCapability\);' "$inventory_file" \
    "BackendCapability must be an inventory collection in $inventory_file"
require_grep 'OnceLock<Box<\[\&'"'"'static BackendRegistration\]>>' "$inventory_file" \
    "registered_backends must freeze inventory into a process-wide OnceLock slice"
require_grep 'inventory::iter::<BackendRegistration>' "$inventory_file" \
    "registered_backends must be populated from inventory::iter::<BackendRegistration>"
require_grep 'registered_backends_by_precedence_slice' "$acquire_file" \
    "backend acquisition must route through the precedence-sorted frozen slice"
require_grep 'backend_dispatches' "$acquire_file" \
    "preferred backend acquisition must consult BackendCapability dispatch metadata"

if rg -n '"(cuda|wgpu|spirv|metal|dxil)"' vyre-driver/src/backend/registry >/tmp/vyre-backend-hardcoded-ids.txt; then
    fail "core backend registry contains concrete backend id literals; new backends must not require editing vyre-driver/src/backend/registry"
    sed -n '1,20p' /tmp/vyre-backend-hardcoded-ids.txt >&2
fi
rm -f /tmp/vyre-backend-hardcoded-ids.txt

for crate in vyre-driver-cuda vyre-driver-wgpu vyre-driver-spirv vyre-driver-reference; do
    if [[ ! -f "$crate/Cargo.toml" ]]; then
        fail "$crate/Cargo.toml missing"
        continue
    fi
    if [[ ! -d "$crate/src" ]]; then
        fail "$crate/src missing"
        continue
    fi
    require_grep 'vyre-driver' "$crate/Cargo.toml" \
        "$crate must depend on vyre-driver instead of editing core registry code"
    require_grep 'inventory\.workspace|inventory[[:space:]]*=' "$crate/Cargo.toml" \
        "$crate must depend on inventory for link-time backend registration"
    require_rg 'impl .*VyreBackend for' "$crate/src" \
        "$crate must implement VyreBackend in its own crate"
    require_rg 'inventory::submit![[:space:]]*\{' "$crate/src" \
        "$crate must submit backend metadata through inventory::submit!"
    require_rg 'BackendRegistration[[:space:]]*\{' "$crate/src" \
        "$crate must submit BackendRegistration"
    require_rg 'BackendPrecedence[[:space:]]*\{' "$crate/src" \
        "$crate must submit BackendPrecedence"
    require_rg 'BackendCapability[[:space:]]*\{' "$crate/src" \
        "$crate must submit BackendCapability so dispatch ownership is explicit"
    require_rg 'supported_ops[[:space:]]*:' "$crate/src" \
        "$crate BackendRegistration must advertise supported_ops"
done

if (( failures > 0 )); then
    echo "backend-extension-contract gate failed with $failures violation(s)." >&2
    echo "Fix: keep backend addition as one concrete crate implementing VyreBackend and registering via inventory::submit!." >&2
    exit 1
fi

echo "backend-extension-contract gate: backend addition remains one crate + inventory::submit!."
