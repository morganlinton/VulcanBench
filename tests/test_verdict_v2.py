import random

import pytest

from harness.verdict.typesafe_adapter import build_request, parse_answer
from harness.verdict.v2.gate import admission
from harness.verdict.v2.items import (
    choice_question,
    make_item,
    noul_question,
    score_question,
    shuffled_options,
    split_for_unit,
)
from harness.verdict.v2.registry import BY_ID, FAMILIES, PILLARS
from harness.verdict.v2.scoring import floor_for, item_rows, score, separated, skill


def _pair_item(n, correct_first, unit):
    options = {"A": "first", "B": "second"}
    return make_item(
        family="patch-pair",
        key=f"k{n}",
        source_unit=unit,
        state=f"state {n}",
        question=choice_question("Which passes?", options),
        answer="A" if correct_first else "B",
        reference="verifier",
        shortcuts={"larger": "A"},
    ).__dict__


def test_registry_has_twenty_families_in_two_pillars():
    assert len(FAMILIES) == 20
    assert {f.pillar for f in FAMILIES} == set(PILLARS)
    assert sum(f.pillar == "software" for f in FAMILIES) == 12
    assert len(BY_ID) == 20


def test_split_is_stable_and_by_unit():
    assert split_for_unit("task-a") == split_for_unit("task-a")
    splits = {split_for_unit(f"unit-{i}") for i in range(200)}
    assert splits == {"dev", "test"}


def test_make_item_rejects_llm_references_and_bad_answers():
    with pytest.raises(ValueError, match="ground truth"):
        make_item(
            family="x",
            key="k",
            source_unit="u",
            state="s",
            question=noul_question("q"),
            answer=True,
            reference="llm-panel",
        )
    with pytest.raises(ValueError, match="not an option"):
        make_item(
            family="x",
            key="k",
            source_unit="u",
            state="s",
            question=score_question("q", ["low", "high"]),
            answer="mid",
            reference="generator",
        )


def test_shuffled_options_track_the_correct_text():
    rng = random.Random(1)
    seen = set()
    for _ in range(50):
        options, correct = shuffled_options(rng, "right", ["w1", "w2", "w3"])
        assert options == ["A", "B", "C", "D"]
        seen.add(correct)
    assert seen == {"A", "B", "C", "D"}
    with pytest.raises(ValueError):
        shuffled_options(rng, "same", ["same"])


def test_skill_is_zero_at_floor_and_100_at_perfect():
    assert skill(0.5, 0.5) == 0
    assert skill(1.0, 0.5) == 100
    assert skill(0.25, 0.5) == -50


def test_floor_takes_the_best_shortcut():
    items = [_pair_item(i, correct_first=i % 4 != 0, unit=f"u{i}") for i in range(40)]
    rows = item_rows(items, [])
    floor, name = floor_for(rows)
    assert name in {"majority", "shortcut:larger"}
    assert floor == pytest.approx(0.75)


def test_a_model_at_the_floor_scores_zero_not_the_floor():
    """The v1 failure: 62.8% against a 62.7% floor must read as about 0."""
    items = [_pair_item(i, correct_first=i % 4 != 0, unit=f"u{i}") for i in range(400)]
    for it in items:
        it["split"] = "test"
    always_a = [{"item_id": it["item_id"], "probs": {"A": 0.9, "B": 0.1}} for it in items]
    result = score(items, always_a, samples=50)
    fam = result["families"]["patch-pair"]
    assert fam["accuracy"] == pytest.approx(0.75)
    assert fam["skill"] == pytest.approx(0.0)
    perfect = [{"item_id": it["item_id"], "probs": {it["answer"]: 1.0}} for it in items]
    best = score(items, perfect, samples=50)
    assert best["families"]["patch-pair"]["skill"] == pytest.approx(100.0)
    assert best["indices"]["software_index"]["value"] == pytest.approx(100.0)
    assert best["indices"]["general_index"]["value"] is None
    assert separated(result, best)


def test_missing_predictions_count_as_wrong():
    items = [_pair_item(i, correct_first=True, unit=f"u{i}") for i in range(10)]
    rows = item_rows(items, [])
    assert not any(r["correct"] for r in rows)


def test_score_questions_round_trip_through_the_adapter():
    levels = ["patch", "minor", "major"]
    item = {"question": score_question("Which bump?", levels), "state": "diff"}
    request = build_request(item)
    assert request["questions"]["q"]["criteria"] == levels
    body = {"answers": {"q": {"type": "score", "probabilities": {"0": 0.2, "1": 0.7, "2": 0.1}}}}
    assert parse_answer(item, body) == {"patch": 0.2, "minor": 0.7, "major": 0.1}


def test_gate_admits_an_answerable_unshortcuttable_family_and_explains_failures():
    items = []
    for i in range(600):
        it = _pair_item(i, correct_first=i % 2 == 0, unit=f"u{i % 60}")
        # Shortcut "larger" is right half the time on a balanced family.
        it["split"] = "dev" if i < 40 else "test"
        items.append(it)
    pilot = items[:40]
    # Reference right on 34 of 40 (skill 70).
    reference = [
        {
            "item_id": it["item_id"],
            "probs": {it["answer"]: 0.8, ("B" if it["answer"] == "A" else "A"): 0.2}
            if n < 34
            else {it["answer"]: 0.2, ("B" if it["answer"] == "A" else "A"): 0.8},
        }
        for n, it in enumerate(pilot)
    ]
    gate = admission(items, pilot, reference)["patch-pair"]
    assert gate["admitted"], gate["failed"]
    assert gate["reference_skill"] == pytest.approx(70.0)

    saturated = [{"item_id": it["item_id"], "probs": {it["answer"]: 1.0}} for it in pilot]
    failed = admission(items[:100], pilot, saturated)["patch-pair"]
    assert not failed["admitted"]
    assert any("outside 40 to 95" in f for f in failed["failed"])
    assert any("test items" in f for f in failed["failed"])
