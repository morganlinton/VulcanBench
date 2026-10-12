#!/usr/bin/env bash
# Phase 23a enforcement: every .expect("...") must include actionable "Fix:" guidance.
# Counts a site as OK if "Fix:" appears on the same line or within the next 3 lines.
#
# Ratchet: set VYRE_EXPECT_BASELINE to committed baseline; fails if count exceeds baseline.
set -uo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

count=0
while IFS= read -r hit; do
  file="${hit%%:*}"
  line_no="${hit#*:}"
  line_no="${line_no%%:*}"
  line_text=$(sed -n "${line_no}p" "$file")
  # Skip string-literal / hygiene scans, not real expect calls
  [[ "$line_text" == *'contains(".expect("'* ]] && continue
  [[ "$line_text" == *"contains('.expect('"* ]] && continue
  # Skip tests/benches
  [[ "$file" == *"/tests/"* || "$file" == *"/benches/"* ]] && continue
  # Skip LAW7 fragment dirs (often mid-statement chunks)
  [[ "$file" == *"/__law7_split/"* ]] && continue
  window=$(sed -n "${line_no},$((line_no + 3))p" "$file")
  # Skip meta-string hygiene scans on this line's window only
  grep -q 'production\.contains.*\.expect(' <<<"$window" 2>/dev/null && continue
  grep -q 'contains("\.expect(")' <<<"$window" 2>/dev/null && continue
  if ! grep -q 'Fix:' <<<"$window"; then
    count=$((count + 1))
  fi
done < <(grep -rn '\.expect("' --include='*.rs' \
  --exclude-dir=target --exclude-dir=.git \
  --exclude-dir=__law7_split \
  . 2>/dev/null \
  | grep -v '("\.expect("' \
  | grep -v 'concat!.*expect')

baseline="${VYRE_EXPECT_BASELINE:-0}"

echo "expect() sites lacking 'Fix:' guidance: $count (baseline $baseline)"

if (( count > baseline )); then
  echo "New expect() site without 'Fix:' annotation  -  ratchet violated." >&2
  echo "Either add a Fix: clause or bump the baseline in scripts/check_expect_has_fix.sh." >&2
  exit 1
fi

exit 0
