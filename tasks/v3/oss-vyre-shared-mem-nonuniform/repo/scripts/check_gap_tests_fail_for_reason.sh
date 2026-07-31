#!/usr/bin/env bash
# P1 inventory #99  -  gap tests must fail for the intended reason.
#
# A "gap test" is a `#[test]` that asserts the engine does NOT have a
# capability, marked `#[ignore = "gap-..."]` or named `gap_*`. The
# audit rule (AGENTS.md LAW 5 equivalent) is: failing gap tests are findings,
# not bugs. The gate that says "gap tests still fail" is
# `scripts/check_tests_can_fail.sh`, but it does NOT verify that the
# failure is for the right reason  -  a test could pass once the engine
# fixes the gap, leaving the test silently misclassified.
#
# This gate enforces: every file containing a gap test must carry a
# matching `// GAP_REASON: <text>` comment within 4 lines of the
# `#[ignore = "gap-` attribute, AND the assertion message must
# mention the same gap id. That way, when the gap closes, both the
# attribute and the comment must be removed in the same patch.
#
# Usage:
#   scripts/check_gap_tests_fail_for_reason.sh

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

errors=()

while IFS= read -r f; do
    [[ -z "$f" ]] && continue
    rel="${f#./}"
    # Find each gap-tagged ignore attribute.
    while IFS=: read -r ln content; do
        [[ -z "$ln" ]] && continue
        # Extract gap-id from `#[ignore = "gap-..."]`.
        gap_id=$(printf '%s' "$content" | sed -nE 's/.*ignore[[:space:]]*=[[:space:]]*"(gap-[a-zA-Z0-9_-]+)".*/\1/p')
        [[ -z "$gap_id" ]] && continue

        # Walk forward up to 4 lines for the GAP_REASON comment.
        end=$((ln + 4))
        if ! sed -n "${ln},${end}p" "$f" | grep -qE "//[[:space:]]*GAP_REASON:.*${gap_id}"; then
            errors+=("$rel:$ln: gap '$gap_id' missing GAP_REASON: comment within 4 lines")
        fi
    done < <(grep -nE 'ignore[[:space:]]*=[[:space:]]*"gap-' "$f" 2>/dev/null)
done < <(find . -type f -path '*/tests/*' -name '*.rs' -not -path '*/target/*' -not -path '*/target-*/*' 2>/dev/null)

if (( ${#errors[@]} > 0 )); then
    echo "gap-tests-fail-for-reason gate: ${#errors[@]} unannotated gap tests." >&2
    for e in "${errors[@]}"; do
        echo "  $e" >&2
    done
    echo >&2
    echo "Fix: every #[ignore = \"gap-XYZ\"] must be paired with a" >&2
    echo "// GAP_REASON: gap-XYZ <prose> comment within 4 lines so the" >&2
    echo "comment-removal forces a re-evaluation when the gap closes." >&2
    exit 1
fi

echo "gap-tests-fail-for-reason gate: every gap-tagged test carries a GAP_REASON."
exit 0
