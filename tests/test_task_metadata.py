"""Tests for task scale metadata helpers."""

from __future__ import annotations

import json

from harness.task_metadata import (
    CII_COMPLEXITY_BASE_MINUTES,
    CII_DIFFICULTY_MULTIPLIERS,
    SCALE_DEFAULTS,
    complexity_scaled_budgets,
    infer_task_complexity_from_gold_patch,
    repo_scale,
    resolve_agent_timeout_s,
    resolve_max_steps,
    resolve_verifier_timeout_s,
    task_complexity,
    task_difficulty,
    validate_scale_fields,
)


def test_repo_scale_default() -> None:
    assert repo_scale({}) == "micro"


def test_task_complexity_default_and_valid_values() -> None:
    assert task_complexity({}) == "localized"
    assert task_complexity({"task_complexity": "system"}) == "system"


def test_infer_task_complexity_from_gold_patch() -> None:
    assert (
        infer_task_complexity_from_gold_patch(
            "diff --git a/a.py b/a.py\n"
            "diff --git a/b.py b/b.py\n"
            "diff --git a/README.md b/README.md\n"
        )
        == "multi_file"
    )
    assert (
        infer_task_complexity_from_gold_patch(
            "diff --git a/a.py b/a.py\ndiff --git a/b.ts b/b.ts\ndiff --git a/c.go b/c.go\n"
        )
        == "system"
    )


def test_resolve_max_steps_from_hints() -> None:
    meta = {"repo_scale": "large", "agent_hints": {"suggested_max_steps": 120}}
    assert resolve_max_steps(meta) == 120


def test_resolve_max_steps_cli_caps_hints() -> None:
    meta = {"agent_hints": {"suggested_max_steps": 100}}
    assert resolve_max_steps(meta, cli_max_steps=30) == 30
    assert resolve_max_steps(meta) == 100


def test_resolve_max_steps_override_exceeds_default() -> None:
    meta = {"agent_hints": {"suggested_max_steps": 100}}
    assert resolve_max_steps(meta, cli_max_steps=400, override=True) == 400
    # Without override the CLI value can only cap.
    assert resolve_max_steps(meta, cli_max_steps=400) == 100
    # Override with no CLI value falls back to the task default.
    assert resolve_max_steps(meta, override=True) == 100


def test_resolve_agent_timeout_override_exceeds_default() -> None:
    meta = {"repo_scale": "medium"}
    assert resolve_agent_timeout_s(meta, cli_timeout=7200, override=True) == 7200
    assert resolve_agent_timeout_s(meta, cli_timeout=7200) == 1200.0
    assert resolve_agent_timeout_s(meta, override=True) == 1200.0


def test_resolve_agent_timeout_ignores_test_timeout_s() -> None:
    meta = {"repo_scale": "medium", "test_timeout_s": 120}
    assert resolve_agent_timeout_s(meta) == 1200.0
    assert resolve_verifier_timeout_s(meta) == 120


def test_resolve_agent_timeout_cli_cap() -> None:
    meta = {"repo_scale": "medium", "test_timeout_s": 120}
    assert resolve_agent_timeout_s(meta, cli_timeout=100.0) == 100.0


def test_resolve_verifier_timeout_default() -> None:
    assert resolve_verifier_timeout_s({}) == 120


def test_validate_scale_oss_requires_base_commit() -> None:
    reasons = validate_scale_fields(
        __import__("pathlib").Path("."),
        {"source": "oss", "upstream": {"url": "https://example.com"}},
    )
    assert any("base_commit" in r for r in reasons)


def test_validate_scale_rejects_placeholder_commit() -> None:
    reasons = validate_scale_fields(
        __import__("pathlib").Path("."),
        {
            "source": "oss",
            "base_commit": "0000000000000000000000000000000000000001",
            "upstream": {"url": "https://example.com"},
        },
    )
    assert any("placeholder" in r for r in reasons)


def test_validate_scale_rejects_bad_task_complexity() -> None:
    reasons = validate_scale_fields(
        __import__("pathlib").Path("."),
        {"task_complexity": "giant"},
    )
    assert any("task_complexity" in r for r in reasons)


def test_scale_budgets_can_spend_their_step_allowance() -> None:
    """Wall-clock budget must cover the step allowance at large-repo step cost.

    Steps get more expensive as the repo grows (context-heavy model round trips,
    slower test runs). Measured cost on large repos is 10-15s/step; a budget
    below allowance * that rate cuts runs off mid-work, which measures repo size
    rather than capability.
    """
    worst_case_step_s = 15
    for scale, d in SCALE_DEFAULTS.items():
        steps = d["suggested_max_steps"]
        budget = d["suggested_timeout_s"]
        if scale in ("large", "xlarge"):
            assert budget >= steps * worst_case_step_s, (
                f"{scale}: {budget}s cannot spend {steps} steps at {worst_case_step_s}s/step"
            )


def test_bigger_repos_get_at_least_as_much_wall_clock() -> None:
    # xlarge previously shared large's budget while allowing 33% more steps.
    order = ["micro", "small", "medium", "large", "xlarge"]
    budgets = [SCALE_DEFAULTS[s]["suggested_timeout_s"] for s in order]
    steps = [SCALE_DEFAULTS[s]["suggested_max_steps"] for s in order]
    assert budgets == sorted(budgets), budgets
    assert steps == sorted(steps), steps
    assert (
        SCALE_DEFAULTS["xlarge"]["suggested_timeout_s"]
        > (SCALE_DEFAULTS["large"]["suggested_timeout_s"])
    )


def test_complexity_scaled_budgets_formula() -> None:
    # TerminalBench-style: complexity base x difficulty x scale, half-hour
    # increments, clamped to [30min, 8h].
    # medium multi_file at medium difficulty: 120min * 1.0 * 1.0 = 2h.
    assert complexity_scaled_budgets("medium", "multi_file") == {
        "suggested_max_steps": 360,
        "suggested_timeout_s": 7200,
    }
    # Easy localized on a small repo clamps up to the 30min floor.
    assert complexity_scaled_budgets("small", "localized", "easy")["suggested_timeout_s"] == 1800
    # Hard system on an xlarge repo: 150 * 1.5 * 1.5 = 337.5 -> 360min = 6h.
    assert complexity_scaled_budgets("xlarge", "system", "hard")["suggested_timeout_s"] == 21600
    # veryhard architecture on xlarge hits the 8h ceiling (240*2*1.5 = 720 -> 480).
    assert complexity_scaled_budgets("xlarge", "architecture", "veryhard") == {
        "suggested_max_steps": 1440,
        "suggested_timeout_s": 28800,
    }
    # Steps track the clock at ~20s/step, rounded to tens.
    b = complexity_scaled_budgets("large", "system", "medium")  # 150*1.25=187.5 -> 210min
    assert b["suggested_timeout_s"] == 12600
    assert b["suggested_max_steps"] == 630
    # Unknown values normalize conservatively.
    assert complexity_scaled_budgets("bogus", "bogus", "bogus") == complexity_scaled_budgets(
        "medium", "localized", "medium"
    )
    # Budgets are monotone in complexity and difficulty.
    order_c = ["localized", "multi_file", "system", "architecture"]
    assert [CII_COMPLEXITY_BASE_MINUTES[c] for c in order_c] == sorted(
        CII_COMPLEXITY_BASE_MINUTES[c] for c in order_c
    )
    order_d = ["easy", "medium", "hard", "veryhard"]
    assert [CII_DIFFICULTY_MULTIPLIERS[d] for d in order_d] == sorted(
        CII_DIFFICULTY_MULTIPLIERS[d] for d in order_d
    )


def test_task_difficulty_normalizes() -> None:
    assert task_difficulty({}) == "medium"
    assert task_difficulty({"difficulty": "VeryHard"}) == "veryhard"
    assert task_difficulty({"difficulty": "impossible"}) == "medium"


def test_suite_requiring_explicit_budgets_fails_unstamped_task(tmp_path) -> None:
    suite_dir = tmp_path / "cii-test"
    task_dir = suite_dir / "some-task"
    task_dir.mkdir(parents=True)
    (suite_dir / "suite.json").write_text(
        json.dumps({"require_explicit_budgets": True, "tasks": ["some-task"]}),
        encoding="utf-8",
    )
    meta = {"id": "some-task", "repo_scale": "medium", "task_complexity": "system"}

    reasons = validate_scale_fields(task_dir, meta)
    assert any("suggested_max_steps" in r for r in reasons)
    assert any("suggested_timeout_s" in r for r in reasons)

    # Stamped budgets satisfy the rule.
    meta["agent_hints"] = {"suggested_max_steps": 160, "suggested_timeout_s": 1920}
    assert validate_scale_fields(task_dir, meta) == []

    # Zero/negative/boolean values are rejected.
    meta["agent_hints"] = {"suggested_max_steps": 0, "suggested_timeout_s": True}
    assert len(validate_scale_fields(task_dir, meta)) == 2


def test_suite_without_flag_does_not_require_budgets(tmp_path) -> None:
    suite_dir = tmp_path / "plain"
    task_dir = suite_dir / "some-task"
    task_dir.mkdir(parents=True)
    (suite_dir / "suite.json").write_text(json.dumps({"tasks": ["some-task"]}), encoding="utf-8")
    assert validate_scale_fields(task_dir, {"id": "some-task"}) == []
    # No suite.json at all (loose task dir) also passes.
    loose = tmp_path / "loose" / "task"
    loose.mkdir(parents=True)
    assert validate_scale_fields(loose, {"id": "task"}) == []
