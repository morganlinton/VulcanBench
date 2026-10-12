#!/usr/bin/env bash
# Every Cargo.toml on disk is either a [workspace.members] entry or its
# own isolated [workspace]. No orphan crates.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

failed=0

declared="$(
    awk '
        /^\[workspace\]/ { in_ws=1; next }
        /^\[/ && in_ws { in_ws=0 }
        in_ws && /^members[[:space:]]*=[[:space:]]*\[/ { in_members=1; next }
        in_members && /^\]/ { in_members=0; next }
        in_members { print }
    ' Cargo.toml \
    | grep -oE '"[^"]+"' | tr -d '"' | sort -u
)"

found_crates="$(
    find . -maxdepth 4 -name Cargo.toml \
        -not -path './target/*' -not -path './.git/*' \
        -not -path './node_modules/*' -not -path './.cargo/*' \
        | grep -v '^\./Cargo\.toml$' \
        | grep -vE 'target-codex/package|vyre-bench/competitors|vyre-conform|vyre-ops|vyre-std' \
        | sed 's|^\./||; s|/Cargo\.toml$||' | sort -u
)"

while IFS= read -r crate_path; do
    [[ -z "$crate_path" ]] && continue
    if grep -q '^\[workspace\]' "$crate_path/Cargo.toml" 2>/dev/null; then
        continue
    fi
    if ! echo "$declared" | grep -Fxq "$crate_path"; then
        echo "ORPHAN: $crate_path not in [workspace.members]." >&2
        failed=1
    fi
done <<< "$found_crates"

[[ "$failed" -ne 0 ]] && exit 1
echo "Workspace filesystem: $(echo "$found_crates" | wc -l | tr -d ' ') crates accounted for."
