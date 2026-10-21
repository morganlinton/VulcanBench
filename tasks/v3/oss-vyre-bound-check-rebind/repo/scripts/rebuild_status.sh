#!/usr/bin/env bash
# One-shot rebuild status. Runs every architectural guard, prints a
# one-line-per-law summary, and surfaces overall cargo health.
#
# Intended for quick eyeballing during the sweep  -  "are we converging?"

set -u

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"
source scripts/lib/cargo_runner.sh
vyre_select_cargo_runner

color_ok="\033[32m"
color_fail="\033[31m"
color_warn="\033[33m"
color_reset="\033[0m"

count_violations() {
  local script="$1"
  local pattern="$2"
  bash "$script" 2>&1 | grep -c "^$pattern" || true
}

print_law() {
  local name="$1"
  local count="$2"
  local description="$3"
  if [[ "$count" -eq 0 ]]; then
    printf "  ${color_ok}%-35s PASS${color_reset}  %s\n" "$name" "$description"
  else
    printf "  ${color_fail}%-35s %d violations${color_reset}  %s\n" "$name" "$count" "$description"
  fi
}

echo "Architectural laws"
echo "=================="

arch="$(count_violations scripts/check_architectural_invariants.sh 'ARCH VIOLATION')"
print_law "pure-crate dependency invariant" "$arch" "core owns nothing but graph + traits"

law_a="$(count_violations scripts/check_no_closed_ir_enums.sh 'LAW A VIOLATION')"
print_law "Law A  -  no closed IR enums" "$law_a" "open traits only, tagged-union hybrid allowed"

law_b="$(count_violations scripts/check_no_string_wgsl.sh 'LAW B VIOLATION')"
print_law "Law B  -  no string WGSL" "$law_b" "naga AST pipeline only"

law_c="$(count_violations scripts/check_capability_negotiation.sh 'LAW C VIOLATION')"
print_law "Law C  -  capability negotiation" "$law_c" "supported_ops + validate_program"

law_d="$(count_violations scripts/check_registry_consistency.sh 'LAW D VIOLATION')"
print_law "Law D  -  registry consistency" "$law_d" "NodeKindRegistration ↔ supported_ops symmetry"

law_h="$(count_violations scripts/check_unsafe_justifications.sh 'LAW H VIOLATION')"
print_law "Law H  -  unsafe SAFETY comments" "$law_h" "every unsafe block justified"

echo
echo "Cargo health"
echo "============"

cargo_output="$("$CARGO_RUNNER" check --workspace 2>&1)"
cargo_errors="$(echo "$cargo_output" | grep -c '^error' || true)"

if [[ "$cargo_errors" -eq 0 ]]; then
  printf "  ${color_ok}cargo_full check --workspace   PASS${color_reset}\n"
else
  printf "  ${color_fail}cargo_full check --workspace   %d errors${color_reset}\n" "$cargo_errors"
  echo "  First 3:"
  echo "$cargo_output" | grep -A1 '^error\[' | head -9 | sed 's/^/    /'
fi

echo
echo "Audit files landed: $(ls docs/audits/AUDIT_2026-04-18_*.md 2>/dev/null | wc -l | tr -d ' ')"
echo "Tasks closed since rebuild start: $(git log --since='1 hour ago' --oneline | grep -cE 'BUILD|FIX|CUT|RIP|DO-' || true)"
