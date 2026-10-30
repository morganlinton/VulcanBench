#!/usr/bin/env python3
"""Compare the target's results with the reference's on the visible sample.

usage: python3 /app/corpus/check_visible.py [--show N] [--op OP]

Runs every case in /app/corpus/visible_corpus.jsonl through
/app/corpus/pnrun.py (the same driver grading uses) against the package in
/app/python, and prints how many match the reference's output, per operation,
then the first N mismatches (default 3), optionally only for one operation.
"""
import collections
import json
import subprocess
import sys
import tempfile

show = int(sys.argv[sys.argv.index("--show") + 1]) if "--show" in sys.argv else 3
only = sys.argv[sys.argv.index("--op") + 1] if "--op" in sys.argv else None
cases = [json.loads(line) for line in open("/app/corpus/visible_corpus.jsonl", encoding="utf-8")]
with tempfile.NamedTemporaryFile("w", suffix=".jsonl", encoding="utf-8", delete=False) as f:
    for c in cases:
        f.write(json.dumps({k: v for k, v in c.items() if k != "expected"}, ensure_ascii=False) + "\n")
    path = f.name
r = subprocess.run([sys.executable, "-I", "/app/corpus/pnrun.py", path, "/app/python"],
                   capture_output=True, text=True, timeout=600)
got = {}
for line in r.stdout.splitlines():
    try:
        d = json.loads(line)
        got[d["id"]] = d["out"]
    except (ValueError, KeyError):
        pass
if r.returncode != 0:
    print("driver exited with status %d:\n%s" % (r.returncode, r.stderr[-2000:]))
per = collections.Counter()
tot = collections.Counter()
bad = []
for c in cases:
    tot[c["op"]] += 1
    if got.get(c["id"]) == c["expected"]:
        per[c["op"]] += 1
    elif only in (None, c["op"]):
        bad.append(c)
print("%d / %d visible cases match the reference" % (sum(per.values()), len(cases)))
print("  " + ", ".join("%s %d/%d" % (op, per[op], tot[op]) for op in sorted(tot)))
for c in bad[:show]:
    inp = {k: v for k, v in c.items() if k not in ("expected", "id")}
    print("--- %s %s\n### input\n%s\n### expected\n%s\n### got\n%s" % (
        c["id"], c["op"], json.dumps(inp, ensure_ascii=False), c["expected"], got.get(c["id"], "<no result>")))
