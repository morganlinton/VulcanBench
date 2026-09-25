"""Re-price finished runs from their token receipts using the current price table.

Rewrites cost_usd, cost_detail and economics.api_equivalent_cost_usd in each
summary.json, keeping the first-seen values under "repriced". Scores, tokens
and timings are untouched. Idempotent: rerunning recomputes from tokens again.

    python scripts/cii-v4-board/reprice_runs.py runs-effort-gpt55 [more roots...]
"""

import datetime
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from harness.pricing import cost_usd, has_cached_input_price  # noqa: E402


def reprice(summary_path: Path) -> tuple[float | None, float | None]:
    s = json.loads(summary_path.read_text())
    tokens = s["tokens"]
    model = s["model"]
    new = cost_usd(
        model,
        tokens["prompt"],
        tokens["completion"],
        cached_input_tokens=tokens.get("cached_input", 0),
    )
    old = s.get("cost_usd")
    if new is None or (old is not None and abs(new - old) < 1e-9):
        return old, new
    s.setdefault(
        "repriced", {"original_cost_usd": old, "original_cost_detail": s.get("cost_detail")}
    )
    s["repriced"]["date"] = datetime.date.today().isoformat()
    s["repriced"]["reason"] = "price table update: cached-input rate added or list price changed"
    s["cost_usd"] = new
    detail = s.get("cost_detail") or {}
    judges = detail.get("judges", 0.0) or 0.0
    detail.update(
        {
            "agent": new - judges,
            "total": new,
            "cache_pricing_applied": bool(
                tokens.get("cached_input") and has_cached_input_price(model)
            ),
        }
    )
    s["cost_detail"] = detail
    if s.get("economics"):
        s["economics"]["api_equivalent_cost_usd"] = new
    summary_path.write_text(json.dumps(s, indent=2) + "\n")
    return old, new


def main():
    roots = [Path(a) for a in sys.argv[1:]] or [Path("runs")]
    changed = 0
    for root in roots:
        for path in sorted(root.rglob("summary.json")):
            old, new = reprice(path)
            if old != new:
                changed += 1
    print(f"repriced {changed} run(s)")


if __name__ == "__main__":
    main()
