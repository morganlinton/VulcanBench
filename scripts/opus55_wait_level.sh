#!/usr/bin/env bash
# Block until the next unfinished Frontier v4 effort level reaches 23 distinct
# tasks (or the chain stops), then print that level's result and exit.
cd "$(dirname "$0")/.."
count() { find "runs-effort-opus55/$1" -name summary.json 2>/dev/null -exec \
  python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["task_id"])' {} \; 2>/dev/null | sort -u | wc -l | tr -d ' '; }
for l in low medium high extra-high max; do
  [ "$(count $l)" -ge 23 ] && continue
  until [ "$(count $l)" -ge 23 ] || ! pgrep -f run_opus55_all_suites.sh >/dev/null; do sleep 120; done
  if [ "$(count $l)" -ge 23 ]; then
    echo "LEVEL DONE: $l $(date '+%F %H:%M')"
    python3 - "$l" <<'PY'
import json, glob, statistics, sys
l = sys.argv[1]
s = [json.load(open(f)) for f in glob.glob(f"runs-effort-opus55/{l}/*/summary.json")]
p = sum(x["scores"].get("functional") == 1.0 for x in s)
print(f"pass {p}/{len(s)}  mean functional {statistics.mean(x['scores']['functional'] for x in s):.3f}  "
      f"mean total {statistics.mean(x['scores']['total'] for x in s):.3f}  "
      f"median {statistics.median(x['duration_s'] for x in s)/60:.1f} min  "
      f"cost ${sum((x.get('economics') or {}).get('api_equivalent_cost_usd') or 0 for x in s):.2f}  "
      f"models {sorted({(x.get('cli_agent') or {}).get('reported_model') for x in s})}")
import collections, os
reply = collections.Counter(); ev = collections.Counter()
for f in glob.glob(f"runs-effort-opus55/{l}/*/cli-agent-stream.jsonl"):
    for line in open(f, errors="replace"):
        if "refusal" in line and '"subtype"' in line:
            try: ev[json.loads(line).get("subtype")] += 1
            except Exception: pass
        if '"assistant"' in line:
            try:
                e = json.loads(line)
                if e.get("type") == "assistant": reply[e["message"].get("model")] += 1
            except Exception: pass
print(f"replies by model {dict(reply)}  refusal events {dict(ev)}")
PY
  else
    echo "CHAIN STOPPED during $l at $(count $l)/23 $(date '+%F %H:%M')"; tail -3 logs/opus55-all-suites.log
  fi
  exit 0
done
echo "ALL FRONTIER LEVELS DONE"
