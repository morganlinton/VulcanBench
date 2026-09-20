"""Score typed-decision predictions against verified answers.

A prediction is ``{"item_id", "probs": {label: p}, "latency_ms", "cost_usd"}``.
For a ``noul`` item the labels are ``"true"`` and ``"false"``; for a
``choice`` item they are the options. Accuracy says whether the top label is
right; Brier, log loss and expected calibration error say whether the stated
probabilities deserve to be believed, which is the claim these models make.
"""

from __future__ import annotations

import math
from collections import defaultdict
from typing import Any

from harness.decisions.items import GROUND_TRUTH_REFERENCES

ECE_BINS = 10
LOG_FLOOR = 1e-6


def answer_label(item: dict[str, Any]) -> str:
    if item["question"]["type"] == "noul":
        return "true" if item["answer"] else "false"
    return str(item["answer"])


def item_labels(item: dict[str, Any]) -> list[str]:
    if item["question"]["type"] == "noul":
        return ["true", "false"]
    return [str(option) for option in item["question"]["options"]]


def noul_probs(p_true: float) -> dict[str, float]:
    return {"true": p_true, "false": 1.0 - p_true}


def normalized_probs(item: dict[str, Any], probs: dict[str, float]) -> dict[str, float]:
    """Probabilities over exactly the item's labels, summing to 1."""
    labels = item_labels(item)
    unknown = set(probs) - set(labels)
    if unknown:
        raise ValueError(f"{item['item_id']}: labels not in the question: {sorted(unknown)}")
    values = {label: float(probs.get(label, 0.0)) for label in labels}
    if any(v < 0 or math.isnan(v) for v in values.values()):
        raise ValueError(f"{item['item_id']}: negative or NaN probability")
    total = sum(values.values())
    if total <= 0:
        raise ValueError(f"{item['item_id']}: probabilities sum to zero")
    return {label: v / total for label, v in values.items()}


def brier(item: dict[str, Any], probs: dict[str, float], truth: str) -> float:
    """Binary Brier (0 to 1) for noul; multiclass Brier (0 to 2) for choice."""
    if item["question"]["type"] == "noul":
        return (probs["true"] - (truth == "true")) ** 2
    return sum((p - (label == truth)) ** 2 for label, p in probs.items())


def percentile(values: list[float], q: float) -> float | None:
    if not values:
        return None
    ordered = sorted(values)
    rank = (len(ordered) - 1) * q
    low, high = math.floor(rank), math.ceil(rank)
    return ordered[low] + (ordered[high] - ordered[low]) * (rank - low)


def expected_calibration_error(
    confidences: list[float], correct: list[bool], bins: int = ECE_BINS
) -> float:
    """Mean gap between stated confidence and observed accuracy, weighted by bin size."""
    buckets: dict[int, list[tuple[float, bool]]] = defaultdict(list)
    for confidence, ok in zip(confidences, correct, strict=True):
        buckets[min(int(confidence * bins), bins - 1)].append((confidence, ok))
    total = len(confidences)
    return sum(
        len(rows)
        / total
        * abs(sum(c for c, _ in rows) / len(rows) - sum(ok for _, ok in rows) / len(rows))
        for rows in buckets.values()
    )


def _aggregate(rows: list[dict[str, Any]], expected: int) -> dict[str, Any]:
    n = len(rows)
    latencies = [r["latency_ms"] for r in rows if r["latency_ms"] is not None]
    costs = [r["cost_usd"] for r in rows if r["cost_usd"] is not None]
    out: dict[str, Any] = {
        "n": n,
        "expected": expected,
        "coverage": n / expected if expected else 0.0,
    }
    if not n:
        return out
    accuracy = sum(r["correct"] for r in rows) / n
    out.update(
        accuracy=accuracy,
        # Unanswered items count as wrong, so skipping hard items cannot help.
        accuracy_all_items=sum(r["correct"] for r in rows) / expected,
        accuracy_stderr=math.sqrt(accuracy * (1 - accuracy) / n),
        brier=sum(r["brier"] for r in rows) / n,
        log_loss=sum(r["log_loss"] for r in rows) / n,
        ece=expected_calibration_error(
            [r["confidence"] for r in rows], [r["correct"] for r in rows]
        ),
        latency_ms_p50=percentile(latencies, 0.5),
        latency_ms_p95=percentile(latencies, 0.95),
        cost_usd_per_1k=sum(costs) / len(costs) * 1000 if costs else None,
    )
    return out


def score(
    items: list[dict[str, Any]], predictions: list[dict[str, Any]], split: str | None = "test"
) -> dict[str, Any]:
    """Per-family and overall metrics for one model's predictions."""
    pool = {i["item_id"]: i for i in items if split is None or i["split"] == split}
    expected: dict[str, int] = defaultdict(int)
    for item in pool.values():
        expected[item["family"]] += 1

    rows: dict[str, list[dict[str, Any]]] = defaultdict(list)
    seen: set[str] = set()
    for prediction in predictions:
        item = pool.get(prediction["item_id"])
        if item is None:
            continue
        if item["item_id"] in seen:
            raise ValueError(f"duplicate prediction for {item['item_id']}")
        seen.add(item["item_id"])
        probs = normalized_probs(item, prediction["probs"])
        truth = answer_label(item)
        top = max(probs, key=probs.get)
        rows[item["family"]].append(
            {
                "correct": top == truth,
                "confidence": probs[top],
                "brier": brier(item, probs, truth),
                "log_loss": -math.log(max(probs[truth], LOG_FLOOR)),
                "latency_ms": prediction.get("latency_ms"),
                "cost_usd": prediction.get("cost_usd"),
            }
        )

    families = {
        family: _aggregate(rows.get(family, []), count)
        for family, count in sorted(expected.items())
    }
    # Headline numbers use ground-truth families only; judge-agreement families stay apart.
    truth_families = {
        i["family"]
        for i in pool.values()
        if i.get("reference", "verifier") in GROUND_TRUTH_REFERENCES
    }
    everything = [row for family in truth_families for row in rows.get(family, [])]
    return {
        "split": split,
        "overall": _aggregate(everything, sum(expected[f] for f in truth_families)),
        "families": families,
        "agreement_only_families": sorted(set(expected) - truth_families),
    }


def base_rate_predictions(items: list[dict[str, Any]], split: str = "test") -> list[dict[str, Any]]:
    """The floor any model must beat: predict each family's dev-split label frequencies."""
    counts: dict[str, dict[str, int]] = defaultdict(lambda: defaultdict(int))
    for item in items:
        # Only families with one fixed label set have a base rate; the rest stay uniform.
        if item["split"] != split and item["family"] not in (
            "fix-localization",
            "quality-preference",
        ):
            counts[item["family"]][answer_label(item)] += 1
    predictions = []
    for item in items:
        if item["split"] != split:
            continue
        labels = item_labels(item)
        seen = counts.get(item["family"], {})
        total = sum(seen.values())
        # Add-one smoothing.
        probs = {label: (seen.get(label, 0) + 1) / (total + len(labels)) for label in labels}
        predictions.append(
            {"item_id": item["item_id"], "probs": probs, "latency_ms": None, "cost_usd": None}
        )
    return predictions
