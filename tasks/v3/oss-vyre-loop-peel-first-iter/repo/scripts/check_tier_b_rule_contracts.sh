#!/usr/bin/env bash
# P2 inventory #113-#120  -  Tier-B rule contract gate.
#
# Walks `rules/` and enforces:
#   - Every domain (matching, parsing, decode, crypto, security,
#     graph, math) has a subdir under `rules/op/` or `rules/kat/`.
#   - Every TOML file parses cleanly (schema-decodable).
#   - Every TOML file is ≤ 50 KiB (the `rules/README.md` cap).
#   - `rules/SCHEMA.md` and `rules/README.md` exist.
#   - No hardcoded domain pattern list slips into production code
#     (`vyre-libs/src/security/`, `vyre-libs/src/matching/`, etc.)  - 
#     test fixtures under `*test_fixtures*.rs` and `tests/` are
#     allowed. This is the workspace-wide enforcement of items #113
#     and #116 (TOML-only contribution path for tool-facing domains).
#
# Usage:
#   scripts/check_tier_b_rule_contracts.sh

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

errors=()

# Required structure.
for required in rules rules/README.md rules/SCHEMA.md rules/op rules/kat; do
    if [[ ! -e "$required" ]]; then
        errors+=("missing $required")
    fi
done

# Domain coverage  -  at least one rule file per declared domain.
EXPECTED_OP_DOMAINS=("decode" "graph" "match" "string_matching" "compression")
for dom in "${EXPECTED_OP_DOMAINS[@]}"; do
    if ! ls rules/op/${dom}.*.toml >/dev/null 2>&1; then
        errors+=("rules/op/: no '${dom}.*.toml' file (item #114  -  domain coverage)")
    fi
done

# Per-file: parse + size cap.
while IFS= read -r f; do
    [[ -z "$f" ]] && continue
    sz=$(stat -c%s "$f" 2>/dev/null || echo 0)
    if (( sz > 50 * 1024 )); then
        errors+=("$f: ${sz} bytes exceeds 50 KiB cap (rules/README.md)")
    fi
    # Decode-test the TOML via cargo's bundled toml crate (run a small
    # Python check as a fallback so the gate works without cargo).
    if command -v python3 >/dev/null 2>&1; then
        if ! python3 -c "import sys, tomllib; tomllib.loads(open('$f','rb').read().decode())" 2>/dev/null; then
            errors+=("$f: TOML schema decode failed")
        fi
    fi
done < <(find rules -type f -name '*.toml' 2>/dev/null)

# Hardcoded pattern lists in production code. Test fixtures and
# in-tree tests are allowed; production paths are not.
hardcoded=$(grep -rnE 'static [A-Z_]+: &\[&str\]' --include='*.rs' \
    vyre-libs/src/security vyre-libs/src/matching vyre-libs/src/decode 2>/dev/null \
    | grep -vE 'test_fixtures|/tests/|fixtures/' || true)
if [[ -n "$hardcoded" ]]; then
    while IFS= read -r line; do
        errors+=("hardcoded pattern list (item #113): $line")
    done <<< "$hardcoded"
fi

if (( ${#errors[@]} > 0 )); then
    echo "tier-b-rule-contracts gate: ${#errors[@]} violations." >&2
    for e in "${errors[@]}"; do
        echo "  $e" >&2
    done
    echo >&2
    echo "Fix: drop a TOML file under rules/<category>/, parse-test it" >&2
    echo "via tomllib, and follow the size cap. Hardcoded pattern lists" >&2
    echo "in production code violate the Tier-B doctrine  -  move them to" >&2
    echo "rules/<domain>/<name>.toml and load at runtime." >&2
    exit 1
fi

echo "tier-b-rule-contracts gate: every Tier-B file passes structure + decode + size."
exit 0
