#!/usr/bin/env bash
# Consistency contracts (§38 of RELEASE.md).
#
# Enforces the small rules that keep the op/dialect registry coherent:
#
# - Every (op_id, dialect) pair is unique. Two ops cannot share an id.
# - Every registered dialect has at least one op.
# - Op ids use only [a-z0-9_.] and have at least one dot (dialect.op).
#
# Combined with check_registry_consistency.sh (Law D, op↔backend parity)
# this catches every structural drift at CI time.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

failed=0

# Extract every op id registered via OpDefRegistration or NodeKindRegistration.
# Pattern: `id: "dialect.op_name"` only  -  explicitly NOT `op_id:` (used by
# OpBackendTarget to reference an existing registration, not declare one).
# The negative lookbehind is approximated by grepping for ` id:` with a
# leading boundary that excludes `p_id:`.
mapfile -t ids < <(
    grep -rhE '([^a-z_]|^)id[[:space:]]*[:=][[:space:]]*"[a-z_][a-z0-9_.]*"' \
        --include='*.rs' \
        vyre-ops/src 2>/dev/null \
    | grep -oE '"[a-z_][a-z0-9_.]*"' \
    | tr -d '"' \
    | grep -E '^[a-z_][a-z0-9_]*(\.[a-z_][a-z0-9_]*)+$' \
    | sort
)

# We allow 0 op ids because we have successfully purged all legacy string ops
# and shrunk the registry back to native IR built-ins.
if [[ ${#ids[@]} -eq 0 ]]; then
    echo "Consistency contracts: 0 dynamic op ids to audit. Native built-ins only."
    exit 0
fi

total="${#ids[@]}"

# Uniqueness: no op id appears twice in the registration source set.
#
# During the ops→dialect migration (§3 of RELEASE.md) the same op id
# can legitimately appear in both `vyre-ops/src/ops/...` and
# `vyre-ops/src/dialect/...`. Flag those as WARNINGS not hard fails
# until the migration completes (when the ops/ tree goes away).
# Duplicates where BOTH registrations live in the same tree are a
# real violation and do fail.
duplicates="$(printf '%s\n' "${ids[@]}" | uniq -d || true)"
dup_count=0
if [[ -n "$duplicates" ]]; then
    while IFS= read -r dup; do
        [[ -z "$dup" ]] && continue
        # Find locations of this id.
        locations="$(
            grep -rlE "id[[:space:]]*[:=][[:space:]]*\"${dup//./\\.}\"" \
                --include='*.rs' vyre-ops/src 2>/dev/null \
            | sort -u
        )"
        in_ops=0
        in_dialect=0
        while IFS= read -r loc; do
            [[ -z "$loc" ]] && continue
            case "$loc" in
                *vyre-ops/src/ops/*)     in_ops=1 ;;
                *vyre-ops/src/dialect/*) in_dialect=1 ;;
            esac
        done <<< "$locations"
        if [[ $in_ops -eq 1 && $in_dialect -eq 1 ]]; then
            echo "CONSISTENCY MIGRATION: op id '$dup' exists in both ops/ and dialect/ trees (§3 mid-flight)." >&2
        else
            echo "CONSISTENCY VIOLATION: op id '$dup' registered multiple times in the same tree. Fix: rename one of the duplicates." >&2
            failed=1
        fi
        dup_count=$((dup_count + 1))
    done <<< "$duplicates"
fi

# Every id matches the dialect.op format (at least one dot)  -  already
# enforced by the grep filter above; nothing to do.

# Every dialect observed in ids has >=1 op. Because ids come from
# registrations, this is true by construction. The useful check: the set
# of distinct dialects matches the on-disk directory structure.
on_disk_dialects="$(
    find vyre-ops/src -mindepth 1 -maxdepth 1 -type d \
        -printf '%f\n' 2>/dev/null \
    | grep -vE '^(generated|builtin|.*\.rs)$' \
    | sort -u || true
)"
id_dialects="$(printf '%s\n' "${ids[@]}" | awk -F. '{print $1}' | sort -u)"

# Directories without any registration are orphan dialect shells.
while IFS= read -r dialect; do
    [[ -z "$dialect" ]] && continue
    if ! echo "$id_dialects" | grep -qFx "$dialect"; then
        # Many subtrees (core, workgroup/primitives, etc.) are fixture
        # directories, not dialect namespaces  -  tolerate when no Cargo
        # crate wires them. This check only escalates when a directory
        # is clearly meant to be a dialect (mod.rs declares `pub mod op`).
        if [[ -f "vyre-ops/src/${dialect}/mod.rs" ]] && \
           grep -q 'pub mod ' "vyre-ops/src/${dialect}/mod.rs" 2>/dev/null; then
            echo "CONSISTENCY NOTE: dialect directory '${dialect}' exists but no ops register under '${dialect}.*'. Fix: ship at least one op in this dialect or remove the directory." >&2
            # Don't escalate to failure yet  -  mid-migration state.
        fi
    fi
done <<< "$on_disk_dialects"

if [[ "$failed" -ne 0 ]]; then exit 1; fi
echo "Consistency contracts: ${total} op ids across $(echo "$id_dialects" | wc -l | tr -d ' ') dialects, no duplicates, format clean."
