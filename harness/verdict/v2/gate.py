"""The admission gate every Verdict v2 family must pass before it ships.

Rules (docs/DECISIONS.md, 2026-09-24 and its 2026-09-25 amendment): on the
pilot, the reference model's skill is at least 40 (the family is answerable)
and the subject model's skill is below 90 (the family still measures it);
on the full build, the best shortcut's skill is at most 15 and there are at
least 200 test items from at least 20 source units. Shortcut skill is
measured against majority and uniform guessing only, since the shortcut is
what is being judged.
"""

from __future__ import annotations

import difflib
from collections import defaultdict
from collections.abc import Sequence
from typing import Any

from harness.verdict.v2.scoring import floor_for, item_rows, skill

REFERENCE_SKILL_MIN = 40.0
# The ceiling applies to the model under test, not the reference: a reasoning
# reference at high effort solves generated puzzles perfectly (pilot,
# 2026-09-25), which proves them answerable rather than useless.
SUBJECT_SKILL_MAX = 90.0
SHORTCUT_SKILL_MAX = 15.0
MIN_TEST_ITEMS = 200
MIN_SOURCE_UNITS = 20


def option_text_shortcuts(item: dict[str, Any]) -> dict[str, str]:
    """Generic shortcuts over the option texts, applied to every choice family.

    When wrong options are built as small edits of the right one, the right
    one is the option most similar to the rest (``text-medoid``). code-output
    leaked this way (88.7% on its first build) without any builder recording
    it, so the gate now checks it everywhere, with its mirror
    ``text-outlier``.
    """
    q = item["question"]
    if q["type"] != "choice" or len(q["options"]) < 3:
        return {}
    descriptions = q.get("descriptions") or {}
    texts = {str(o): str(descriptions.get(o) or o) for o in q["options"]}
    if len(set(texts.values())) < len(texts):
        return {}

    def closeness(option: str) -> float:
        return sum(
            difflib.SequenceMatcher(None, texts[option], texts[other], autojunk=False).ratio()
            for other in texts
            if other != option
        )

    ranked = sorted(texts, key=closeness)
    return {"text-medoid": ranked[-1], "text-outlier": ranked[0]}


def shortcut_skills(rows: Sequence[dict[str, Any]]) -> dict[str, float]:
    plain = [{**r, "shortcuts": {}} for r in rows]
    base, _ = floor_for(plain)
    names = {name for r in rows for name in r["shortcuts"]}
    return {
        name: skill(sum(r["shortcuts"].get(name, False) for r in rows) / len(rows), base)
        for name in sorted(names)
    }


def _family_rows(
    items: Sequence[dict[str, Any]], predictions: Sequence[dict[str, Any]]
) -> dict[str, list[dict[str, Any]]]:
    rows: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in item_rows(items, predictions):
        rows[row["family"]].append(row)
    return rows


def _skill(rows: Sequence[dict[str, Any]]) -> float:
    floor, _ = floor_for(rows)
    return skill(sum(r["correct"] for r in rows) / len(rows), floor)


def admission(
    all_items: Sequence[dict[str, Any]],
    pilot_items: Sequence[dict[str, Any]],
    reference_predictions: Sequence[dict[str, Any]],
    subject_predictions: Sequence[dict[str, Any]],
) -> dict[str, dict[str, Any]]:
    """Per family: the gate's measurements and a list of failed rules."""
    test_items: dict[str, int] = defaultdict(int)
    units: dict[str, set[str]] = defaultdict(set)
    for item in all_items:
        units[item["family"]].add(item["source_unit"])
        if item["split"] == "test":
            test_items[item["family"]] += 1
    full_rows = _family_rows(
        [
            {**i, "shortcuts": {**i.get("shortcuts", {}), **option_text_shortcuts(i)}}
            for i in all_items
        ],
        [],
    )
    reference_rows = _family_rows(pilot_items, reference_predictions)
    subject_rows = _family_rows(pilot_items, subject_predictions)

    out: dict[str, dict[str, Any]] = {}
    for family in sorted(set(units) | set(reference_rows)):
        failed = []
        ref, subj = reference_rows.get(family, []), subject_rows.get(family, [])
        reference_skill = _skill(ref) if ref else None
        subject_skill = _skill(subj) if subj else None
        shortcuts = shortcut_skills(full_rows[family]) if full_rows.get(family) else {}
        best_shortcut = max(shortcuts.values()) if shortcuts else None
        if reference_skill is None:
            failed.append("no reference pilot answers")
        else:
            answered = sum(r["answered"] for r in ref)
            if answered < len(ref):
                failed.append(f"reference answered {answered} of {len(ref)}")
            if reference_skill < REFERENCE_SKILL_MIN:
                failed.append(f"reference skill {reference_skill:.0f} below 40")
        if subject_skill is None:
            failed.append("no subject pilot answers")
        elif subject_skill >= SUBJECT_SKILL_MAX:
            failed.append(f"subject skill {subject_skill:.0f} at or above 90")
        if best_shortcut is not None and best_shortcut > SHORTCUT_SKILL_MAX:
            failed.append(f"shortcut skill {best_shortcut:.1f} above 15")
        if test_items[family] < MIN_TEST_ITEMS:
            failed.append(f"{test_items[family]} test items, need {MIN_TEST_ITEMS}")
        if len(units[family]) < MIN_SOURCE_UNITS:
            failed.append(f"{len(units[family])} source units, need {MIN_SOURCE_UNITS}")
        out[family] = {
            "pilot_items": len(ref),
            "reference_skill": reference_skill,
            "subject_skill": subject_skill,
            "best_shortcut_skill": best_shortcut,
            "test_items": test_items[family],
            "source_units": len(units[family]),
            "failed": failed,
            "admitted": not failed,
        }
    return out
