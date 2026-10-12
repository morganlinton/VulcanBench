#!/usr/bin/env bash
# Inventory P1 #109 (IR ↔ wire sync) / #39 (field coverage):
# Sentinel that every `Program` field that participates in wire I/O is represented
# in the encode/decode sources. Expand the field list when `Program` or the VIR0
# envelope gains new serialized state.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CORE="${ROOT}/vyre-foundation/src/ir_inner/model/program/core.rs"
ENC="${ROOT}/vyre-foundation/src/serial/wire/encode/to_wire.rs"
DEC="${ROOT}/vyre-foundation/src/serial/wire/decode/from_wire.rs"

for path in "$CORE" "$ENC" "$DEC"; do
  if [[ ! -f "$path" ]]; then
    echo "check_ir_wire_field_sync: missing $path" >&2
    exit 1
  fi
done

serialized_fields=(
  entry_op_id
  buffers
  workgroup_size
  entry
  non_composable_with_self
)

transient_fields=(
  buffer_index
  hash
  validation_set
  structural_validated
  fingerprint
  output_buffer_index
  has_indirect_dispatch
  stats
)

for field in "${serialized_fields[@]}"; do
  grep -q "$field" "$CORE" || {
    echo "check_ir_wire_field_sync: expected Program field '$field' in $CORE" >&2
    exit 1
  }
done

# Each serialized concept must appear in encode or decode (comments count for docs-only strings).
for field in "${serialized_fields[@]}"; do
  if ! grep -q "$field" "$ENC" && ! grep -q "$field" "$DEC"; then
    echo "check_ir_wire_field_sync: '$field' not mentioned in encode or decode ($ENC / $DEC)" >&2
    exit 1
  fi
done

program_fields="$(
  perl -ne '
    $in = 1, next if /pub struct Program\s*\{/;
    exit if $in && /^}/;
    print "$1\n" if $in && /^\s*pub(?:\(crate\))?\s+([A-Za-z_][A-Za-z0-9_]*)\s*:/;
  ' "$CORE"
)"

field_is_known() {
  local candidate="$1"
  local known
  for known in "${serialized_fields[@]}" "${transient_fields[@]}"; do
    [[ "$candidate" == "$known" ]] && return 0
  done
  return 1
}

while IFS= read -r field; do
  [[ -z "$field" ]] && continue
  if ! field_is_known "$field"; then
    echo "check_ir_wire_field_sync: Program field '$field' is neither serialized nor explicitly transient." >&2
    echo "Fix: wire the field through encode/decode in the same patch, or add it to transient_fields with a cache-only rationale." >&2
    exit 1
  fi
done <<< "$program_fields"

echo "check_ir_wire_field_sync: Program wire sync sentinels OK (inventories #39 / #109)."
