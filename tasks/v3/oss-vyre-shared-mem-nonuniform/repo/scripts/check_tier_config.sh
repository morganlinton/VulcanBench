#!/usr/bin/env bash
# P0 inventory #75  -  Tier A / Tier B configurability gate.
#
# Reads `contracts/tier_config_manifest.toml` and verifies every
# tool-facing crate declares both tiers and that the referenced files
# exist on disk.
#
# Doctrine:
#   Tier A  -  operational knobs (CLI, env, TOML defaults).
#   Tier B  -  community knowledge (TOML data files, never CLI).
#
# A tool-facing crate without a Tier B layer is a maintainability bug  - 
# it forces every detection / fingerprint / signature change to ride a
# Rust patch and a release. The gate enforces both tiers so that
# discipline doesn't drift.
#
# Library crates declare `kind = "library"` and are excluded from the
# tier requirement; the gate still verifies they appear in the manifest
# (so adding a new crate without classifying it fails CI).
#
# Usage:
#   scripts/check_tier_config.sh

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

MANIFEST="contracts/tier_config_manifest.toml"

if [[ ! -f "$MANIFEST" ]]; then
    echo "tier-config gate: $MANIFEST missing." >&2
    exit 1
fi

errors=()

# Parse the manifest with awk: emit name|kind|tier_a|tier_b for each [[crate]] block.
parsed=$(awk '
    /^\[\[crate\]\]/ { if (name != "") emit(); name = ""; kind = ""; tier_a = ""; tier_b = ""; next }
    /^name = / { name = $0; sub(/^name = "/, "", name); sub(/"$/, "", name) }
    /^kind = / { kind = $0; sub(/^kind = "/, "", kind); sub(/"$/, "", kind) }
    /^tier_a = / { tier_a = $0; sub(/^tier_a = "/, "", tier_a); sub(/"$/, "", tier_a) }
    /^tier_b = / { tier_b = $0; sub(/^tier_b = "/, "", tier_b); sub(/"$/, "", tier_b) }
    END { if (name != "") emit() }
    function emit() {
        print name "|" kind "|" tier_a "|" tier_b
    }
' "$MANIFEST")

# Workspace members the manifest must cover. Kept in sync with
# `[workspace] members` in Cargo.toml; new members must appear in the
# manifest with an explicit kind.
EXPECTED_MEMBERS=(
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
    "vyre-runtime"
    "vyre-libs"
    "vyre-intrinsics"
    "vyre-aot"
    "vyre-harness"
    "xtask"
    "vyre-conform-spec"
    "vyre-conform-generate"
    "vyre-conform-enforce"
    "vyre-conform-runner"
    "vyre-test-harness"
)

declare -A SEEN

while IFS= read -r row; do
    [[ -z "$row" ]] && continue
    IFS='|' read -r name kind tier_a tier_b <<< "$row"
    SEEN[$name]=1
    case "$kind" in
        library)
            # Libraries must NOT declare tier_a or tier_b  -  those are tool concerns.
            if [[ -n "$tier_a" || -n "$tier_b" ]]; then
                errors+=("$name: kind=library but declares tier_a/tier_b (libraries should not have tiers)")
            fi
            ;;
        tool|internal-tool)
            if [[ -z "$tier_a" ]]; then
                errors+=("$name: kind=$kind missing tier_a entry")
            elif [[ ! -f "$tier_a" ]]; then
                errors+=("$name: tier_a points at missing file '$tier_a'")
            fi
            if [[ -z "$tier_b" ]]; then
                errors+=("$name: kind=$kind missing tier_b entry")
            elif [[ ! -d "$tier_b" ]]; then
                errors+=("$name: tier_b points at missing directory '$tier_b'")
            fi
            ;;
        *)
            errors+=("$name: unknown kind '$kind' (must be tool, internal-tool, or library)")
            ;;
    esac
done <<< "$parsed"

# Every expected workspace member must appear in the manifest.
for m in "${EXPECTED_MEMBERS[@]}"; do
    if [[ -z "${SEEN[$m]:-}" ]]; then
        errors+=("$m: workspace member not classified in $MANIFEST")
    fi
done

if (( ${#errors[@]} > 0 )); then
    echo "tier-config gate: ${#errors[@]} violations." >&2
    for e in "${errors[@]}"; do
        echo "  $e" >&2
    done
    echo >&2
    echo "Fix: classify the crate in $MANIFEST as kind=library or" >&2
    echo "kind=tool / kind=internal-tool, and write the missing CONFIG.md /" >&2
    echo "rules dir. New tool-facing crates ship with both tiers from day one." >&2
    exit 1
fi

echo "tier-config gate: ${#EXPECTED_MEMBERS[@]} workspace members classified."
exit 0
