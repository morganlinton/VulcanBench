#!/usr/bin/env bash
#
# Layout Law  -  vyre-core prelude must re-export ≤ 15 items.
#
# The prelude is the "one-stop import" (`use vyre::prelude::*;`) a
# frontend writes once. Keeping it small forces us to pick the
# public surface carefully  -  every item exported here is a
# compatibility commitment.
#
# Count: `pub use` items (unique idents). `pub mod` declarations
# don't count. Re-exports of whole modules (`pub use foo::*`)
# expand via `rg -c` for a rough upper bound  -  warn if suspicious.
#
# Modes: default warns; VYRE_LAW_STRICT=1 fails.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

MAX_ITEMS=15
STRICT="${VYRE_LAW_STRICT:-0}"
PRELUDE="vyre-core/src/prelude.rs"

if [[ ! -f "$PRELUDE" ]]; then
  # Prelude file doesn't exist yet  -  A-C13 will ship it as part of
  # the 3-line XOR example. No violation while it's absent.
  echo "Layout Law: prelude $PRELUDE not present yet (informational)."
  exit 0
fi

# Count top-level `pub use X::Y;` items  -  one item per line.
count=$(grep -cE '^\s*pub use ' "$PRELUDE" || true)

# Rough expansion detection for `pub use foo::*;`  -  each is a
# potentially-unbounded surface. Flag if ANY glob re-exports exist.
globs=$(grep -cE '^\s*pub use .*::\*;' "$PRELUDE" || true)
if (( globs > 0 )); then
  echo "Layout Law: prelude uses glob re-exports ($globs total)." >&2
  echo "  Fix: replace `pub use foo::*;` with explicit `pub use foo::{A, B, C};`." >&2
  echo "       The prelude is a pinned public surface; glob imports drift silently." >&2
  if [[ "$STRICT" == "1" ]]; then
    exit 1
  fi
fi

if (( count > MAX_ITEMS )); then
  echo "Layout Law: prelude re-exports $count items, cap is $MAX_ITEMS." >&2
  echo "  Fix: trim to the essentials a frontend imports once per file." >&2
  if [[ "$STRICT" == "1" ]]; then
    exit 1
  fi
  echo '(informational mode  -  set VYRE_LAW_STRICT=1 to fail the build)' >&2
  exit 0
fi

echo "Layout Law: prelude re-exports $count item(s), cap $MAX_ITEMS."
