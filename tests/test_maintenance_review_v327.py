"""Code quality v3.27 (DeepSeek V4.1 Flash judged by Muse Spark 1.3 and GPT-6.1 Sol): offline checks only.

Importing a round module rebinds the frozen v3 implementation, so every test
here imports it through a fixture that restores v3 afterwards.
"""

from __future__ import annotations

import importlib
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
)
TASKS = [f"legacy-t{i:02d}" for i in range(23)]


@pytest.fixture
def isolate_v3() -> Iterator[None]:
    saved = {name: getattr(v3, name) for name in REBOUND}
    yield
    for name, value in saved.items():
        setattr(v3, name, value)


@pytest.fixture
def m(isolate_v3: None) -> ModuleType:
    module = importlib.import_module("harness.maintenance_review_v327")
    module._bind()
    return module


def _row(task: str, effort: str, version: str = "0.2.1-rc.2") -> dict:
    return {"model": "dsflash", "effort": effort, "task": task, "solver_cli_version": version}


def _record(**gaps: tuple[str, ...]) -> dict:
    rows = [_row(t, e) for e in ("low", "high", "max") for t in TASKS if t not in gaps.get(e, ())]
    missing = [{**_row(t, e), "reason": "no run"} for e, ts in gaps.items() for t in ts]
    return {"rows": rows, "excluded": [], "missing": missing}


def test_round_identity(m: ModuleType) -> None:
    assert m.PROTOCOL_ID == "code-quality-maintenance-v3.27"
    assert m.OUT.name == "runs-code-quality-maintenance-v3.27"
    assert m.COMPARISON.parent.name == "swe-v4-deepseek-flash-2026-10"
    assert m.MODELS == {"dsflash": ("low", "high", "max")}
    assert m.PANELS == ("muse", "sol")


def test_bind_rebinds_the_frozen_core(m: ModuleType) -> None:
    assert v3.OUT == m.OUT and v3.PROTOCOL_ID == m.PROTOCOL_ID
    assert v3.PANELS == v3.SCORED_PANELS == ("muse", "sol")
    assert v3.call is m.call and v3.parse_stream_for is m.parse_stream_for
    assert v3.prepare is m.prepare
    assert v3.SCORED_SIBLING_OUT == {} and v3.SENSITIVITY_PANELS == ()


def test_sol_seat_and_codex_pins(m: ModuleType, tmp_path: Path) -> None:
    codex = {
        "launcher": {"path": "/x/codex.js", "sha256": m.CODEX_SHA256},
        "native": {"path": "/x/codex", "sha256": m.CODEX_NATIVE_SHA256},
    }
    sol = m.sol_settings(codex)
    assert sol["model"] == "gpt-6.1-sol" and sol["effort"] == "medium"
    assert sol["codex_version"] == "0.159.0" and sol["lab"] == "OpenAI"
    launcher, native = tmp_path / "codex.js", tmp_path / "codex"
    launcher.write_text("x")
    native.write_text("y")
    with pytest.raises(ValueError, match="differs from its pinned sha256"):
        m.pinned_codex(launcher, native)
    with pytest.raises(ValueError, match="not found"):
        m.pinned_codex(tmp_path / "absent.js", native)


def test_muse_panel_uses_the_frozen_call_and_sol_the_codex_parser(
    m: ModuleType, monkeypatch: pytest.MonkeyPatch
) -> None:
    seen = []
    monkeypatch.setattr(m, "_frozen_call", lambda *args: seen.append(args[0]) or {"ok": True})
    assert m.call("muse", "run", "x", "review", {}, {}) == {"ok": True}
    assert seen == ["muse"]
    monkeypatch.setattr(v3, "parse_codex_stream", lambda text: {"codex": text})
    assert m.parse_stream_for("sol", "s") == {"codex": "s"}


def test_population_check(m: ModuleType) -> None:
    rows, _, _, counts = m.check_population(_record())
    assert len(rows) == 69 and set(counts.values()) == {23}
    m.check_population(_record(max=("legacy-t05",)))
    bad = _record()
    bad["missing"] = [{**_row("legacy-t05", "max")}]
    bad["rows"] = [
        r for r in bad["rows"] if not (r["effort"] == "max" and r["task"] == "legacy-t05")
    ]
    with pytest.raises(ValueError, match="needs a reason"):
        m.check_population(bad)
    short = _record()
    short["rows"] = short["rows"][1:]
    with pytest.raises(ValueError, match="not 23"):
        m.check_population(short)
    wrong = _record()
    wrong["rows"][0]["solver_cli_version"] = "0.2.0"
    with pytest.raises(ValueError, match=r"0\.2\.1-rc\.2"):
        m.check_population(wrong)


def test_diagnostic_selection_pairs_each_level_with_the_next(m: ModuleType) -> None:
    manifest = [{"id": f"s-{e}-{t}", **_row(t, e)} for e in ("low", "high", "max") for t in TASKS]
    selection = m.diagnostic_selection(manifest, set(TASKS))
    assert len(selection["repeats"]) == 3
    assert len(selection["pairs"]) == 3
    levels = [tuple(i.split("-")[1] for i in pair) for pair in selection["pairs"]]
    assert levels == [("low", "high"), ("high", "max"), ("max", "low")]
    for a, b in selection["pairs"]:
        assert a.split("-", 2)[2] == b.split("-", 2)[2]


def test_doc_carries_the_amendment(m: ModuleType) -> None:
    text = m.SOURCE_DOC.read_text()
    assert "## Amendment v3.27" in text and "code-quality-maintenance-v3.27" in text
    assert chr(0x2014) not in text and chr(0x2013) not in text
