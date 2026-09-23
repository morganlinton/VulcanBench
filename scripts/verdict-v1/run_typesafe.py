#!/usr/bin/env python3
"""Run every Verdict v1 item through Jev and write one prediction per line.

Resumable: items already present in the output file are skipped, so a
rate-limit or network failure costs nothing but a rerun. Prints no secrets
and no item content.
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

from harness.verdict.items import load_items  # noqa: E402
from harness.verdict.typesafe_adapter import DEFAULT_MODEL, predict  # noqa: E402


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--items", type=Path, default=REPO / "verdict-v1-items" / "items.jsonl")
    parser.add_argument("--env", type=Path, default=REPO / ".env")
    parser.add_argument("--model", default=DEFAULT_MODEL)
    parser.add_argument("--split", default=None, help="dev or test; default both")
    parser.add_argument("-o", "--out", type=Path, default=None)
    args = parser.parse_args()
    load_dotenv(args.env)
    out = args.out or REPO / "verdict-v1-items" / f"predictions-{args.model}.jsonl"

    items = [r for r in load_items(args.items) if args.split is None or r["split"] == args.split]
    done = (
        {json.loads(line)["item_id"] for line in out.read_text().splitlines() if line.strip()}
        if out.exists()
        else set()
    )
    todo = [i for i in items if i["item_id"] not in done]
    print(f"{len(done)} done, {len(todo)} to go, model {args.model}", flush=True)

    started = time.monotonic()
    failures = 0
    with out.open("a") as handle:
        for n, item in enumerate(todo, 1):
            try:
                prediction = predict(item, model=args.model)
            except Exception as error:
                failures += 1
                print(
                    f"[{n}/{len(todo)}] {item['item_id']} failed: {type(error).__name__}: {error}",
                    flush=True,
                )
                continue
            prediction["queried_at"] = time.strftime("%Y-%m-%dT%H:%M:%S%z")
            handle.write(json.dumps(prediction) + "\n")
            handle.flush()
            if n % 100 == 0 or n == len(todo):
                print(f"[{n}/{len(todo)}] {time.monotonic() - started:.0f}s elapsed", flush=True)
    print(
        f"finished: {len(todo) - failures} written, {failures} failed, {time.monotonic() - started:.0f}s"
    )
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
