#!/usr/bin/env bash
# Law H  -  Every `unsafe` block carries a `// SAFETY:` comment on the
# line immediately above.
#
# Unsafe code is a contract between the author and the compiler. The
# contract has to be human-readable or the contract does not exist.
# `// SAFETY:` above every unsafe block is the standard established by
# the Rust project itself (rustc/std follow this convention exactly).
# We hold the same line.
#
# The guard rejects unsafe blocks without the comment. It also rejects
# `// SAFETY: TODO`, `// SAFETY: unclear`, `// SAFETY: investigate`
# and other cop-out comments that mean "we don't know yet".

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

violations=0

# Find every `unsafe {` occurrence in production source (not tests,
# not docs, not target/).
while IFS=: read -r file line; do
  # Walk backwards through the contiguous block of `//` comment lines
  # immediately above the unsafe block. The SAFETY: marker may appear
  # anywhere within that block  -  multi-line comments are first-class.
  comment_block=""
  i=$((line - 1))
  while (( i >= 1 )); do
    cur="$(sed -n "${i}p" "$file" 2>/dev/null || true)"
    # Stop scanning once we leave the comment block (non-empty, non-comment line).
    if ! echo "$cur" | grep -qE '^[[:space:]]*(//|$)'; then
      break
    fi
    comment_block="$cur"$'\n'"$comment_block"
    # Bound the scan to 8 lines above the block  -  anything more is a smell.
    if (( line - i >= 8 )); then
      break
    fi
    i=$((i - 1))
  done

  if echo "$comment_block" | grep -qE '//[[:space:]]*SAFETY:[[:space:]]+\S+'; then
    # Check for cop-out markers.
    if echo "$comment_block" | grep -qiE 'SAFETY:[[:space:]]*(TODO|FIXME|unclear|investigate|unknown|tbd|\?\?\?)'; then
      echo "LAW H VIOLATION: unsafe block at $file:$line has a cop-out SAFETY comment." >&2
      echo "  A SAFETY: comment that says 'TODO' or 'unclear' is worse than no comment  -  it promises a justification that does not exist." >&2
      echo "  Fix: write a real SAFETY justification explaining which invariants make this unsafe block sound." >&2
      echo "" >&2
      violations=$((violations + 1))
    fi
    continue
  fi

  echo "LAW H VIOLATION: unsafe block at $file:$line has no SAFETY comment." >&2
  echo "  Every unsafe block must carry a \`// SAFETY: <justification>\` comment in the immediately-preceding comment block." >&2
  echo "  Fix: add the comment explaining why the block is sound." >&2
  echo "" >&2
  violations=$((violations + 1))
done < <(grep -rn -E 'unsafe[[:space:]]*\{' --include='*.rs' "$REPO_ROOT" 2>/dev/null \
          | grep -vE '/target[^/]*/|/\.git/|tests/|benches/|/docs/|/\.cargo-target[^/]*/' \
          | awk -F: '{print $1 ":" $2}')

if [[ "$violations" -gt 0 ]]; then
  echo "Law H failed: $violations unsafe block(s) without SAFETY justification." >&2
  echo "Unsafe code is a contract that must be human-readable." >&2
  exit 1
fi

echo "Law H: every unsafe block has a SAFETY justification."
