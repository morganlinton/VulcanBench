#!/usr/bin/env bash
# Enforce docs/INDEX.md as the complete freshness map for public docs.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

INDEX="docs/INDEX.md"
if [[ ! -f "$INDEX" ]]; then
  printf 'docs index missing: %s\nFix: create docs/INDEX.md with status and last-verified metadata.\n' "$INDEX" >&2
  exit 1
fi

actual="$(mktemp)"
indexed="$(mktemp)"
missing="$(mktemp)"
stale="$(mktemp)"
trap 'rm -f "$actual" "$indexed" "$missing" "$stale"' EXIT

find docs -type f -name '*.md' ! -path "$INDEX" | sort > "$actual"
grep -Eo '\((docs/)?[^)]*\.md\)' "$INDEX" \
  | tr -d '()' \
  | awk '{ if ($0 ~ /^docs\//) print $0; else print "docs/" $0 }' \
  | sort -u > "$indexed"

comm -23 "$actual" "$indexed" > "$missing"
comm -13 "$actual" "$indexed" > "$stale"

violations=()
if [[ -s "$missing" ]]; then
  violations+=("docs missing from docs/INDEX.md:")
  while IFS= read -r path; do
    violations+=("  $path")
  done < "$missing"
fi

if [[ -s "$stale" ]]; then
  violations+=("docs/INDEX.md points at missing files:")
  while IFS= read -r path; do
    violations+=("  $path")
  done < "$stale"
fi

while IFS= read -r path; do
  relative_path="${path#docs/}"
  line="$(grep -F "]($path)" "$INDEX" || grep -F "]($relative_path)" "$INDEX" || true)"
  if [[ -z "$line" ]]; then
    continue
  fi
  case "$path" in
    docs/archive/*)
      [[ "$line" == \|*\`archived\`* ]] || violations+=("$path must be indexed with status archived")
      ;;
    docs/generated/*)
      [[ "$line" == \|*\`generated\`* ]] || violations+=("$path must be indexed with status generated")
      ;;
    docs/legacy/*)
      if [[ "$line" != \|*\`archived\`* && "$line" != \|*\`superseded\`* ]]; then
        violations+=("$path must be indexed with status archived or superseded")
      fi
      ;;
  esac
done < "$actual"

if ! grep -Eq '^Last verified: [0-9]{4}-[0-9]{2}-[0-9]{2}$' "$INDEX"; then
  violations+=("docs/INDEX.md must declare Last verified: YYYY-MM-DD")
fi

if (( ${#violations[@]} > 0 )); then
  printf 'documentation index contract failed.\n' >&2
  printf '%s\n' "${violations[@]}" >&2
  printf '\nFix: add every docs/*.md file to docs/INDEX.md with current/generated/archived/superseded status and remove stale links.\n' >&2
  exit 1
fi

printf 'documentation index contract: docs/INDEX.md covers every markdown document.\n'
