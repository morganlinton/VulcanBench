#!/usr/bin/env bash
#
# Run every Law / layout check in order.
#
# Aggregates the per-law scripts under `scripts/laws/` and runs
# them. In informational mode (default), violations are printed
# and the script still exits 0 so CI surfaces them without
# breaking the build. Strict mode (`VYRE_LAW_STRICT=1`) forwards
# the flag and fails the build on any violation.

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

LAWS_DIR="scripts/laws"
SCRIPTS=(
  check_file_sizes.sh
  check_mod_rs_size.sh
  check_prelude_size.sh
  check_layout.sh
  check_readmes.sh
)

fail_count=0
for script in "${SCRIPTS[@]}"; do
  path="$LAWS_DIR/$script"
  if [[ ! -x "$path" ]]; then
    echo "SKIP: $path not executable"
    continue
  fi
  echo "=== $script ==="
  if ! "$path"; then
    fail_count=$((fail_count + 1))
  fi
  echo
done

if [[ "$fail_count" -gt 0 ]]; then
  echo "aggregate: $fail_count law(s) reported violations"
  if [[ "${VYRE_LAW_STRICT:-0}" == "1" ]]; then
    exit 1
  fi
fi

echo "aggregate: done"
