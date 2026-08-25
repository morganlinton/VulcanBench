#!/usr/bin/env bash
# Law: inventory::iter is forbidden on the dispatch hot path.
#
# Inventory registrations are link-time metadata. Consuming them per
# dispatch means walking a linked list of static items, which blows the
# hot path's allocation/cache invariants. Every registry has a
# frozen-after-init `OnceLock<FrozenIndex>` that serves lookups in
# sub-ns. If this script fails, the hot path just regressed.
#
# See docs/inventory-contract.md §"Hot-path prohibition".

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

# Files that may contain inventory::iter are init-only code paths.
# Everything else is hot path.
forbidden_paths=(
    "vyre-driver/src/backend"
    "vyre-driver/src/pipeline.rs"
    "vyre-driver-wgpu/src/async_dispatch.rs"
    "vyre-driver-wgpu/src/engine"
    "vyre-driver-wgpu/src/lib.rs"
    "vyre-driver-wgpu/src/pipeline.rs"
    "vyre-driver-wgpu/src/pipeline_binding.rs"
    "vyre-driver-wgpu/src/pipeline_compound.rs"
    "vyre-driver-wgpu/src/pipeline_disk_cache.rs"
    "vyre-driver-wgpu/src/pipeline_persistent.rs"
    "vyre-driver-wgpu/src/runtime"
    "vyre-driver-cuda/src"
    "vyre-driver-spirv/src"
    "vyre-runtime/src"
)

# Files that are LEGITIMATELY init-only, exempt from the hot-path ban.
# Each must document in-file why inventory::iter is acceptable.
allowlist_regex='vyre-driver/src/registry/(registry|migration)\.rs|vyre-driver/src/backend/(dialect_supported_ops|registry|registry/inventory_streams|registry/acquire)\.rs|vyre-foundation/src/optimizer\.rs'

# Match the real call syntax only  -  `inventory::iter::<T>`  -  and skip lines
# that start with `//` (doc comments and explanatory prose reference the
# symbol legitimately).
needle='^\s*[^/]*inventory::iter::<'
exit_code=0

for path in "${forbidden_paths[@]}"; do
    if [ ! -e "$path" ]; then
        continue
    fi
    hits=$(rg -n --hidden -g '!target' -P "$needle" "$path" 2>/dev/null || true)
    if [ -n "$hits" ]; then
        while IFS= read -r line; do
            [ -z "$line" ] && continue
            if grep -qE "$allowlist_regex" <<< "$line"; then
                continue
            fi
            echo "Hot-path inventory::iter detected in $path:" >&2
            echo "$line" >&2
            echo "" >&2
            echo "Fix: route the lookup through the registry's frozen OnceLock." >&2
            echo "If this site is init-only, add it to the allowlist in this script" >&2
            echo "AND document the invariant in a nearby // HOT-PATH-OK: comment." >&2
            exit_code=1
        done <<< "$hits"
    fi
done

exit "$exit_code"
