#!/usr/bin/env bash
# P0 inventory #73  -  workspace metadata/lints normalization gate.
#
# Every workspace member must:
#   - inherit `edition`, `rust-version`, `license`, `authors`, `repository`,
#     `homepage` from the workspace
#   - declare a non-empty `description`
#   - either `[lints] workspace = true` OR `publish = false` and a
#     `[lints] workspace = true` block (the goal is uniform lints across
#     every published and unpublished member; cargo-fuzz subworkspaces are
#     the only exception)
#
# This freezes the metadata baseline so a new crate cannot land without
# the same hygiene surface that 0.6 ships with.
#
# Usage:
#   scripts/check_crate_metadata_normalized.sh

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Workspace members. Kept in sync with `[workspace] members` in
# Cargo.toml. The cargo-fuzz subworkspace `vyre-foundation/fuzz` lives
# in a nested workspace and is excluded by design.
MEMBERS=(
    "vyre-core"
    "vyre-foundation"
    "vyre-driver"
    "vyre-driver-wgpu"
    "vyre-driver-spirv"
    "vyre-driver-cuda"
    "vyre-reference"
    "vyre-spec"
    "vyre-macros"
    "vyre-primitives"
    "conform/vyre-conform-spec"
    "conform/vyre-conform-generate"
    "conform/vyre-conform-enforce"
    "conform/vyre-conform-runner"
    "conform/vyre-test-harness"
    "xtask"
    "vyre-runtime"
    "vyre-libs"
    "vyre-intrinsics"
    "vyre-frontend-c"
    "vyre-aot"
)

REQUIRED_INHERIT=(
    "edition.workspace = true"
    "rust-version.workspace = true"
    "license.workspace = true"
    "authors.workspace = true"
    "repository.workspace = true"
    "homepage.workspace = true"
)

errors=()

for crate in "${MEMBERS[@]}"; do
    cargo_toml="$crate/Cargo.toml"
    if [[ ! -f "$cargo_toml" ]]; then
        errors+=("$crate: missing Cargo.toml")
        continue
    fi
    for field in "${REQUIRED_INHERIT[@]}"; do
        if ! grep -qF "$field" "$cargo_toml"; then
            errors+=("$crate: missing '$field'")
        fi
    done
    if ! grep -qE '^description\s*=\s*"' "$cargo_toml"; then
        errors+=("$crate: missing 'description' field")
    fi
    # Detect [lints] block followed by 'workspace = true'.
    if ! awk '
        /^\[lints\]/ { in_lints = 1; next }
        /^\[/ { in_lints = 0 }
        in_lints && /^workspace[[:space:]]*=[[:space:]]*true/ { found = 1 }
        END { exit found ? 0 : 1 }
    ' "$cargo_toml"; then
        errors+=("$crate: missing '[lints] workspace = true' block")
    fi
done

if (( ${#errors[@]} > 0 )); then
    echo "metadata-normalization gate: ${#errors[@]} violations." >&2
    for e in "${errors[@]}"; do
        echo "  $e" >&2
    done
    echo >&2
    echo "Fix: add inherited fields, description, and \`[lints] workspace = true\`" >&2
    echo "to the offending crate's Cargo.toml. Every workspace crate ships with" >&2
    echo "the same lint floor; cargo-fuzz nested workspaces are the only" >&2
    echo "documented exception (vyre-foundation/fuzz)." >&2
    exit 1
fi

echo "metadata-normalization gate: ${#MEMBERS[@]} workspace crates normalized."
exit 0
