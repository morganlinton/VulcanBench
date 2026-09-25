"""Score v2 predictions: skill above the best shortcut, and the indices.

A prediction is ``{"item_id", "probs": {label: p}, ...}`` exactly as in v1.
Per family, skill is ``100 * (acc - floor) / (1 - floor)``, where ``acc`` is
accuracy of the model's top answer and ``floor`` is the best of: always
giving the most common answer, a uniform guess, and every shortcut baseline
the builder recorded. A missing prediction counts as wrong.

The Verdict Index is the mean skill over families; Software and General
sub-indices are the same mean within a pillar. The Calibration Index is the
mean Brier skill score against the base-rate forecast. Intervals resample
source units, not items.
"""

from __future__ import annotations

import math
import random
from collections import Counter, defaultdict
from collections.abc import Sequence
from typing import Any

from harness.verdict.scoring import auroc, expected_calibration_error
from harness.verdict.v2.items import answer_label, labels_for
from harness.verdict.v2.registry import BY_ID, PILLARS

BOOTSTRAP_SAMPLES = 1000
LOG_FLOOR = 1e-6


def _normalized(labels: list[str], probs: dict[str, float] | None) -> dict[str, float]:
    if not probs:
        return {label: 1 / len(labels) for label in labels}
    unknown = set(probs) - set(labels)
    if unknown:
        raise ValueError(f"labels not in the question: {sorted(unknown)}")
    values = {label: max(0.0, float(probs.get(label, 0.0))) for label in labels}
    total = sum(values.values())
    if total <= 0:
        return {label: 1 / len(labels) for label in labels}
    return {label: v / total for label, v in values.items()}


def item_rows(
    items: Sequence[dict[str, Any]], predictions: Sequence[dict[str, Any]]
) -> list[dict[str, Any]]:
    """One scored row per item, predicted or not."""
    by_id = {p["item_id"]: p for p in predictions}
    rows = []
    for item in items:
        labels = labels_for(item["question"])
        truth = answer_label(item["question"], item["answer"])
        prediction = by_id.get(item["item_id"])
        probs = _normalized(labels, prediction["probs"] if prediction else None)
        # Ties break to the first label in question order, which builders
        # randomise, so a flat answer earns chance and nothing more.
        top = max(labels, key=lambda label: probs[label])
        rows.append(
            {
                "item_id": item["item_id"],
                "family": item["family"],
                "source_unit": item["source_unit"],
                "type": item["question"]["type"],
                "labels": labels,
                "truth": truth,
                "probs": probs,
                "answered": prediction is not None,
                "correct": prediction is not None and top == truth,
                "confidence": probs[top],
                "brier": sum((p - (label == truth)) ** 2 for label, p in probs.items()),
                "log_loss": -math.log(max(probs[truth], LOG_FLOOR)),
                "shortcuts": {
                    name: guess == truth for name, guess in item.get("shortcuts", {}).items()
                },
            }
        )
    return rows


def floor_for(rows: Sequence[dict[str, Any]]) -> tuple[float, str]:
    """Best trivial accuracy on these rows and the name of the strategy."""
    n = len(rows)
    candidates = {
        "majority": Counter(r["truth"] for r in rows).most_common(1)[0][1] / n,
        "uniform": sum(1 / len(r["labels"]) for r in rows) / n,
    }
    names = {name for r in rows for name in r["shortcuts"]}
    for name in names:
        candidates[f"shortcut:{name}"] = sum(r["shortcuts"].get(name, False) for r in rows) / n
    best = max(candidates, key=lambda name: candidates[name])
    return candidates[best], best


def skill(accuracy: float, floor: float) -> float:
    if floor >= 1:
        return 0.0
    return 100 * (accuracy - floor) / (1 - floor)


def brier_skill(rows: Sequence[dict[str, Any]]) -> float | None:
    """1 minus model Brier over the Brier of forecasting the label frequencies."""
    counts = Counter(r["truth"] for r in rows)
    n = len(rows)
    reference = 0.0
    for r in rows:
        base = {label: counts.get(label, 0) / n for label in r["labels"]}
        reference += sum((p - (label == r["truth"])) ** 2 for label, p in base.items())
    reference /= n
    if reference <= 0:
        return None
    model: float = sum(r["brier"] for r in rows) / n
    return 1 - model / reference


def ranking(rows: Sequence[dict[str, Any]]) -> float | None:
    """AUROC for yes/no, macro one-vs-rest otherwise; None when undefined."""
    if not rows:
        return None
    if rows[0]["type"] == "noul":
        value: float | None = auroc(
            [r["probs"]["true"] for r in rows], [r["truth"] == "true" for r in rows]
        )
        return value
    if len({tuple(r["labels"]) for r in rows}) > 1:
        return None
    values = [
        v
        for label in rows[0]["labels"]
        if (v := auroc([r["probs"][label] for r in rows], [r["truth"] == label for r in rows]))
        is not None
    ]
    return sum(values) / len(values) if values else None


def family_point(rows: Sequence[dict[str, Any]]) -> dict[str, Any]:
    n = len(rows)
    accuracy = sum(r["correct"] for r in rows) / n
    floor, floor_name = floor_for(rows)
    out: dict[str, Any] = {
        "n": n,
        "answered": sum(r["answered"] for r in rows),
        "source_units": len({r["source_unit"] for r in rows}),
        "accuracy": accuracy,
        "floor": floor,
        "floor_strategy": floor_name,
        "skill": skill(accuracy, floor),
        "ranking": ranking(rows),
        "brier_skill": brier_skill(rows),
        "ece": expected_calibration_error(
            [r["confidence"] for r in rows], [r["correct"] for r in rows]
        ),
        "log_loss": sum(r["log_loss"] for r in rows) / n,
    }
    if rows[0]["type"] == "score":
        index = {label: i for i, label in enumerate(rows[0]["labels"])}
        out["mean_level_error"] = (
            sum(
                abs(index[max(r["probs"], key=lambda k: r["probs"][k])] - index[r["truth"]])
                for r in rows
            )
            / n
        )
    return out


def _indices(families: dict[str, dict[str, Any]]) -> dict[str, float | None]:
    def mean(values: list[float]) -> float | None:
        return sum(values) / len(values) if values else None

    out = {"verdict_index": mean([f["skill"] for f in families.values()])}
    for pillar in PILLARS:
        out[f"{pillar}_index"] = mean(
            [f["skill"] for fid, f in families.items() if BY_ID[fid].pillar == pillar]
        )
    out["calibration_index"] = mean(
        [f["brier_skill"] for f in families.values() if f["brier_skill"] is not None]
    )
    return out


def _resample(
    rows_by_family: dict[str, list[dict[str, Any]]], rng: random.Random
) -> dict[str, list[dict[str, Any]]]:
    out = {}
    for family, rows in rows_by_family.items():
        by_unit: dict[str, list[dict[str, Any]]] = defaultdict(list)
        for r in rows:
            by_unit[r["source_unit"]].append(r)
        units = list(by_unit)
        out[family] = [r for _ in units for r in by_unit[rng.choice(units)]]
    return out


def score(
    items: Sequence[dict[str, Any]],
    predictions: Sequence[dict[str, Any]],
    split: str = "test",
    samples: int = BOOTSTRAP_SAMPLES,
    seed: int = 0,
) -> dict[str, Any]:
    chosen = [i for i in items if i["split"] == split]
    rows_by_family: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in item_rows(chosen, predictions):
        rows_by_family[row["family"]].append(row)
    families = {f: family_point(rows) for f, rows in sorted(rows_by_family.items())}
    point = _indices(families)

    rng = random.Random(seed)
    draws: dict[str, list[float]] = defaultdict(list)
    for _ in range(samples):
        resampled = _resample(rows_by_family, rng)
        fams = {f: family_point(rows) for f, rows in resampled.items() if rows}
        for f, values in fams.items():
            draws[f"family:{f}"].append(values["skill"])
        for name, value in _indices(fams).items():
            if value is not None:
                draws[name].append(value)

    def interval(key: str) -> list[float] | None:
        values = sorted(draws.get(key, []))
        if not values:
            return None
        return [values[int(0.025 * len(values))], values[int(0.975 * len(values)) - 1]]

    for f, values in families.items():
        values["skill_ci95"] = interval(f"family:{f}")
    return {
        "split": split,
        "families": families,
        "indices": {
            name: {"value": value, "ci95": interval(name)} for name, value in point.items()
        },
    }


def separated(a: dict[str, Any], b: dict[str, Any], index: str = "verdict_index") -> bool:
    """True only when the two models' 95% intervals on an index do not overlap."""
    lo_a, hi_a = a["indices"][index]["ci95"]
    lo_b, hi_b = b["indices"][index]["ci95"]
    return bool(hi_a < lo_b or hi_b < lo_a)
