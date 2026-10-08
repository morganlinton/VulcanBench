#!/usr/bin/env python3
"""Compare the target's HTML with the reference output on the visible sample.

usage: python3 /app/corpus/check_visible.py [--show N]

Builds nothing: run `cargo build --release --bin comrak` first. Each case runs
target/release/comrak with the same flags the grader uses:
  --unsafe --syntax-highlighting none --gfm-quirks [-e EXT ...] [--smart]
"""
import json
import subprocess
import sys

show = int(sys.argv[sys.argv.index("--show") + 1]) if "--show" in sys.argv else 3
cases = [json.loads(line) for line in open("/app/corpus/visible_corpus.jsonl", encoding="utf-8")]
bad = []
for c in cases:
    argv = ["/app/target/release/comrak", "--unsafe", "--syntax-highlighting", "none", "--gfm-quirks"]
    for e in c["extensions"]:
        argv += ["-e", e]
    if c["smart"]:
        argv.append("--smart")
    r = subprocess.run(argv, input=c["markdown"].encode(), capture_output=True, timeout=20)
    if r.returncode != 0 or r.stdout.decode("utf-8", "replace") != c["expected_html"]:
        bad.append((c, r.stdout.decode("utf-8", "replace")))
print(f"{len(cases) - len(bad)} / {len(cases)} visible cases match the reference")
for c, got in bad[:show]:
    print(f"--- {c['id']} extensions={c['extensions']}\n### markdown\n{c['markdown']}### expected\n{c['expected_html']}### got\n{got}")
