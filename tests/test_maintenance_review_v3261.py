"""Code quality v3.26.1 (Muse Wonder Full Medium top-up under the v3.26 judges): offline checks only.

Importing a round module rebinds the frozen v3 implementation, so every test
here imports it through a fixture that restores v3 afterwards.
"""

from __future__ import annotations

import importlib
import json
from collections.abc import Iterator
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


@pytest.fixture
def isolate_v3() -> Iterator[None]:
    saved = {name: getattr(v3, name) for name in REBOUND}
    yield
    for name, value in saved.items():
        setattr(v3, name, value)


@pytest.fixture
def m(isolate_v3: None) -> ModuleType:
    module = importlib.import_module("harness.maintenance_review_v3261")
    module._bind()
    return module


def _write(path: Path, data: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data))


def _row(task: str) -> dict:
    return {"model": "musewonderfull", "effort": "medium", "task": task}


def test_round_identity(m: ModuleType) -> None:
    assert m.PROTOCOL_ID == "code-quality-maintenance-v3.26.1"
    assert m.OUT.name == "runs-code-quality-maintenance-v3.26.1"
    assert m.BASE_OUT.name == "runs-code-quality-maintenance-v3.26"
    assert m.SOL_OUT.name == "runs-code-quality-maintenance-v3.25"
    assert m.COMPARISON.name == "comparison-topup.json"
    assert m.MODELS == {"musewonderfull": ("medium",)}


def test_bind_rebinds_the_frozen_core(m: ModuleType) -> None:
    assert v3.OUT == m.OUT
    assert v3.PANELS == v3.SCORED_PANELS == ("claude", "sol")
    assert v3.SCORED_SIBLING_OUT == {}
    assert v3.panel_passed is m.panel_passed
    assert v3.call is m.call and v3.prepare is m.prepare


def test_each_judge_is_gated_by_its_own_round(m: ModuleType, tmp_path: Path, monkeypatch) -> None:
    claude_root, sol_root = tmp_path / "v3.26", tmp_path / "v3.25"
    for root, pid, panel in (
        (claude_root, m.BASE_ID, "claude"),
        (sol_root, m.SOL_ID, "sol"),
    ):
        _write(root / "protocol.json", {"id": pid})
        _write(
            root / f"calibration-{panel}.json",
            {"passed": True, "protocol_sha256": v3.sha(root / "protocol.json")},
        )
    monkeypatch.setattr(
        m, "VERDICTS", {"claude": (claude_root, m.BASE_ID), "sol": (sol_root, m.SOL_ID)}
    )
    assert m.panel_passed("claude") and m.panel_passed("sol")
    assert not m.panel_passed("grok")
    # A verdict taken under a changed protocol no longer gates.
    _write(sol_root / "protocol.json", {"id": m.SOL_ID, "changed": True})
    assert not m.panel_passed("sol")
    # A failed exam never gates.
    _write(
        claude_root / "calibration-claude.json",
        {"passed": False, "protocol_sha256": v3.sha(claude_root / "protocol.json")},
    )
    assert not m.panel_passed("claude")


def test_population_only_adds_frozen_missing_runs(m: ModuleType) -> None:
    base = [_row(f"t{i:02d}") for i in range(21)]
    frozen = [_row("cellar"), _row("paddock")]
    both = {"rows": [_row("cellar"), _row("paddock")]}
    assert m.check_population(both, base, frozen) == {("musewonderfull", "medium"): 2}
    one = {"rows": [_row("cellar")], "missing": [{**_row("paddock"), "reason": "not yet run"}]}
    assert m.check_population(one, base, frozen) == {("musewonderfull", "medium"): 1}
    with pytest.raises(ValueError, match="already judged"):
        m.check_population({"rows": [_row("t03")]}, base, [*frozen, _row("t03")])
    with pytest.raises(ValueError, match="not recorded missing"):
        m.check_population({"rows": [_row("other")]}, base, frozen)
    with pytest.raises(ValueError, match="not 23"):
        m.check_population({"rows": [_row("cellar")]}, base, frozen)
    low = {"rows": [{**_row("cellar"), "effort": "low"}]}
    with pytest.raises(ValueError, match="Unexpected cells"):
        m.check_population(low, base, frozen)


def test_sol_routes_through_codex_and_claude_through_the_frozen_call(
    m: ModuleType, tmp_path: Path, monkeypatch
) -> None:
    seen = []
    monkeypatch.setattr(m, "_frozen_call", lambda panel, *a: seen.append(("frozen", panel)) or {})
    m.call("claude", "primary", "x", "review", {}, {})
    assert seen == [("frozen", "claude")]

    monkeypatch.setattr(v3, "OUT", tmp_path)
    _write(tmp_path / "protocol.json", {"id": m.PROTOCOL_ID})
    monkeypatch.setattr(v3, "prompt", lambda kind, payload: "prompt")
    monkeypatch.setattr(v3, "validate", lambda kind, vote, payload: None)

    def codex_vote(text, folder, name, settings, schema):
        seen.append(("codex", settings["model"]))
        return {"score": 1}

    monkeypatch.setattr(v3, "codex_vote", codex_vote)
    vote = m.call(
        "sol", "probe", "x", "probe", {}, {"reviewers": {"sol": {"model": "gpt-6.1-sol"}}}
    )
    assert seen[-1] == ("codex", "gpt-6.1-sol") and vote["status"] == "complete"
    assert (tmp_path / "calls/sol/probe/x/selected.json").exists()


def test_sol_stream_uses_the_codex_parser(m: ModuleType, monkeypatch) -> None:
    monkeypatch.setattr(v3, "parse_codex_stream", lambda text: {"parser": "codex"})
    assert m.parse_stream_for("sol", "") == {"parser": "codex"}


def test_doc_carries_the_amendment(m: ModuleType) -> None:
    text = m.SOURCE_DOC.read_text()
    assert "## Amendment v3.26.1" in text and "code-quality-maintenance-v3.26.1" in text
    assert chr(0x2014) not in text and chr(0x2013) not in text
