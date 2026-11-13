"""Code quality v3.30.4 and v3.30.5 (GPT-6.1 Sol on Claude Fable 5.1 and Claude Opus 5.5): offline checks only.

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
from harness import retrospective_judging as base

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
    "verify_frozen",
    "base",
)


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


def test_round_identities(load: Callable[[str], ModuleType]) -> None:
    m4, m5 = load("v3304"), load("v3305")
    v3302 = importlib.import_module("harness.maintenance_review_v3302")
    assert m4.PROTOCOL_ID == "code-quality-maintenance-v3.30.4"
    assert m5.PROTOCOL_ID == "code-quality-maintenance-v3.30.5"
    assert m4.SOURCE_OUT.name == "runs-code-quality-maintenance-v3.4"
    assert m5.SOURCE_OUT.name == "runs-code-quality-maintenance-v3.15"
    assert m4.COMPARISON.parent.name == "swe-v4-astra-fable51-2026-09"
    assert m5.COMPARISON.parent.name == "swe-v4-opus55-2026-09"
    for m in (m4, m5):
        assert m.VERDICTS["sol"] == (v3302.OUT, v3302.PROTOCOL_ID)
        assert m.VERDICTS["muse"] == (m.SOURCE_OUT, m.SOURCE_ID)
    assert m4.ROUND.source_runs == (m4.EARLIER_HOST, None)
    assert m5.ROUND.source_runs == (m4.EARLIER_HOST, m5.ROOT)


def test_binds_sol_and_the_muse_sibling(load: Callable[[str], ModuleType]) -> None:
    v327 = importlib.import_module("harness.maintenance_review_v327")
    m4 = load("v3304")
    assert v3.PANELS == ("sol",) and v3.SCORED_PANELS == ("sol", "muse")
    assert v3.SCORED_SIBLING_OUT == {"muse": m4.SOURCE_OUT}
    assert v3.call is v327.call and v3.verify_frozen is m4.verify_frozen
    m5 = load("v3305")
    assert v3.SCORED_SIBLING_OUT == {"muse": m5.SOURCE_OUT}
    assert v3.verify_frozen is m5.verify_frozen and v3.panel_passed is m5.panel_passed


def _row(ident: str, model: str) -> dict:
    return {"id": ident, "model": model}


def test_subset_keeps_one_model_and_drops_cross_model_pairs(
    load: Callable[[str], ModuleType],
) -> None:
    m = load("v3304")
    manifest = [_row("s1", "fable"), _row("s2", "astra"), _row("s3", "fable"), _row("s4", "astra")]
    signals = {r["id"]: {"x": r["id"]} for r in manifest}
    selection = {"repeats": ["s1", "s2"], "pairs": [["s2", "s1"], ["s1", "s3"], ["s4", "s2"]]}
    rows, kept_signals, kept = m.subset(manifest, signals, selection, "fable")
    assert rows == [manifest[0], manifest[2]]
    assert kept_signals == {"s1": {"x": "s1"}, "s3": {"x": "s3"}}
    assert kept == {"repeats": ["s1"], "pairs": [["s1", "s3"]]}
    with pytest.raises(ValueError, match="No opus rows"):
        m.subset(manifest, signals, selection, "opus")


def test_copy_subset_refuses_another_comparison(
    load: Callable[[str], ModuleType], tmp_path: Path
) -> None:
    m = load("v3304")
    source = tmp_path / "src"
    source.mkdir()
    (source / "protocol.json").write_text(json.dumps({"source_comparison_sha256": "0" * 64}))
    comparison = tmp_path / "comparison.json"
    comparison.write_text("{}")
    with pytest.raises(ValueError, match="differs from the record"):
        m.copy_subset(source, tmp_path / "out", comparison, "fable")
    assert not (tmp_path / "out").exists()


def _round(m: ModuleType, tmp_path: Path, local: Path | None) -> object:
    out = tmp_path / "out"
    out.mkdir(exist_ok=True)
    rows = [
        {
            "source_directory": f"{m.EARLIER_HOST}/runs-x/low/run-1",
            "source_hashes": {"summary": "s"},
        }
    ]
    (out / "private-manifest.json").write_text(json.dumps(rows))
    return m.Round("p", out, "src", tmp_path, tmp_path, {}, tmp_path, (m.EARLIER_HOST, local))


def test_relocated_sources(load: Callable[[str], ModuleType], tmp_path: Path, monkeypatch) -> None:
    m = load("v3304")
    seen = []
    monkeypatch.setattr(
        base, "inputs", lambda run, tasks: seen.append(run) or {"source_hashes": "fresh"}
    )
    lost = m._RelocatedSources(_round(m, tmp_path, None))
    assert lost.inputs(Path(f"{m.EARLIER_HOST}/runs-x/low/run-1"), tmp_path) == {
        "source_hashes": {"summary": "s"}
    }
    assert seen == []  # the runs are gone, so nothing is re-read
    restored = m._RelocatedSources(_round(m, tmp_path, tmp_path / "root"))
    assert restored.inputs(Path(f"{m.EARLIER_HOST}/runs-x/low/run-1"), tmp_path) == {
        "source_hashes": "fresh"
    }
    assert seen == [tmp_path / "root" / "runs-x/low/run-1"]
    assert restored.inputs(Path("/elsewhere/run-2"), tmp_path) == {"source_hashes": "fresh"}
    assert seen[-1] == Path("/elsewhere/run-2")
    assert restored.canonical is base.canonical  # everything else is the frozen module


def test_verify_frozen_for_leaves_other_protocols_alone(
    load: Callable[[str], ModuleType], tmp_path: Path, monkeypatch
) -> None:
    m = load("v3304")
    calls = []
    monkeypatch.setattr(m, "FROZEN_VERIFY", lambda: calls.append(v3.base) or {"id": "x"})
    v3.PROTOCOL_ID = "another"  # the load fixture restores it
    m.verify_frozen_for(_round(m, tmp_path, None))
    assert calls == [base]
    v3.PROTOCOL_ID = "p"
    m.verify_frozen_for(_round(m, tmp_path, None))
    assert isinstance(calls[-1], m._RelocatedSources) and v3.base is base


def test_doc_carries_the_amendments(load: Callable[[str], ModuleType]) -> None:
    m = load("v3304")
    text = m.SOURCE_DOC.read_text()
    for version in ("v3.30.4", "v3.30.5"):
        assert f"## Amendment {version}" in text
        assert f"code-quality-maintenance-{version}" in text
    assert chr(0x2014) not in text and chr(0x2013) not in text
