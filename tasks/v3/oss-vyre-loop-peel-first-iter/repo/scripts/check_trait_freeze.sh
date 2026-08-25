#!/usr/bin/env bash
# Frozen-contract snapshot check.
#
# vyre freezes seven contracts per ARCHITECTURE.md. For each, extract
# the declaration block and diff against docs/frozen-traits/<name>.txt.
# Any diff means the frozen contract changed  -  a semver-major event.
#
# Usage:
#   scripts/check_trait_freeze.sh                      # check (CI)
#   scripts/check_trait_freeze.sh --refresh-snapshots  # regenerate (local)

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

SNAPSHOT_DIR="docs/frozen-traits"
mkdir -p "$SNAPSHOT_DIR"

CONTRACTS_NAME=(VyreBackend ExprVisitor Lowerable AlgebraicLaw EnforceGate MutationClass PassBoundaryClass)
CONTRACTS_FILE=(
  "vyre-driver/src/backend/vyre_backend.rs"
  "vyre-foundation/src/visit/expr.rs"
  "vyre-driver/src/backend/lowering.rs"
  "vyre-spec/src/algebraic_law.rs"
  "vyre-driver/src/registry/enforce.rs"
  "vyre-driver/src/registry/mutation.rs"
  "vyre-foundation/src/optimizer.rs"
)
CONTRACTS_KEYWORD=(
  "pub trait VyreBackend"
  "pub trait ExprVisitor"
  "pub trait LowerableOp"
  "pub enum AlgebraicLaw"
  "pub trait EnforceGate"
  "pub enum MutationClass"
  "pub enum PassBoundaryClass"
)

extract_block() {
    local file="$1"
    local keyword="$2"
    awk -v kw="$keyword" '
        BEGIN { depth = 0; inside = 0 }
        index($0, kw) && !inside { inside = 1 }
        inside {
            line = $0
            sub(/^[[:space:]]+/, "", line)
            if (line != "") print line
            for (i = 1; i <= length($0); i++) {
                ch = substr($0, i, 1)
                if (ch == "{") depth++
                else if (ch == "}") {
                    depth--
                    if (depth == 0) exit 0
                }
            }
        }
    ' "$file"
}

refresh_mode=0
if [[ "${1:-}" == "--refresh-snapshots" ]]; then refresh_mode=1; fi

failed=0
for idx in "${!CONTRACTS_NAME[@]}"; do
    name="${CONTRACTS_NAME[$idx]}"
    file="${CONTRACTS_FILE[$idx]}"
    keyword="${CONTRACTS_KEYWORD[$idx]}"
    snapshot="$SNAPSHOT_DIR/${name}.txt"

    if [[ ! -f "$file" ]]; then
        echo "Frozen contract source missing: ${name} expected at ${file}." >&2
        failed=1
        continue
    fi

    current="$(extract_block "$file" "$keyword")"
    if [[ -z "$current" ]]; then
        echo "Frozen contract not found in source: ${name} (keyword: ${keyword})." >&2
        failed=1
        continue
    fi

    if [[ "$refresh_mode" -eq 1 ]]; then
        printf '%s\n' "$current" > "$snapshot"
        echo "refreshed: $snapshot"
        continue
    fi

    if [[ ! -f "$snapshot" ]]; then
        echo "Missing snapshot for frozen contract ${name}: ${snapshot}. Fix: run scripts/check_trait_freeze.sh --refresh-snapshots and review before committing." >&2
        failed=1
        continue
    fi

    expected="$(cat "$snapshot")"
    if [[ "$current" != "$expected" ]]; then
        echo "FROZEN CONTRACT DRIFT: ${name} (${file})" >&2
        echo "Fix: if intentional, refresh via --refresh-snapshots AND bump the major version (semver-major event)." >&2
        diff <(echo "$expected") <(echo "$current") >&2 || true
        failed=1
    fi
done

if [[ "$failed" -ne 0 ]]; then exit 1; fi
if [[ "$refresh_mode" -eq 0 ]]; then
    echo "Frozen contracts: all ${#CONTRACTS_NAME[@]} byte-stable."
fi
