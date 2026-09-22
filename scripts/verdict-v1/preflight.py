#!/usr/bin/env python3
"""Live preflight for the TypeSafe adapter: a few dev-split items through Jev.

Confirms auth, the request and response shapes, the served model version and
usage reporting before any full pass. Prints no secrets and no item content.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO))

from dotenv import load_dotenv  # noqa: E402

from harness.verdict.items import load_items  # noqa: E402
from harness.verdict.scoring import answer_label, score  # noqa: E402
from harness.verdict.typesafe_adapter import DEFAULT_MODEL, predict  # noqa: E402


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--items", type=Path, default=REPO / "verdict-v1-items" / "items.jsonl")
    parser.add_argument("--env", type=Path, default=REPO / ".env")
    parser.add_argument("--per-family", type=int, default=2)
    parser.add_argument("--model", default=DEFAULT_MODEL)
    parser.add_argument(
        "-o", "--out", type=Path, default=REPO / "verdict-v1-items" / "preflight-predictions.jsonl"
    )
    args = parser.parse_args()
    load_dotenv(args.env)

    rows = [r for r in load_items(args.items) if r["split"] == "dev"]
    picked: list[dict] = []
    seen: dict[str, int] = {}
    for row in rows:
        if seen.get(row["family"], 0) < args.per_family:
            picked.append(row)
            seen[row["family"]] = seen.get(row["family"], 0) + 1

    predictions = []
    for item in picked:
        prediction = predict(item, model=args.model)
        predictions.append(prediction)
        top = max(prediction["probs"], key=prediction["probs"].get)
        truth = answer_label(item)
        print(
            f"{item['family']:18s} served={prediction['model']:12s} tokens={prediction['input_tokens']} "
            f"latency={prediction['latency_ms']:.0f}ms top={top[:24]!r} p={prediction['probs'][top]:.3f} "
            f"{'OK ' if top == truth else 'MISS'} attempts={prediction['attempts']}"
        )
    args.out.write_text("".join(json.dumps(p) + "\n" for p in predictions))
    result = score(picked, predictions, split="dev")
    print(
        json.dumps(
            {
                f: {k: round(v, 3) for k, v in m.items() if k in ("n", "accuracy", "brier")}
                for f, m in result["families"].items()
            }
        )
    )
    print(f"total cost ${sum(p['cost_usd'] or 0 for p in predictions):.5f}; wrote {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
