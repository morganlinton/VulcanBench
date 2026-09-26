#!/usr/bin/env python3
"""Run Verdict v2 items through Jev and write one prediction per line.

Resumable: items already in the output are skipped. Prints no secrets and
no item content.

    python scripts/verdict-v2/run_jev.py --items verdict-v2-items/pilot.jsonl
"""

from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO))

from dotenv import load_dotenv  # noqa: E402

from harness.verdict.typesafe_adapter import DEFAULT_MODEL, predict  # noqa: E402
from harness.verdict.v2.items import load_items  # noqa: E402

ITEMS = REPO / "verdict-v2-items"
MAIN_ENV = Path.home() / "dev" / "VulcanBench" / ".env"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--items", type=Path, default=ITEMS / "items.jsonl")
    parser.add_argument("--env", type=Path, default=MAIN_ENV)
    parser.add_argument("--model", default=DEFAULT_MODEL)
    parser.add_argument("--split", default=None, help="dev or test; default both")
    parser.add_argument("-o", "--out", type=Path, default=None)
    args = parser.parse_args()
    load_dotenv(args.env)
    out = args.out or ITEMS / f"predictions-{args.model}-{args.items.stem}.jsonl"

    items = [r for r in load_items(args.items) if args.split in (None, r["split"])]
    done = (
        {json.loads(line)["item_id"] for line in out.read_text().splitlines() if line.strip()}
        if out.exists()
        else set()
    )
    todo = [i for i in items if i["item_id"] not in done]
    print(f"{len(done)} done, {len(todo)} to go, model {args.model}", flush=True)
    failures = 0
    with out.open("a") as handle:
        for n, item in enumerate(todo, 1):
            try:
                prediction = predict(item, model=args.model)
            except Exception as error:
                failures += 1
                print(f"[{n}/{len(todo)}] {item['item_id']} failed: {type(error).__name__}")
                continue
            prediction["queried_at"] = time.strftime("%Y-%m-%dT%H:%M:%S%z")
            handle.write(json.dumps(prediction) + "\n")
            handle.flush()
            if n % 100 == 0 or n == len(todo):
                print(f"[{n}/{len(todo)}] written", flush=True)
    print(f"finished: {len(todo) - failures} written, {failures} failed")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
