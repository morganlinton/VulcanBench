"""Code quality v3.30.1 to v3.30.3 (Claude Fable 5.2: Muse at Max, then GPT-6.1 Sol's seat): offline checks only.

Importing a round module rebinds the frozen v3 implementation, so every test
here imports it through a fixture that restores v3 afterwards.
"""

from __future__ import annotations

import importlib
import json
from collections.abc import Callable, Iterator
from pathlib import Path
from types import ModuleType

import pytest

from harness import maintenance_review_v3 as v3

REBOUND = (
    "OUT",
    "COMPARISON",
    "PROTOCOL_ID",
    "PANELS",
    "SCORED_SIBLING_OUT",
    "SCORED_PANELS",
    "SENSITIVITY_PANELS",
    "SENSITIVITY_OUT",
    "DOC",
    "call",
    "parse_stream_for",
    "prepare",
    "panel_passed",
)
CLI = "2.1.321 (Claude Code)"


@pytest.fixture
def load() -> Iterator[Callable[[str], ModuleType]]:
    saved = {name: getattr(v3, name) for name in REBOUND}

    def _load(name: str) -> ModuleType:
        module = importlib.import_module(f"harness.maintenance_review_{name}")
        module._bind()
        return module

    yield _load
    for name, value in saved.items():
        setattr(v3, name, value)


def _row(task: str, effort: str = "max", cli: str = CLI) -> dict:
    return {"model": "fable52", "effort": effort, "task": task, "solver_cli_version": cli}


def _verdict(root: Path, panel: str, protocol_id: str, passed: bool = True) -> None:
    root.mkdir(parents=True, exist_ok=True)
    (root / "protocol.json").write_text(json.dumps({"id": protocol_id}))
    gate = {"passed": passed, "protocol_sha256": v3.sha(root / "protocol.json")}
    (root / f"calibration-{panel}.json").write_text(json.dumps(gate))


def test_round_identities(load: Callable[[str], ModuleType]) -> None:
    m1, m2, m3 = load("v3301"), load("v3302"), load("v3303")
    assert m1.PROTOCOL_ID == "code-quality-maintenance-v3.30.1"
    assert m2.PROTOCOL_ID == "code-quality-maintenance-v3.30.2"
    assert m3.PROTOCOL_ID == "code-quality-maintenance-v3.30.3"
    assert m1.OUT.name == "runs-code-quality-maintenance-v3.30.1"
    assert m1.COMPARISON.name == m3.COMPARISON.name == "comparison-max.json"
    assert m2.COMPARISON.name == "comparison.json"
    assert m1.MODELS == m3.MODELS == {"fable52": ("max",)}
    assert m2.MODELS == {"fable52": ("low", "medium", "high", "extra-high")}
    assert m3.MAX_OUT == m1.OUT and m2.BASE_OUT == m1.BASE_OUT


def test_v3301_binds_muse_alone(load: Callable[[str], ModuleType]) -> None:
    m = load("v3301")
    assert v3.OUT == m.OUT and v3.prepare is m.prepare and v3.panel_passed is m.panel_passed
    assert v3.PANELS == v3.SCORED_PANELS == ("muse",)
    assert v3.SCORED_SIBLING_OUT == {}
    assert m.VERDICTS == {"muse": (m.BASE_OUT, "code-quality-maintenance-v3.30")}


def test_sol_rounds_bind_the_codex_seat_and_muse_sibling(load: Callable[[str], ModuleType]) -> None:
    v327 = importlib.import_module("harness.maintenance_review_v327")
    m2 = load("v3302")
    assert v3.PANELS == ("sol",) and v3.SCORED_PANELS == ("sol", "muse")
    assert v3.SCORED_SIBLING_OUT == {"muse": m2.BASE_OUT}
    assert v3.call is v327.call and v3.parse_stream_for is v327.parse_stream_for
    m3 = load("v3303")
    assert v3.SCORED_SIBLING_OUT == {"muse": m3.MAX_OUT}
    assert v3.panel_passed is m3.panel_passed
    assert m3.VERDICTS["sol"] == (m2.OUT, m2.PROTOCOL_ID)
    assert m3.VERDICTS["muse"] == (m2.BASE_OUT, m2.BASE_ID)


def test_verdict_passed(load: Callable[[str], ModuleType], tmp_path: Path) -> None:
    m = load("v3302")
    _verdict(tmp_path / "a", "muse", "p-a")
    _verdict(tmp_path / "b", "sol", "p-b", passed=False)
    verdicts = {"muse": (tmp_path / "a", "p-a"), "sol": (tmp_path / "b", "p-b")}
    assert m.verdict_passed(verdicts, "muse")
    assert not m.verdict_passed(verdicts, "sol")
    assert not m.verdict_passed(verdicts, "grok")
    assert not m.verdict_passed({"muse": (tmp_path / "a", "another-id")}, "muse")
    (tmp_path / "a" / "protocol.json").write_text(json.dumps({"id": "p-a", "edited": True}))
    assert not m.verdict_passed(verdicts, "muse")  # the verdict no longer binds the protocol


def test_v3301_population_is_one_new_level(load: Callable[[str], ModuleType]) -> None:
    m = load("v3301")
    rows = [_row(f"t{i:02d}") for i in range(23)]
    assert m.check_population({"rows": rows}, []) == {("fable52", "max"): 23}
    gaps = {
        "rows": rows[:21],
        "excluded": [{**_row(f"t{i:02d}"), "reason": "x"} for i in (21, 22)],
    }
    assert m.check_population(gaps, []) == {("fable52", "max"): 21}
    with pytest.raises(ValueError, match="not 23"):
        m.check_population({"rows": rows[:22]}, [])
    with pytest.raises(ValueError, match="needs a reason"):
        m.check_population({"rows": rows[:22], "missing": [_row("t22")]}, [])
    with pytest.raises(ValueError, match="already judged"):
        m.check_population({"rows": rows}, [_row("t05")])
    with pytest.raises(ValueError, match="expected only"):
        m.check_population({"rows": [*rows[:22], _row("t22", "high")]}, [])
    with pytest.raises(ValueError, match="Claude Code"):
        m.check_population({"rows": [*rows[:22], _row("t22", cli="2.1.307 (Claude Code)")]}, [])


def test_copy_freeze_refuses_another_comparison(
    load: Callable[[str], ModuleType], tmp_path: Path
) -> None:
    m = load("v3302")
    source = tmp_path / "src"
    source.mkdir()
    (source / "protocol.json").write_text(json.dumps({"source_comparison_sha256": "0" * 64}))
    comparison = tmp_path / "comparison.json"
    comparison.write_text("{}")
    with pytest.raises(ValueError, match="differs from the record"):
        m.copy_freeze(source, tmp_path / "out", comparison)
    assert not (tmp_path / "out").exists()


def test_sol_reviewer_is_neutral_and_pinned(load: Callable[[str], ModuleType]) -> None:
    m = load("v3302")
    v327 = importlib.import_module("harness.maintenance_review_v327")
    codex = {
        "version": "0.159.0",
        "launcher": {"path": "/x/codex.js", "sha256": "a"},
        "native": {"path": "/x/codex", "sha256": "b"},
    }
    original = v327.pinned_codex
    v327.pinned_codex = lambda: codex
    try:
        settings, pins = m.sol_reviewer()
    finally:
        v327.pinned_codex = original
    assert pins == codex
    assert settings["model"] == "gpt-6.1-sol" and settings["lab"] == "OpenAI"
    assert settings["neutrality"] == "neutral for Anthropic submissions"
    assert "v3.30" in settings["seat"]


def test_doc_carries_the_amendments(load: Callable[[str], ModuleType]) -> None:
    m = load("v3301")
    text = m.SOURCE_DOC.read_text()
    for version in ("v3.30.1", "v3.30.2", "v3.30.3"):
        assert f"## Amendment {version}" in text
        assert f"code-quality-maintenance-{version}" in text
    assert chr(0x2014) not in text and chr(0x2013) not in text
