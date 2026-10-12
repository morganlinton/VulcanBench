#!/usr/bin/env bash
# P1 inventory #108 / architecture #63 alignment  -  mega-crate `vyre-libs` default pull.
#
# The `vyre-libs` aggregate currently enables NN, matcher, decoder, crypto, and math tiers by
# default. That defeats compile-time budgeting for downstream dependents. Inventory target is:
# tighten defaults (`default = []` or a single ergonomic umbrella feature) WITHOUT quietly
# bloating everyone's compile graph.
#
# Default mode: ratchet the number of feature strings enumerated in `[features]` `default`.
# Raising FEATURE_HIGHWATER requires explicit sign-off documented here  -  shrinking is always OK.
#
# `--strict`: require `FEATURE_STRICT_CEILING` tokens or fewer in `default`; today this FAILS until
# the megacrat split completes (violations noted in-repo with intent to fix inventory #62–65).

set -euo pipefail

STRICT=false
[[ "${1:-}" == "--strict" ]] && STRICT=true

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

MANIFEST="vyre-libs/Cargo.toml"

n=$(sed -n '/^default = \[/,/^\]/p' "$ROOT/$MANIFEST" | grep -oE '"[^"]+"' | wc -l | tr -d ' ')
FEATURE_HIGHWATER=14

echo "megacrat default-features: $n quoted slug(s) under [features].default on $MANIFEST (HIGHWATER=$FEATURE_HIGHWATER, strict=$STRICT)."

if [[ "$STRICT" == true ]]; then
  FEATURE_STRICT_CEILING=0
  if [[ "$n" -gt "$FEATURE_STRICT_CEILING" ]]; then
    echo "Strict policy: default feature count must stay <= $FEATURE_STRICT_CEILING until inventory §P0 #62–63 land." >&2
    echo "Fix: empty \`default\` slice or consolidate under one umbrella feature gated by dependents." >&2
    exit 1
  fi
  exit 0
fi

if [[ "$n" -gt "$FEATURE_HIGHWATER" ]]; then
  echo "(ratchet) Regression: $n quoted default features exceed FEATURE_HIGHWATER=$FEATURE_HIGHWATER." >&2
  echo "Fix: revert default expansion or raise FEATURE_HIGHWATER with architecture review reference." >&2
  exit 1
fi

exit 0
