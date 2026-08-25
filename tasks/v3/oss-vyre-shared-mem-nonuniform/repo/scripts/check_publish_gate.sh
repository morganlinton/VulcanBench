#!/usr/bin/env bash
# check_publish_gate.sh  -  verify a vyre-* crate is publishable per
# the contract in docs/PUBLISH_GATE.md.
#
# Usage:
#   check_publish_gate.sh <crate-name>
#
# Exit 0 = publishable. Nonzero = number of failed gates.

set -uo pipefail

CRATE="${1:-}"
if [[ -z "$CRATE" ]]; then
    echo "FAIL: usage: check_publish_gate.sh <crate-name>" >&2
    exit 64
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CRATE_DIR="$ROOT/$CRATE"
if [[ ! -d "$CRATE_DIR" ]]; then
    echo "FAIL: crate dir $CRATE_DIR does not exist" >&2
    exit 65
fi

FAILURES=0

# Gate 1: SPEC.md present at crate root.
if [[ ! -f "$CRATE_DIR/SPEC.md" ]]; then
    echo "FAIL: $CRATE missing SPEC.md at crate root" >&2
    FAILURES=$((FAILURES + 1))
fi

# Gate 2: every pub fn doc-commented (warn, not fatal in this gate
# since the crate-wide deny lints catch it during cargo_full doc).
# We just check the deny is present in lib.rs.
LIB_RS="$CRATE_DIR/src/lib.rs"
if [[ -f "$LIB_RS" ]] && ! grep -q 'missing_docs' "$LIB_RS"; then
    echo "WARN: $CRATE lib.rs does not deny(missing_docs)" >&2
fi

# Gate 3: no Program::new in production code.
if find "$CRATE_DIR/src" -name '*.rs' -not -path '*/tests/*' -print0 2>/dev/null \
    | xargs -0 grep -l 'Program::new(' >/dev/null 2>&1; then
    echo "FAIL: $CRATE production code uses Program::new  -  must use Program::wrapped" >&2
    FAILURES=$((FAILURES + 1))
fi

# Gate 4: per-primitive contract on dataflow + security + bitset
# + graph primitives.
PRIM_GATE="$ROOT/scripts/check_primitive_contract.sh"
if [[ -x "$PRIM_GATE" ]]; then
    case "$CRATE" in
        vyre-libs|vyre-primitives)
            if ! "$PRIM_GATE" >/dev/null 2>&1; then
                echo "FAIL: $CRATE has primitives violating SKILL_BUILD_DATAFLOW_PRIMITIVE.md" >&2
                FAILURES=$((FAILURES + 1))
            fi
            ;;
    esac
fi

# Gate 5: CHANGELOG entry for the current version.
CHANGELOG="$CRATE_DIR/CHANGELOG.md"
if [[ -f "$CHANGELOG" ]]; then
    VERSION=$(grep '^version =' "$CRATE_DIR/Cargo.toml" | head -1 | sed -E 's/.*"([^"]+)".*/\1/')
    if ! grep -q "$VERSION" "$CHANGELOG"; then
        echo "FAIL: $CRATE CHANGELOG.md has no entry for version $VERSION" >&2
        FAILURES=$((FAILURES + 1))
    fi
else
    echo "WARN: $CRATE missing CHANGELOG.md" >&2
fi

# Gate 6: cargo_full publish --dry-run.
# Skipped here  -  runs in CI separately to keep this gate fast.
echo "INFO: cargo_full publish --dry-run -p $CRATE  -  run separately in CI"

if [[ "$FAILURES" -gt 0 ]]; then
    echo
    echo "FAIL: $CRATE has $FAILURES publish-gate violations" >&2
    exit "$FAILURES"
fi
echo "OK: $CRATE satisfies the publish gate"
exit 0
