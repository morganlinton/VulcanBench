#!/usr/bin/env bash
# Gap #8  -  doctest coverage.
#
# Every `pub fn`, `pub struct`, `pub trait`, `pub const`, `pub enum`
# in vyre's published crates must carry a doctest in its docstring.
# Tokio is 95%+; vyre today is ~20%.
#
# Pass: zero public items without a doctest.
# Fail: list the items missing a doctest + exit non-zero.

set -euo pipefail
cd "$(dirname "$0")/.."

CRATES=(vyre-core vyre-foundation vyre-driver vyre-driver-wgpu)
MISSING=0

# Find every `pub (fn|struct|trait|enum|const)` definition in each
# crate's src/. For each, check that the immediately-preceding doc
# comments contain a ``` block.
#
# This is a structural heuristic; the only false positives are
# items with an explicit `#[doc(hidden)]` which we skip.

for crate in "${CRATES[@]}"; do
    [ -d "$crate/src" ] || continue
    while IFS= read -r file; do
        awk -v file="$file" '
            /^\s*#\[doc\(hidden\)\]/ { hidden = 1 }
            /^\s*\/\/\//            { doc = doc "\n" $0 }
            /^\s*pub\s+(fn|struct|trait|enum|const)\s/ {
                if (!hidden) {
                    if (doc !~ /```/) {
                        print file ":" NR ": " $0
                    }
                }
                doc = ""; hidden = 0
                next
            }
            /^\s*$/ || /^\s*\/\// { next }
            { doc = ""; hidden = 0 }
        ' "$file"
    done < <(find "$crate/src" -name '*.rs')
done | tee /tmp/vyre-doctest-missing.txt

count=$(wc -l < /tmp/vyre-doctest-missing.txt)
if [ "$count" -gt 0 ]; then
    echo "gap #8: $count public items missing a doctest (see /tmp/vyre-doctest-missing.txt)" >&2
    exit 1
fi
