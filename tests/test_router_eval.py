"""Router-eval scripts: ladder prompt builder, analysis maths, cards."""

from __future__ import annotations

import importlib.util
import json
import sys
from collections import Counter
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts" / "router-eval"


def _load(name: str):
    spec = importlib.util.spec_from_file_location(f"router_eval_{name}", SCRIPTS / f"{name}.py")
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


analyze = _load("analyze")
ladder = _load("probe_ladder")


def test_chi2_sf_matches_known_values() -> None:
    # chi2 with 1 dof: P(X > 3.841) = 0.05; 2 dof: P(X > 5.991) = 0.05
    assert analyze.chi2_sf(3.841, 1) == pytest.approx(0.05, abs=0.001)
    assert analyze.chi2_sf(5.991, 2) == pytest.approx(0.05, abs=0.001)
    assert analyze.chi2_sf(0.0, 3) == 1.0
    assert analyze.chi2_sf(50.0, 1) < 1e-10


def test_chi_square_independence_on_hand_tables() -> None:
    perfect = {"easy": Counter({"cheap": 10}), "hard": Counter({"frontier": 10})}
    t = analyze.chi_square(perfect)
    assert t["statistic"] == pytest.approx(20.0)
    assert t["dof"] == 1
    assert t["p_value"] < 1e-4
    assert analyze.normalized_mutual_information(perfect) == pytest.approx(1.0)

    flat = {
        "easy": Counter({"cheap": 5, "frontier": 5}),
        "hard": Counter({"cheap": 5, "frontier": 5}),
    }
    t = analyze.chi_square(flat)
    assert t["statistic"] == 0.0
    assert t["p_value"] == pytest.approx(1.0)
    assert analyze.normalized_mutual_information(flat) == 0.0

    constant = {"easy": Counter({"cheap": 8}), "hard": Counter({"cheap": 8})}
    assert analyze.chi_square(constant)["dof"] == 0
    assert analyze.normalized_mutual_information(constant) == 0.0


def _run(task: str, leg: str, cell: dict[str, str], **extra) -> dict:
    base = {
        "run_id": f"{task}-abc",
        "task_id": task,
        "scores": {"functional": 1.0},
        "cost_usd": 0.5,
        "tokens": {"total": 1000},
        "duration_s": 60,
        "steps": 4,
        "economics": {
            "billing_mode": "api-metered",
            "cost_basis": "metered-api-pricing",
            "marginal_cash_usd": 0.5,
            "api_equivalent_cost_usd": 0.5,
        },
    }
    base.update(extra)
    return base


CELLS = {
    "a": {"language": "python", "scale": "small", "difficulty": "easy"},
    "b": {"language": "python", "scale": "large", "difficulty": "hard"},
}


def test_summary_row_router_and_subscription() -> None:
    router = analyze.summary_row(
        _run(
            "a",
            "r",
            CELLS["a"],
            served_models={"x/cheap": 3, "x/big": 1},
            served_model_switches=1,
            router_reported_cost_usd=0.2,
            economics={
                "billing_mode": "api-metered",
                "cost_basis": "router-reported",
                "marginal_cash_usd": 0.2,
                "api_equivalent_cost_usd": 0.5,
            },
        ),
        "r",
        CELLS,
    )
    assert router["cash_usd"] == 0.2 and router["api_equivalent_usd"] == 0.5
    assert router["cost_basis"] == "router-reported"
    assert router["served_models"] == {"x/cheap": 3, "x/big": 1}

    sub = analyze.summary_row(
        _run(
            "b",
            "max",
            CELLS["b"],
            cost_usd=None,
            cli_agent={
                "harness": "claude-code",
                "plan_name": "max",
                "cli_reported_cost_usd": 2.0,
                "served_models": {"claude-opus-5-5": 5},
            },
            economics={
                "billing_mode": "subscription-included",
                "cost_basis": "subscription-plus-api-equivalent",
                "marginal_cash_usd": None,
                "api_equivalent_cost_usd": 2.0,
                "plan_name": "max",
                "quota": {"source": "stream", "delta": {"five_hour": 0.02, "seven_day": 0.004}},
            },
        ),
        "max",
        CELLS,
    )
    assert sub["cash_usd"] is None and sub["api_equivalent_usd"] == 2.0
    assert sub["quota_week_window"] == "seven_day" and sub["quota_week_delta"] == 0.004
    assert sub["served_models"] == {"claude-opus-5-5": 5}


def test_capacity_and_break_even() -> None:
    rows = [
        analyze.summary_row(
            _run(
                "a",
                "max",
                CELLS["a"],
                cost_usd=None,
                cli_agent={
                    "harness": "claude-code",
                    "plan_name": "max",
                    "cli_reported_cost_usd": 1.0,
                },
                economics={
                    "billing_mode": "subscription-included",
                    "cost_basis": "subscription-plus-api-equivalent",
                    "marginal_cash_usd": None,
                    "api_equivalent_cost_usd": 1.0,
                    "quota": {"source": "probe", "delta": {"seven_day": 0.01}},
                },
            ),
            "max",
            CELLS,
        ),
        analyze.summary_row(
            _run(
                "b",
                "max",
                CELLS["b"],
                cost_usd=None,
                cli_agent={
                    "harness": "claude-code",
                    "plan_name": "max",
                    "cli_reported_cost_usd": 3.0,
                },
                economics={
                    "billing_mode": "subscription-included",
                    "cost_basis": "subscription-plus-api-equivalent",
                    "marginal_cash_usd": None,
                    "api_equivalent_cost_usd": 3.0,
                    "quota": {"source": "probe", "delta": {"seven_day": 0.03}},
                },
            ),
            "max",
            CELLS,
        ),
        analyze.summary_row(
            _run(
                "a",
                "router",
                CELLS["a"],
                economics={
                    "billing_mode": "api-metered",
                    "cost_basis": "router-reported",
                    "marginal_cash_usd": 0.25,
                    "api_equivalent_cost_usd": None,
                },
            ),
            "router",
            CELLS,
        ),
        analyze.summary_row(
            _run(
                "b",
                "router",
                CELLS["b"],
                scores={"functional": 0.0},
                economics={
                    "billing_mode": "api-metered",
                    "cost_basis": "router-reported",
                    "marginal_cash_usd": 0.75,
                    "api_equivalent_cost_usd": None,
                },
            ),
            "router",
            CELLS,
        ),
    ]
    analysis = analyze.analyze([], rows, {"max": 200.0}, {})
    cap = analysis["subscription_capacity"]["max"]
    # pooled: (1 + 3) / (0.01 + 0.03) = 100 list-price dollars per week
    assert cap["capacity_usd_week"] == pytest.approx(100.0)
    assert cap["capacity_usd_month"] == pytest.approx(100 * 30 / 7, abs=0.01)
    # mean API-equivalent per task is 2.0, so capacity is ~214 tasks per month
    assert cap["tasks_per_month_capacity"] == pytest.approx(100 * 30 / 7 / 2.0, abs=0.1)
    cost = analysis["cost"]
    assert cost["router"]["cash"]["mean_usd"] == pytest.approx(0.5)
    assert cost["router"]["cash"]["per_solved_usd"] == pytest.approx(1.0)  # $1 spent, 1 solved
    assert cost["router"]["cash"]["pass_rate"] == 0.5
    assert cost["max"]["cash"]["mean_usd"] is None  # never a fake zero
    be = analysis["break_even"]
    cross = next(c for c in be["crossovers"] if c["metered"] == "router")
    assert cross["tasks_per_month_where_seat_wins"] == pytest.approx(200 / 0.5)
    assert cross["within_one_seat_capacity"] is False  # 400 tasks > 214 capacity
    sub_curve = be["curves"]["max"]
    assert sub_curve["points"][0]["usd"] == 200.0  # one seat at low demand


def test_ladder_routing_with_length_control() -> None:
    cell_small = {"language": "python", "scale": "small", "difficulty": "easy"}
    cell_large = {"language": "python", "scale": "large", "difficulty": "hard"}
    rows = []
    for rep in range(6):
        rows.append(
            {
                "router": "r",
                "task_id": "a",
                "cell": cell_small,
                "control": None,
                "rep": rep,
                "served_model": "x/cheap",
                "prompt_tokens": 1000,
                "reported_cost_usd": 0.001,
                "latency_s": 1.0,
            }
        )
        rows.append(
            {
                "router": "r",
                "task_id": "a",
                "cell": cell_small,
                "control": "padded",
                "rep": rep,
                "served_model": "x/big",
                "prompt_tokens": 20000,
                "reported_cost_usd": 0.01,
                "latency_s": 2.0,
            }
        )
        rows.append(
            {
                "router": "r",
                "task_id": "b",
                "cell": cell_large,
                "control": None,
                "rep": rep,
                "served_model": "x/big",
                "prompt_tokens": 20000,
                "reported_cost_usd": 0.01,
                "latency_s": 2.0,
            }
        )
    rows.append(
        {
            "router": "r",
            "task_id": "b",
            "cell": cell_large,
            "control": None,
            "rep": 9,
            "error": "HTTP 500",
        }
    )
    out = analyze.ladder_routing(rows)["r"]
    assert out["n"] == 12
    assert out["distinct_models"] == 2
    assert out["cell_test"]["p_value"] < 0.01
    assert out["length_control"]["test"]["p_value"] < 0.01  # padding alone moved the choice
    assert out["length_control"]["shares"]["padded"]["share"] == {"x/big": 1.0}
    assert out["errors"] == 1
    assert out["cost_reported_share"] == 1.0


def test_ladder_prompt_builder_on_a_suite_task(tmp_path: Path) -> None:
    suite = ROOT / "tasks" / "router-v1"
    if not (suite / "oss-py-m2-00" / "metadata.json").exists():
        pytest.skip("router-v1 suite not present")
    cells = {"oss-py-m2-00": {"language": "python", "scale": "small", "difficulty": "easy"}}
    prompt = ladder.build_prompt(suite / "oss-py-m2-00", cells["oss-py-m2-00"], 6000, tmp_path)
    assert prompt.task_id == "oss-py-m2-00"
    assert "# Issue" in prompt.text and "# Repository files" in prompt.text
    assert prompt.approx_tokens <= 6000
    assert any(name.endswith("service.py") for name in prompt.files)
    padded = ladder.build_prompt(
        suite / "oss-py-m2-00",
        cells["oss-py-m2-00"],
        2000,
        tmp_path,
        pad_from=suite / "py-txn-kvstore",
        control="padded",
    )
    assert padded.control == "padded"
    assert "Additional repository context" in padded.text
    assert padded.approx_tokens > prompt.approx_tokens


def test_router_spec_parsing(monkeypatch: pytest.MonkeyPatch) -> None:
    spec = ladder.RouterSpec.parse("openrouter=https://openrouter.ai/api/v1/=openrouter/auto")
    assert (
        spec.base_url == "https://openrouter.ai/api/v1" and spec.api_key_env == "OPENROUTER_API_KEY"
    )
    monkeypatch.setenv("ROUTER_NOT_DIAMOND_BASE_URL", "https://nd.example/v1")
    spec = ladder.RouterSpec.parse("not-diamond=router")
    assert spec.api_key_env == "ROUTER_NOT_DIAMOND_API_KEY" and spec.model == "router"
    with pytest.raises(SystemExit):
        ladder.RouterSpec.parse("bad")


def test_cards_render_from_analysis(tmp_path: Path) -> None:
    pytest.importorskip("matplotlib")
    cards = _load("make_router_cards")
    cell_small = {"language": "python", "scale": "small", "difficulty": "easy"}
    rows = [
        {
            "router": "r",
            "task_id": "a",
            "cell": cell_small,
            "control": None,
            "rep": i,
            "served_model": "openai/gpt-x" if i % 2 else "anthropic/claude-y",
            "prompt_tokens": 100,
            "reported_cost_usd": 0.001,
            "latency_s": 1.0,
        }
        for i in range(4)
    ]
    runs = [
        analyze.summary_row(
            _run(
                "a",
                "router",
                CELLS["a"],
                economics={
                    "billing_mode": "api-metered",
                    "cost_basis": "router-reported",
                    "marginal_cash_usd": 0.3,
                    "api_equivalent_cost_usd": None,
                },
            ),
            "router",
            CELLS,
        )
    ]
    analysis = analyze.analyze(rows, runs, {}, {})
    path = tmp_path / "analysis.json"
    path.write_text(json.dumps(analysis))
    cards.main([str(path), "--out-dir", str(tmp_path)])
    assert (tmp_path / "router_routing_card.png").stat().st_size > 10_000
    assert (tmp_path / "router_cost_card.png").stat().st_size > 10_000
