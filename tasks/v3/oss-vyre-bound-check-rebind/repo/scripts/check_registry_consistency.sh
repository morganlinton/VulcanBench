#!/usr/bin/env bash
# Law D  -  Registry consistency: no orphans on either side of op↔backend.
#
# Every op_id that appears in a backend's supported_ops() set MUST have a
# matching NodeKindRegistration in the workspace. An op_id that a backend
# claims to support but no one has implemented is a lie.
#
# Every NodeKindRegistration MUST be referenced by at least one backend's
# supported_ops() set. A primitive that no substrate can execute is an
# orphan that silently fails validation at dispatch time.
#
# Together: the registered-ops set equals the union of all supported_ops
# across all backends. Anything else is a drift between frontend and
# substrate that conformance will not catch until GPU dispatch time.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

violations=0

registered_ids="$(grep -rn -A2 'define_op!' --include='*.rs' "$REPO_ROOT" 2>/dev/null \
    | grep -vE '/tests?/|/target[^/]*/' \
    | grep -vE '(^|[:-][0-9]+[:-])[[:space:]]*///' \
    | grep -vE '(^|[:-][0-9]+[:-])[[:space:]]*//(!)?' \
    | grep -oE 'id[[:space:]]*=[[:space:]]*"[^"]+"' \
    | sed -E 's/id[[:space:]]*=[[:space:]]*"([^"]+)"/\1/' \
    | sort -u || true)"

# Extract op_ids that appear in any supported_ops() set.
# Pattern: HashSet::from(["vyre.foo", "vyre.bar", ...])
#         or [Arc::from("vyre.foo"), ...]
supported_ids="$(grep -rn -E '(supported_ops|SUPPORTED_OPS|supported_op_ids)' --include='*.rs' "$REPO_ROOT" 2>/dev/null \
    | grep -vE '/tests?/|/target[^/]*/' \
    | grep -oE '"[a-z_][a-z0-9_]*(\.[a-z_][a-z0-9_]*)+\"' \
    | tr -d '"' \
    | sort -u || true)"

# Anything registered but not supported by any backend.
registered_but_unsupported=""
if [[ -n "$registered_ids" ]]; then
  registered_but_unsupported="$(comm -23 \
      <(echo "$registered_ids") \
      <(echo "${supported_ids:-}"))"
fi

if [[ -n "$registered_but_unsupported" ]]; then
  while IFS= read -r op_id; do
    [[ -z "$op_id" ]] && continue
    echo "LAW D VIOLATION: op_id '$op_id' is registered as a NodeKind but no backend's supported_ops() includes it." >&2
    echo "  Every registered op must be executable by at least one backend." >&2
    echo "  Fix: either add '$op_id' to a backend's supported_ops() set, or delete the NodeKindRegistration." >&2
    echo "" >&2
    violations=$((violations + 1))
  done <<< "$registered_but_unsupported"
fi

# Anything supported but not registered (excluding common built-ins for
# the hot-path NodeStorage tagged union  -  those are reserved ids).
reserved_ids="vyre.bin_op vyre.un_op vyre.load vyre.store vyre.lit_u32 vyre.lit_i32 vyre.lit_f32 vyre.lit_bool vyre.var vyre.call vyre.select vyre.cast vyre.atomic vyre.fma vyre.buf_len vyre.invocation_id vyre.workgroup_id vyre.local_id"

if [[ -n "$supported_ids" ]]; then
  supported_but_unregistered="$(comm -23 \
      <(echo "$supported_ids") \
      <(echo "${registered_ids:-}") \
      | grep -vFxf <(echo "$reserved_ids" | tr ' ' '\n') || true)"
  if [[ -n "$supported_but_unregistered" ]]; then
    while IFS= read -r op_id; do
      [[ -z "$op_id" ]] && continue
      echo "LAW D VIOLATION: op_id '$op_id' is claimed as supported_ops() but has no NodeKindRegistration." >&2
      echo "  A backend cannot execute an op no one has registered." >&2
      echo "  Fix: either register the NodeKind for '$op_id' or remove it from the supported_ops() set." >&2
      echo "" >&2
      violations=$((violations + 1))
    done <<< "$supported_but_unregistered"
  fi
fi

if [[ "$violations" -gt 0 ]]; then
  echo "Law D failed: $violations registry-consistency violation(s)." >&2
  echo "Registered ops and backend-supported ops must match exactly." >&2
  exit 1
fi

echo "Law D: registered NodeKinds and backend supported_ops are consistent."
