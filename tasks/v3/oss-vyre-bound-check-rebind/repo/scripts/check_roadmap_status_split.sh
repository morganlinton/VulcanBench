#!/usr/bin/env bash
# Enforce the root roadmap/status/changelog split.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

violations=()

[[ -f ROADMAP.md ]] || violations+=("ROADMAP.md is missing")
[[ -f STATUS.md ]] || violations+=("STATUS.md is missing")
[[ -f CHANGELOG.md ]] || violations+=("CHANGELOG.md is missing")
[[ -f docs/archive/ROADMAP_APPEND_ONLY_2026-05-22.md ]] || violations+=("archived append-only roadmap is missing")

if [[ -f ROADMAP.md ]]; then
  roadmap_lines="$(wc -l < ROADMAP.md | tr -d ' ')"
  if (( roadmap_lines > 200 )); then
    violations+=("ROADMAP.md has ${roadmap_lines} lines; keep future roadmap under 200 lines")
  fi
  if grep -Eq '^### T[0-9]+ \[(x|~|!)\]' ROADMAP.md; then
    violations+=("ROADMAP.md contains append-only task-log entries; move task history to docs/archive/ and status to STATUS.md")
  fi
  if ! grep -Fq 'STATUS.md' ROADMAP.md || ! grep -Fq 'CHANGELOG.md' ROADMAP.md; then
    violations+=("ROADMAP.md must point readers to STATUS.md and CHANGELOG.md")
  fi
fi

if [[ -f STATUS.md ]]; then
  if ! grep -Eq '^Last verified: [0-9]{4}-[0-9]{2}-[0-9]{2}$' STATUS.md; then
    violations+=("STATUS.md must declare Last verified: YYYY-MM-DD")
  fi
  for required in 'Current validated gates' 'Current architectural state' 'Open release risks' 'Historical sources'; do
    if ! grep -Fq "## $required" STATUS.md; then
      violations+=("STATUS.md must contain section: $required")
    fi
  done
fi

if [[ -f docs/INDEX.md ]] && ! grep -Fq 'docs/archive/ROADMAP_APPEND_ONLY_2026-05-22.md' docs/INDEX.md; then
  violations+=("docs/INDEX.md must index the archived append-only roadmap")
fi

if (( ${#violations[@]} > 0 )); then
  printf 'roadmap/status split contract failed.\n' >&2
  printf '%s\n' "${violations[@]}" >&2
  printf '\nFix: keep ROADMAP.md future-only, STATUS.md current-state-only, CHANGELOG.md shipped-history-only, and archive append-only task logs under docs/archive/.\n' >&2
  exit 1
fi

printf 'roadmap/status split contract: root roadmap is future-only and status is separated.\n'
