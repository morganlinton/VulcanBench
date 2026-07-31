#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

CAP=400
DIR="vyre-core/src"

if [[ ! -d "$DIR" ]]; then
    echo "vyre-core-src-file-cap gate: $DIR absent; count=0 cap=$CAP."
    exit 0
fi

count=$(find "$DIR" -type f -name '*.rs' | wc -l | tr -d ' ')
if (( count >= CAP )); then
    echo "vyre-core-src-file-cap gate: $count Rust files in $DIR (cap <$CAP)." >&2
    echo "Fix: split core surface into modular crates before adding more source files." >&2
    exit 1
fi

echo "vyre-core-src-file-cap gate: $count Rust file(s) in $DIR (cap <$CAP)."
