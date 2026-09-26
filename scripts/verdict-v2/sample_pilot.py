#!/usr/bin/env python3
"""Pick the pilot: a fixed random sample of dev items per family.

The pilot runs on the dev split so the gate never touches published test
items. Writes a JSONL of items; prints counts only.
"""

from __future__ import annotations

import argparse
import json
import random
from collections import defaultdict
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]

ITEMS = REPO / "verdict-v2-items"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--items", type=Path, default=ITEMS / "items.jsonl")
    parser.add_argument("--per-family", type=int, default=30)
    parser.add_argument("--seed", type=int, default=20260924)
    parser.add_argument("-o", "--out", type=Path, default=ITEMS / "pilot.jsonl")
    args = parser.parse_args()

    by_family: dict[str, list[str]] = defaultdict(list)
    for line in args.items.read_text().splitlines():
        if line.strip():
            item = json.loads(line)
            if item["split"] == "dev":
                by_family[item["family"]].append(line)
    rng = random.Random(args.seed)
    chosen = []
    for family in sorted(by_family):
        pool = by_family[family]
        picked = rng.sample(pool, min(args.per_family, len(pool)))
        chosen.extend(picked)
        short = "" if len(picked) == args.per_family else "  (short)"
        print(f"{family:18} {len(picked):3}{short}")
    args.out.write_text("".join(line + "\n" for line in chosen))
    print(f"wrote {len(chosen)} pilot items to {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
