#!/usr/bin/env python3
"""Apply the admission gate to the pilot and print Jev beside the reference.

    python scripts/verdict-v2/score_pilot.py \
        --reference verdict-v2-items/reference-gpt-6-astra-high-pilot.jsonl \
        --jev verdict-v2-items/predictions-jev-1.13.0-pilot.jsonl
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO))

from harness.verdict.v2.gate import admission  # noqa: E402
from harness.verdict.v2.items import load_items  # noqa: E402
from harness.verdict.v2.scoring import score  # noqa: E402

ITEMS = REPO / "verdict-v2-items"


def _predictions(path: Path | None) -> list[dict[str, Any]]:
    if path is None or not path.exists():
        return []
    rows = [json.loads(line) for line in path.read_text().splitlines() if line.strip()]
    # A reference answer that used a tool did not see the same inputs as Jev.
    return [r for r in rows if not r.get("tool_calls")]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--items", type=Path, default=ITEMS / "items.jsonl")
    parser.add_argument("--pilot", type=Path, default=ITEMS / "pilot.jsonl")
    parser.add_argument("--reference", type=Path, required=True)
    parser.add_argument("--jev", type=Path, default=None)
    parser.add_argument("--json", type=Path, default=None, help="also write results here")
    args = parser.parse_args()

    all_items = load_items(args.items)
    pilot = load_items(args.pilot)
    reference = _predictions(args.reference)
    jev = _predictions(args.jev)
    gate = admission(all_items, pilot, reference)
    jev_result = score(pilot, jev, split="dev", samples=200) if jev else None

    print(f"{'family':18} {'ref':>5} {'jev':>5} {'short':>5} {'test':>5} {'units':>5}  gate")
    for family, g in gate.items():
        jev_skill = jev_result["families"].get(family, {}).get("skill") if jev_result else None

        def fmt(value: float | None) -> str:
            return f"{value:5.0f}" if value is not None else "    -"

        verdict = "admit" if g["admitted"] else "FAIL: " + "; ".join(g["failed"])
        print(
            f"{family:18} {fmt(g['reference_skill'])} {fmt(jev_skill)} "
            f"{fmt(g['best_shortcut_skill'])} {g['test_items']:5} {g['source_units']:5}  {verdict}"
        )
    admitted = sum(g["admitted"] for g in gate.values())
    print(f"{admitted} of {len(gate)} families admitted")
    if args.json:
        args.json.write_text(json.dumps({"gate": gate, "jev": jev_result}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
