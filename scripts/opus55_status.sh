#!/usr/bin/env bash
# Progress of the Opus 5.5 sweep chain. Safe to run any time; reads only.
cd "$(dirname "$0")/.."
echo "== Frontier v4 =="
t=0; for l in low medium high extra-high max; do
  n=$(find "runs-effort-opus55/$l" -name summary.json 2>/dev/null | wc -l | tr -d ' ')
  t=$((t+n)); printf "  %-11s %2s/23\n" "$l" "$n"
done; echo "  total: $t/115"
echo "== Safety v1 =="
t=0; for l in low medium high extra-high max; do
  n=$(find "/Users/morganlinton/dev/VulcanConduct/runs/runs-conduct-v1-opus55/$l" -name summary.json 2>/dev/null | wc -l | tr -d ' ')
  t=$((t+n)); printf "  %-11s %2s/10\n" "$l" "$n"
done; echo "  total: $t/50"
pgrep -f run_opus55_all_suites.sh >/dev/null && echo "chain: ALIVE" || echo "chain: NOT RUNNING"
tail -2 logs/opus55-all-suites.log
