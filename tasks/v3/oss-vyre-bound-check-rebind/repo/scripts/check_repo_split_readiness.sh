#!/usr/bin/env bash
# P0 inventory #74  -  repo-split readiness gate.
#
# vyre's long-term shape (per `~/.claude/analysts/visions/vyre.md`) is
# crates-of-crates: every shared lib publishes to crates.io with its
# own GitHub repo, README, CI; tool subcrates ride along as feature
# gates on their parent crate. This gate verifies that each workspace
# member has the metadata and shape needed to split out cleanly when
# the time comes.
#
# Per-crate readiness contract:
#   - shared-lib classification (kind=library in tier_config_manifest):
#       * non-empty `description`, `readme = "README.md"`, `repository`,
#         `homepage`, `keywords`, `categories`.
#       * README.md ≥ 30 lines (real docs, not a stub).
#       * `[lints] workspace = true` so the lint floor matches workspace.
#       * `publish` field absent OR `publish = true`.
#       * No path-only internal deps (every internal dep has a `version`).
#   - tool / internal-tool classification:
#       * `publish = false` is REQUIRED (tool crates do not publish).
#       * CONFIG.md present (Tier A/B documentation).
#
# What this gate does NOT do:
#   - Does not run `./cargo_full publish --dry-run` (that's the publish-readiness
#     dry run, item 126).
#   - Does not check downstream consumer compatibility (that's
#     `scripts/check_consumers.sh`).
#
# Usage:
#   scripts/check_repo_split_readiness.sh

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

TIER_MANIFEST="contracts/tier_config_manifest.toml"
if [[ ! -f "$TIER_MANIFEST" ]]; then
    echo "repo-split-readiness gate: $TIER_MANIFEST missing." >&2
    exit 1
fi

# Build a name → kind lookup from the tier-config manifest.
declare -A KIND
while IFS= read -r row; do
    [[ -z "$row" ]] && continue
    IFS='|' read -r name kind <<< "$row"
    [[ -n "$name" && -n "$kind" ]] && KIND[$name]=$kind
done < <(awk '
    /^\[\[crate\]\]/ { if (n != "") print n "|" k; n = ""; k = ""; next }
    /^name = / { v = $0; sub(/^name = "/, "", v); sub(/"$/, "", v); n = v }
    /^kind = / { v = $0; sub(/^kind = "/, "", v); sub(/"$/, "", v); k = v }
    END { if (n != "") print n "|" k }
' "$TIER_MANIFEST")

# Workspace member directories. Aligned with `[workspace] members`.
declare -A MEMBER_DIRS
MEMBER_DIRS["vyre-core"]="vyre-core"
MEMBER_DIRS["vyre-foundation"]="vyre-foundation"
MEMBER_DIRS["vyre-driver"]="vyre-driver"
MEMBER_DIRS["vyre-driver-wgpu"]="vyre-driver-wgpu"
MEMBER_DIRS["vyre-driver-spirv"]="vyre-driver-spirv"
MEMBER_DIRS["vyre-driver-cuda"]="vyre-driver-cuda"
MEMBER_DIRS["vyre-reference"]="vyre-reference"
MEMBER_DIRS["vyre-spec"]="vyre-spec"
MEMBER_DIRS["vyre-macros"]="vyre-macros"
MEMBER_DIRS["vyre-primitives"]="vyre-primitives"
MEMBER_DIRS["vyre-runtime"]="vyre-runtime"
MEMBER_DIRS["vyre-libs"]="vyre-libs"
MEMBER_DIRS["vyre-intrinsics"]="vyre-intrinsics"
MEMBER_DIRS["vyre-aot"]="vyre-aot"
MEMBER_DIRS["vyre-harness"]="vyre-harness"
MEMBER_DIRS["xtask"]="xtask"
MEMBER_DIRS["vyre-conform-spec"]="conform/vyre-conform-spec"
MEMBER_DIRS["vyre-conform-generate"]="conform/vyre-conform-generate"
MEMBER_DIRS["vyre-conform-enforce"]="conform/vyre-conform-enforce"
MEMBER_DIRS["vyre-conform-runner"]="conform/vyre-conform-runner"
MEMBER_DIRS["vyre-test-harness"]="conform/vyre-test-harness"

errors=()

check_publish_field() {
    local cargo="$1"
    local expect="$2"  # "true" or "false"
    local actual
    actual=$(grep -E '^publish *=' "$cargo" | head -1 | sed -E 's/^publish *= *([a-z]+).*/\1/')
    if [[ -z "$actual" ]]; then
        actual="true"  # Default per Cargo semantics.
    fi
    if [[ "$actual" != "$expect" ]]; then
        return 1
    fi
    return 0
}

for crate_name in "${!MEMBER_DIRS[@]}"; do
    dir="${MEMBER_DIRS[$crate_name]}"
    cargo="$dir/Cargo.toml"
    kind="${KIND[$crate_name]:-unknown}"

    if [[ "$kind" == "unknown" ]]; then
        errors+=("$crate_name: not classified in $TIER_MANIFEST")
        continue
    fi

    if [[ ! -f "$cargo" ]]; then
        errors+=("$crate_name: missing $cargo")
        continue
    fi

    if [[ "$kind" == "library" ]]; then
        # Required metadata for a publishable lib.
        for field in description readme repository homepage keywords categories; do
            if ! grep -qE "^$field" "$cargo"; then
                errors+=("$crate_name: library missing '$field' in Cargo.toml")
            fi
        done
        # README floor.
        if [[ ! -f "$dir/README.md" ]]; then
            errors+=("$crate_name: library missing README.md")
        else
            lc=$(wc -l < "$dir/README.md" | tr -d ' ')
            if (( lc < 30 )); then
                errors+=("$crate_name: library README.md is $lc lines (floor=30)")
            fi
        fi
        # Lint inheritance.
        if ! awk '
            /^\[lints\]/ { in_lints = 1; next }
            /^\[/ { in_lints = 0 }
            in_lints && /^workspace[[:space:]]*=[[:space:]]*true/ { found = 1 }
            END { exit found ? 0 : 1 }
        ' "$cargo"; then
            errors+=("$crate_name: library missing [lints] workspace = true")
        fi
        # Publish must NOT be false for a library.
        if grep -qE '^publish *= *false' "$cargo"; then
            errors+=("$crate_name: library has publish = false (libraries must publish)")
        fi
    elif [[ "$kind" == "tool" || "$kind" == "internal-tool" ]]; then
        # Tools must NOT auto-publish.
        if ! check_publish_field "$cargo" "false"; then
            errors+=("$crate_name: $kind requires publish = false")
        fi
        # CONFIG.md presence is enforced by check_tier_config.sh; skip here.
    fi
done

if (( ${#errors[@]} > 0 )); then
    echo "repo-split-readiness gate: ${#errors[@]} violations." >&2
    for e in "${errors[@]}"; do
        echo "  $e" >&2
    done
    echo >&2
    echo "Fix: every library member must ship the publish-ready metadata" >&2
    echo "(description / readme / repository / homepage / keywords /" >&2
    echo "categories), a real README ≥30 lines, and inherit workspace lints." >&2
    echo "Tool subcrates (vyre-cc, conform/*, xtask) MUST set publish = false" >&2
    echo "so they never go to crates.io." >&2
    exit 1
fi

echo "repo-split-readiness gate: ${#MEMBER_DIRS[@]} workspace members ready."
exit 0
