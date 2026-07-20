#!/usr/bin/env bash
# fetch_honest_corpus.sh  -  download and verify honest workload corpora.
#
# Usage: ./scripts/fetch_honest_corpus.sh
#
# Idempotent: skips files whose SHA-256 already matches CHECKSUMS.toml.
# Requires: curl, sha256sum, toml-cli (or grep+awk fallback).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BENCH_DIR="$(dirname "$SCRIPT_DIR")"
CHECKSUMS_FILE="$BENCH_DIR/corpus/honest/CHECKSUMS.toml"
CORPUS_DIR="$BENCH_DIR/corpus/honest"

if [ ! -f "$CHECKSUMS_FILE" ]; then
    echo "No CHECKSUMS.toml found at $CHECKSUMS_FILE"
    echo "Currently all honest workload corpora are synthesized in prepare()."
    echo "When external corpora are added, this script will download them."
    exit 0
fi

echo "=== vyre-bench honest corpus fetcher ==="
echo "Corpus dir: $CORPUS_DIR"

# Parse CHECKSUMS.toml entries
# Expected format:
# [[file]]
# path = "parser.json/nativejson-benchmark/test.json"
# url = "https://..."
# sha256 = "abc123..."
#
# For now, all honest workloads use synthesized data, so this is a no-op.

DOWNLOADED=0
SKIPPED=0
FAILED=0

echo ""
echo "All honest workload corpora are currently synthesized."
echo "No external downloads required."
echo ""
echo "Summary: downloaded=$DOWNLOADED skipped=$SKIPPED failed=$FAILED"
