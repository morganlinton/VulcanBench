#!/usr/bin/env bash
# Architectural invariant guard for vyre.
#
# Vyre's design contract (see THESIS.md) requires core to own nothing but the
# graph structure and the trait contracts. The moment vyre-core,
# vyre-foundation, vyre-primitives, or vyre-reference declares a real dependency on a backend
# crate or on vyre-conform, the substrate-neutral claim collapses: the
# "abstract" IR would require a specific backend to compile. This script
# enforces the contract at CI time and fails any PR that violates it.
#
# Dev-dependencies are allowed  -  tests that exercise cross-crate integration
# are still a legitimate part of the workspace. What is forbidden is a
# NON-dev dependency edge: anything that would force a downstream consumer
# of the core crates to pull a backend or the conformance harness.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

# Crates that MUST stay substrate-neutral. A violation in any of these is
# a rebuild regression, not a style nit.
PURE_CRATES=(
  "vyre-core"
  "vyre-foundation"
  "vyre-primitives"
  "vyre-reference"
  "vyre-spec"
  "vyre-driver"
)

# Crates that the pure crates must never depend on (outside dev-dependencies).
FORBIDDEN_DEPS=(
  "vyre-driver-wgpu"
  "vyre-driver-cuda"
  "vyre-driver-spirv"
  "vyre-runtime"
  "vyre-aot"
  "vyre-frontend-c"
  "wgpu"
  "naga"
)

violations=0

for crate in "${PURE_CRATES[@]}"; do
  manifest="$REPO_ROOT/$crate/Cargo.toml"
  if [[ ! -f "$manifest" ]]; then
    # Crate may not exist yet in the rebuild; skip silently. When it lands,
    # this guard starts enforcing.
    continue
  fi

  # Extract the [dependencies] and [build-dependencies] sections only.
  # [dev-dependencies] are intentionally permitted.
  pure_deps="$(awk '
    /^\[dependencies\]/          { inside=1; next }
    /^\[build-dependencies\]/    { inside=1; next }
    /^\[dev-dependencies\]/      { inside=0; next }
    /^\[target\.[^]]+\.dev-dependencies\]/ { inside=0; next }
    /^\[target\.[^]]+\.dependencies\]/     { inside=1; next }
    /^\[target\.[^]]+\.build-dependencies\]/ { inside=1; next }
    /^\[/                        { inside=0; next }
    inside && NF > 0             { print }
  ' "$manifest" | grep -v 'optional[[:space:]]*=[[:space:]]*true')"

  for forbidden in "${FORBIDDEN_DEPS[@]}"; do
    if echo "$pure_deps" | grep -qE "^[[:space:]]*\"?${forbidden}\"?[[:space:]]*="; then
      echo "ARCH VIOLATION: $crate depends on $forbidden outside [dev-dependencies]." >&2
      echo "  Manifest: $manifest" >&2
      echo "  Pure crates must stay substrate-neutral per THESIS.md." >&2
      echo "  Fix: move the dependency under [dev-dependencies] or, if the" >&2
      echo "  usage is non-test, relocate the code to a downstream crate." >&2
      violations=$((violations + 1))
    fi
  done
done

# A second invariant: backend crates must not depend on non-existent legacy
# crate names. Stale references make this gate pass or fail for the wrong
# architecture.
if rg -n '^[[:space:]]*"?vyre-(ir|wgpu)"?[[:space:]]*=' --glob Cargo.toml "$REPO_ROOT" >/tmp/vyre_arch_legacy_hits.$$ 2>/dev/null; then
  echo "ARCH VIOLATION: stale legacy crate names in manifests:" >&2
  cat /tmp/vyre_arch_legacy_hits.$$ >&2
  rm -f /tmp/vyre_arch_legacy_hits.$$
  violations=$((violations + 1))
else
  rm -f /tmp/vyre_arch_legacy_hits.$$
fi

if [[ "$violations" -gt 0 ]]; then
  echo "" >&2
  echo "Architectural invariants failed: $violations violation(s)." >&2
  echo "See THESIS.md for the substrate-neutrality contract." >&2
  exit 1
fi

echo "Architectural invariants: all $(printf '%s\n' "${PURE_CRATES[@]}" | wc -l | tr -d ' ') substrate-neutral crates clean."
