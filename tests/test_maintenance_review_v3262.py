"""Code quality v3.26.2 (Muse Wonder Full at High under the v3.26 judges): offline checks only.

Importing a round module rebinds the frozen v3 implementation, so every test
here imports it through a fixture that restores v3 afterwards.
"""

from __future__ import annotations

import importlib
from collections.abc import Iterator
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
V149 = "Muse Code 1.4.9 (1.4.9-R6297.1)"


@pytest.fixture
def isolate_v3() -> Iterator[None]:
    saved = {name: getattr(v3, name) for name in REBOUND}
    yield
    for name, value in saved.items():
        setattr(v3, name, value)


@pytest.fixture
def m(isolate_v3: None) -> ModuleType:
    module = importlib.import_module("harness.maintenance_review_v3262")
    module._bind()
    return module


def _row(task: str, effort: str = "high", cli: str = V149) -> dict:
    return {"model": "musewonderfull", "effort": effort, "task": task, "solver_cli_version": cli}


def test_round_identity(m: ModuleType) -> None:
    assert m.PROTOCOL_ID == "code-quality-maintenance-v3.26.2"
    assert m.OUT.name == "runs-code-quality-maintenance-v3.26.2"
    assert m.COMPARISON.name == "comparison-high.json"
    assert m.MODELS == {"musewonderfull": ("high",)}
    assert [r.name for r in m.JUDGED] == [
        "runs-code-quality-maintenance-v3.26",
        "runs-code-quality-maintenance-v3.26.1",
    ]


def test_bind_reuses_the_v3261_gates_and_routing(m: ModuleType) -> None:
    v3261 = importlib.import_module("harness.maintenance_review_v3261")
    assert v3.OUT == m.OUT and v3.PROTOCOL_ID == m.PROTOCOL_ID
    assert v3.prepare is m.prepare
    assert v3.panel_passed is v3261.panel_passed
    assert v3.call is v3261.call and v3.parse_stream_for is v3261.parse_stream_for
    assert v3.PANELS == v3.SCORED_PANELS == ("claude", "sol")


def test_population_is_one_new_level_on_149(m: ModuleType) -> None:
    rows = [_row(f"t{i:02d}") for i in range(23)]
    assert m.check_population({"rows": rows}, []) == {("musewonderfull", "high"): 23}
    gaps = {
        "rows": rows[:20],
        "missing": [{**_row(f"t{i:02d}"), "reason": "x"} for i in range(20, 23)],
    }
    assert m.check_population(gaps, []) == {("musewonderfull", "high"): 20}
    with pytest.raises(ValueError, match="not 23"):
        m.check_population({"rows": rows[:20]}, [])
    with pytest.raises(ValueError, match="needs a reason"):
        m.check_population({"rows": rows[:22], "missing": [_row("t22")]}, [])
    with pytest.raises(ValueError, match="already judged"):
        m.check_population({"rows": rows}, [_row("t05")])
    with pytest.raises(ValueError, match="expected only"):
        m.check_population({"rows": [*rows[:22], _row("t22", "medium")]}, [])
    with pytest.raises(ValueError, match=r"Muse Code 1\.4\.9"):
        m.check_population(
            {"rows": [*rows[:22], _row("t22", cli="Muse Code 1.4.6 (1.4.6-R6000.1)")]}, []
        )


def test_doc_carries_the_amendment(m: ModuleType) -> None:
    text = m.SOURCE_DOC.read_text()
    assert "## Amendment v3.26.2" in text and "code-quality-maintenance-v3.26.2" in text
    assert chr(0x2014) not in text and chr(0x2013) not in text
