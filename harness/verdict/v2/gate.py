"""The admission gate every Verdict v2 family must pass before it ships.

Rules (docs/DECISIONS.md, 2026-09-24): on the pilot, the reference model's
skill is between 40 and 95, the best shortcut's skill is at most 15, and the
full build has at least 200 test items from at least 20 source units.
Shortcut skill is measured against majority and uniform guessing only, since
the shortcut is what is being judged.
"""

from __future__ import annotations

from collections import defaultdict
from collections.abc import Sequence
from typing import Any

from harness.verdict.v2.scoring import floor_for, item_rows, skill

REFERENCE_SKILL_MIN = 40.0
REFERENCE_SKILL_MAX = 95.0
SHORTCUT_SKILL_MAX = 15.0
MIN_TEST_ITEMS = 200
MIN_SOURCE_UNITS = 20


def shortcut_skills(rows: Sequence[dict[str, Any]]) -> dict[str, float]:
    plain = [{**r, "shortcuts": {}} for r in rows]
    base, _ = floor_for(plain)
    names = {name for r in rows for name in r["shortcuts"]}
    return {
        name: skill(sum(r["shortcuts"].get(name, False) for r in rows) / len(rows), base)
        for name in sorted(names)
    }


def admission(
    all_items: Sequence[dict[str, Any]],
    pilot_items: Sequence[dict[str, Any]],
    reference_predictions: Sequence[dict[str, Any]],
) -> dict[str, dict[str, Any]]:
    """Per family: the gate's measurements and a list of failed rules."""
    test_items: dict[str, int] = defaultdict(int)
    units: dict[str, set[str]] = defaultdict(set)
    for item in all_items:
        units[item["family"]].add(item["source_unit"])
        if item["split"] == "test":
            test_items[item["family"]] += 1
    pilot_rows: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in item_rows(pilot_items, reference_predictions):
        pilot_rows[row["family"]].append(row)

    out: dict[str, dict[str, Any]] = {}
    for family in sorted(set(test_items) | set(units) | set(pilot_rows)):
        rows = pilot_rows.get(family, [])
        failed = []
        reference_skill = None
        best_shortcut = None
        if rows:
            floor, _ = floor_for(rows)
            reference_skill = skill(sum(r["correct"] for r in rows) / len(rows), floor)
            shortcuts = shortcut_skills(rows)
            best_shortcut = max(shortcuts.values()) if shortcuts else None
            answered = sum(r["answered"] for r in rows)
            if answered < len(rows):
                failed.append(f"reference answered {answered} of {len(rows)}")
            if not REFERENCE_SKILL_MIN <= reference_skill <= REFERENCE_SKILL_MAX:
                failed.append(f"reference skill {reference_skill:.0f} outside 40 to 95")
            if best_shortcut is not None and best_shortcut > SHORTCUT_SKILL_MAX:
                failed.append(f"shortcut skill {best_shortcut:.0f} above 15")
        else:
            failed.append("no pilot items")
        if test_items[family] < MIN_TEST_ITEMS:
            failed.append(f"{test_items[family]} test items, need {MIN_TEST_ITEMS}")
        if len(units[family]) < MIN_SOURCE_UNITS:
            failed.append(f"{len(units[family])} source units, need {MIN_SOURCE_UNITS}")
        out[family] = {
            "pilot_items": len(rows),
            "reference_skill": reference_skill,
            "best_shortcut_skill": best_shortcut,
            "test_items": test_items[family],
            "source_units": len(units[family]),
            "failed": failed,
            "admitted": not failed,
        }
    return out
