import random

import pytest

from harness.verdict.typesafe_adapter import build_request, parse_answer
from harness.verdict.v2.gate import admission, option_text_shortcuts
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
    groups = [split_for_unit(f"entailment-g{k:02d}") for k in range(50)]
    assert groups.count("dev") == 10


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


def test_gate_bounds_the_reference_below_and_the_subject_above():
    items = []
    for i in range(600):
        it = _pair_item(i, correct_first=i % 2 == 0, unit=f"u{i % 60}")
        it["split"] = "dev" if i < 40 else "test"
        items.append(it)
    pilot = items[:40]

    def answers(n_right):
        out = []
        for n, it in enumerate(pilot):
            other = "B" if it["answer"] == "A" else "A"
            pick = it["answer"] if n < n_right else other
            out.append(
                {
                    "item_id": it["item_id"],
                    "probs": {pick: 0.8, (other if pick == it["answer"] else it["answer"]): 0.2},
                }
            )
        return out

    perfect, middling = answers(40), answers(30)
    gate = admission(items, pilot, perfect, middling)["patch-pair"]
    assert gate["admitted"], gate["failed"]  # a perfect reference proves answerability
    assert gate["reference_skill"] == pytest.approx(100.0)
    assert gate["subject_skill"] == pytest.approx(50.0)

    saturated = admission(items, pilot, perfect, answers(39))["patch-pair"]
    assert any("at or above 90" in f for f in saturated["failed"])
    unanswerable = admission(items[:100], pilot, answers(22), middling)["patch-pair"]
    assert any("below 40" in f for f in unanswerable["failed"])
    assert any("test items" in f for f in unanswerable["failed"])


def test_score_families_with_per_item_levels_score_by_position():
    items = []
    for i in range(20):
        levels = [f"{i * 10 + k} to {i * 10 + k + 1}" for k in range(3)]
        it = make_item(
            family="estimate-band",
            key=f"e{i}",
            source_unit=f"estimate-band-g{i:02d}",
            state="s",
            question=score_question("Which band?", levels),
            answer=levels[i % 3],
            reference="generator",
        ).__dict__
        it["split"] = "test"
        items.append(it)
    right = [{"item_id": it["item_id"], "probs": {it["answer"]: 0.9}} for it in items]
    fam = score(items, right, samples=20)["families"]["estimate-band"]
    assert fam["accuracy"] == 1.0
    assert fam["mean_level_error"] == 0.0
    assert fam["floor_strategy"] in {"majority", "uniform"}
    assert fam["brier_skill"] > 0


def test_option_text_shortcuts_find_the_option_nearest_the_rest():
    item = {
        "question": choice_question(
            "Which output?",
            {"A": "total 41", "B": "total 42", "C": "total 43", "D": "error: missing key"},
        )
    }
    guesses = option_text_shortcuts(item)
    assert guesses["text-outlier"] == "D"
    assert guesses["text-medoid"] in {"A", "B", "C"}
    pair = {"question": choice_question("Which?", {"A": "x", "B": "y"})}
    assert option_text_shortcuts(pair) == {}
