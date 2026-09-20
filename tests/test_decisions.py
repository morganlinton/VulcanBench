import json
import math

import pytest

from harness.decisions.items import build_items, outcome_label, split_for_task
from harness.decisions.scoring import (
    base_rate_predictions,
    expected_calibration_error,
    noul_probs,
    score,
)
from harness.decisions.typesafe_adapter import (
    TypeSafeNotConfigured,
    build_request,
    parse_answer,
    predict,
)

GOLD = "--- a/core/calc.py\n+++ b/core/calc.py\n@@ -1 +1 @@\n-x = 1\n+x = 2\n"


def _summary(targets, p2p_ok=True, finished=True, contaminated=False):
    return {
        "suite": "coding-intelligence-index-v4",
        "task_id": "demo-task",
        "run_id": "demo-task-0001",
        "model": "codex:demo",
        "finished": finished,
        "effort": {"requested": "low"},
        "integrity_audit": {"contaminated": contaminated},
        "verifier": {"fail_to_pass": targets, "pass_to_pass_ok": p2p_ok},
    }


def _write_run(root, name, summary, patch):
    run = root / name
    run.mkdir(parents=True)
    (run / "summary.json").write_text(json.dumps(summary))
    (run / "final.patch").write_text(patch)


@pytest.fixture
def tree(tmp_path):
    task = tmp_path / "tasks" / "demo-task"
    (task / "repo" / "core").mkdir(parents=True)
    (task / "repo" / "core" / "calc.py").write_text("x = 1\n")
    (task / "repo" / "README.md").write_text("demo\n")
    (task / "issue.md").write_text("x should be 2")
    (task / "gold_patch.diff").write_text(GOLD)
    runs = tmp_path / "runs-demo"
    _write_run(runs, "a", _summary({"t1": True, "t2": True}), "diff a")
    _write_run(runs, "b", _summary({"t1": True, "t2": False}), "diff b")
    _write_run(runs, "c", _summary({"t1": True, "t2": True}, p2p_ok=False), "diff c")
    _write_run(runs, "dup", _summary({"t1": True, "t2": True}), "diff a")
    _write_run(runs, "leak", _summary({"t1": True}, contaminated=True), "diff leak")
    _write_run(runs, "empty", _summary({"t1": False}), "")
    return runs, tmp_path / "tasks"


def test_outcome_labels():
    assert outcome_label(_summary({"a": True})) == "all_pass"
    assert outcome_label(_summary({"a": True, "b": False})) == "partial_fix"
    assert outcome_label(_summary({"a": False})) == "no_progress"
    assert outcome_label(_summary({"a": True}, p2p_ok=False)) == "regression"
    assert outcome_label(_summary({"a": True}, finished=False)) is None


def test_build_items_filters_and_dedupes(tree):
    runs, tasks = tree
    items = [json.loads(i.to_json()) for i in build_items([runs], tasks)]
    by_family = {}
    for item in items:
        by_family.setdefault(item["family"], []).append(item)
    # Three distinct clean patches; the duplicate, contaminated and empty runs drop out.
    assert {f: len(v) for f, v in by_family.items()} == {
        "patch-verdict": 3,
        "patch-regression": 3,
        "patch-outcome": 3,
        "fix-localization": 1,
    }
    assert sorted(i["answer"] for i in by_family["patch-outcome"]) == [
        "all_pass",
        "partial_fix",
        "regression",
    ]
    assert sum(i["answer"] for i in by_family["patch-verdict"]) == 1
    localization = by_family["fix-localization"][0]
    assert localization["answer"] == "core/calc.py"
    assert localization["question"]["options"] == ["README.md", "core/calc.py"]
    assert {i["split"] for i in items} == {split_for_task("demo-task")}
    assert all(i["canary"] for i in items)


def _noul(item_id, answer, split="test"):
    return {
        "item_id": item_id,
        "family": "patch-verdict",
        "split": split,
        "question": {"type": "noul", "instructions": "s"},
        "answer": answer,
    }


def test_score_perfect_and_confidently_wrong():
    items = [_noul("i1", True), _noul("i2", False)]
    perfect = score(
        items,
        [
            {"item_id": "i1", "probs": noul_probs(1.0), "latency_ms": 100, "cost_usd": 0.001},
            {"item_id": "i2", "probs": noul_probs(0.0), "latency_ms": 300, "cost_usd": 0.003},
        ],
    )["overall"]
    assert perfect["accuracy"] == 1.0 and perfect["brier"] == 0.0 and perfect["ece"] == 0.0
    assert perfect["latency_ms_p50"] == 200 and perfect["cost_usd_per_1k"] == pytest.approx(2.0)
    wrong = score(
        items,
        [{"item_id": "i1", "probs": noul_probs(0.0)}, {"item_id": "i2", "probs": noul_probs(1.0)}],
    )
    assert (
        wrong["overall"]["accuracy"] == 0.0
        and wrong["overall"]["brier"] == 1.0
        and wrong["overall"]["ece"] == 1.0
    )


def test_score_counts_missing_items_as_wrong_and_rejects_bad_input():
    items = [_noul("i1", True), _noul("i2", False), _noul("d1", True, split="dev")]
    result = score(items, [{"item_id": "i1", "probs": noul_probs(0.9)}])["overall"]
    assert (
        result["coverage"] == 0.5
        and result["accuracy"] == 1.0
        and result["accuracy_all_items"] == 0.5
    )
    with pytest.raises(ValueError, match="not in the question"):
        score(items, [{"item_id": "i1", "probs": {"maybe": 1.0}}])
    with pytest.raises(ValueError, match="duplicate"):
        score(items, [{"item_id": "i1", "probs": noul_probs(0.9)}] * 2)


def test_ece_and_base_rate_floor():
    assert expected_calibration_error([0.75] * 4, [True, True, True, False]) == pytest.approx(0.0)
    items = [_noul(f"d{i}", i < 3, split="dev") for i in range(4)] + [_noul("t1", True)]
    (prediction,) = base_rate_predictions(items)
    assert prediction["probs"]["true"] == pytest.approx(4 / 6)
    assert math.isclose(sum(prediction["probs"].values()), 1.0)


def test_typesafe_request_shape_and_unconfigured(monkeypatch):
    item = {**_noul("i1", True), "state": "ticket text"}
    request = build_request(item)
    assert request == {
        "model": "jev-1.13.0",
        "state": "ticket text",
        "questions": {"q": {"type": "noul", "instructions": "s"}},
    }
    body = {"answers": {"q": {"type": "noul", "noul": 0.999}}}
    assert parse_answer(item, body) == pytest.approx({"true": 0.999, "false": 0.001})
    choice = {
        "item_id": "c1",
        "state": "s",
        "question": {
            "type": "choice",
            "options": ["a", "b"],
            "descriptions": {"a": "first"},
            "instructions": "pick",
        },
    }
    assert build_request(choice)["questions"]["q"]["criteria"] == {"a": "first", "b": None}
    answer = {
        "type": "choice",
        "choice": "b",
        "probabilities": {"b": 0.7, "a": 0.3},
        "confidence": 0.4,
    }
    assert parse_answer(choice, {"answers": {"q": answer}}) == {"b": 0.7, "a": 0.3}
    monkeypatch.delenv("TYPESAFE_API_KEY", raising=False)
    with pytest.raises(TypeSafeNotConfigured):
        predict(item)


def test_panel_preference_needs_both_judges_to_agree(tree, tmp_path):
    runs, tasks = tree
    review = tmp_path / "runs-review"
    review.mkdir()

    def row(sub, muse, grok):
        return {
            "id": sub,
            "task": "demo-task",
            "panels": {
                "muse": {"l1": {"score": muse}, "reviewer_fallback": False},
                "grok": {"l1": {"score": grok}, "reviewer_fallback": False},
            },
        }

    (review / "summary.json").write_text(
        json.dumps({"rows": [row("s1", 80, 75), row("s2", 50, 55), row("s3", 85, 70)]})
    )
    (review / "private-manifest.json").write_text(
        json.dumps(
            [
                {"id": s, "source_directory": str(runs / d)}
                for s, d in (("s1", "a"), ("s2", "b"), ("s3", "c"))
            ]
        )
    )
    items = [json.loads(i.to_json()) for i in build_items([], tasks, review_dirs=[review])]
    pairs = [i for i in items if i["family"] == "quality-preference"]
    # s1 vs s3 is dropped: the judges disagree on direction and the margins are small. s2 loses both remaining pairs.
    assert len(pairs) == 2 and all(i["reference"] == "llm-panel" for i in pairs)
    for item in pairs:
        winner = item["state"].split("## Patch B")[0 if item["answer"] == "A" else 1]
        assert "diff b" not in winner
    result = score(
        items, [{"item_id": i["item_id"], "probs": {i["answer"]: 1.0}} for i in pairs], split=None
    )
    assert result["agreement_only_families"] == ["quality-preference"]
    assert (
        result["overall"]["n"] == 0 and result["families"]["quality-preference"]["accuracy"] == 1.0
    )
