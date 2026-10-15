"""Code quality v3.26 (Opus 5.5 replaces the Grok 4.7 seat for Muse Wonder Full): offline checks only.

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
)


@pytest.fixture
def isolate_v3() -> Iterator[None]:
    saved = {name: getattr(v3, name) for name in REBOUND}
    yield
    for name, value in saved.items():
        setattr(v3, name, value)


@pytest.fixture
def m(isolate_v3: None) -> ModuleType:
    module = importlib.import_module("harness.maintenance_review_v326")
    module._bind()
    return module


def _write(path: Path, data: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data))


def test_round_identity_and_population(m: ModuleType) -> None:
    assert m.PROTOCOL_ID == "code-quality-maintenance-v3.26"
    assert m.OUT.name == "runs-code-quality-maintenance-v3.26"
    assert m.SOL_OUT.name == "runs-code-quality-maintenance-v3.25"
    assert m.COMPARISON.name == "comparison.json"
    assert m.MODELS == {"musewonderfull": ("low", "medium")}


def test_opus_seat_with_sol_as_scored_sibling(m: ModuleType) -> None:
    assert m.PANELS == ("claude",)
    assert m.SCORED_PANELS == ("claude", "sol")
    assert m.SCORED_SIBLING_OUT == {"sol": m.SOL_OUT}
    assert "muse" in m.EXCLUDED_JUDGES and "grok" in m.REPLACED_SEATS


def test_bind_rebinds_the_frozen_core_and_keeps_the_frozen_call(m: ModuleType) -> None:
    assert v3.OUT == m.OUT
    assert v3.PANELS == ("claude",)
    assert v3.SCORED_PANELS == ("claude", "sol")
    assert v3.panel_root("sol") == m.SOL_OUT
    assert v3.panel_root("claude") == m.OUT
    assert v3.SENSITIVITY_PANELS == ()
    assert v3.prepare is m.prepare
    # The Claude seat runs on v3's own call, which picks claude_vote for this panel.
    assert v3.call.__module__ == "harness.maintenance_review_v3"


def test_bind_refuses_a_muse_panel(m: ModuleType, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(m, "SCORED_PANELS", ("claude", "muse"))
    with pytest.raises(ValueError, match="excluded"):
        m._bind()


def test_opus_55_settings(m: ModuleType) -> None:
    pin = {"path": "/x/claude", "version": "2.1.300", "sha256": "ab"}
    s = m.claude_settings(pin)
    assert s["model"] == "claude-opus-5-5"
    assert s["effort"] == "medium"
    assert s["claude"] == "/x/claude"
    assert s["lab"] == "Anthropic" != m.SUBMISSION_LAB
    assert s["timeout"] == 900
    # The frozen transport sends no zero-thinking budget unless a reader asks for it.
    assert "extended_thinking" not in s


def test_claude_pin_is_a_copied_binary_checked_by_sha(m: ModuleType, tmp_path: Path) -> None:
    assert m.CLAUDE.parent.name == "claude-2.1.300"
    assert ".local/share/vulcanbench" in str(m.CLAUDE)
    assert "node_modules" not in str(m.CLAUDE)
    with pytest.raises(ValueError, match="not found"):
        m.pinned_claude(tmp_path / "missing")
    other = tmp_path / "claude"
    other.write_text("not the pinned build")
    with pytest.raises(ValueError, match="pinned sha256"):
        m.pinned_claude(other)


def test_sibling_record_requires_v325_and_a_passing_sol_exam(m: ModuleType, tmp_path: Path) -> None:
    root = tmp_path / "v3.25"
    _write(
        root / "protocol.json",
        {"id": "code-quality-maintenance-v3.25", "reviewers": {"sol": {"model": "gpt-6.1-sol"}}},
    )
    _write(
        root / "calibration-sol.json",
        {"passed": True, "protocol_sha256": v3.sha(root / "protocol.json")},
    )
    record = m.sibling_record(root)
    assert record["model"] == "gpt-6.1-sol"
    assert record["protocol_sha256"] == v3.sha(root / "protocol.json")

    _write(
        root / "calibration-sol.json",
        {"passed": False, "protocol_sha256": v3.sha(root / "protocol.json")},
    )
    with pytest.raises(ValueError, match="no passing exam"):
        m.sibling_record(root)
    _write(root / "calibration-sol.json", {"passed": True, "protocol_sha256": "stale"})
    with pytest.raises(ValueError, match="no passing exam"):
        m.sibling_record(root)
    _write(root / "protocol.json", {"id": "code-quality-maintenance-v3.24", "reviewers": {}})
    with pytest.raises(ValueError, match="not the"):
        m.sibling_record(root)


def test_inputs_must_be_byte_identical_to_the_sibling(m: ModuleType, tmp_path: Path) -> None:
    out, sibling = tmp_path / "out", tmp_path / "sibling"
    for root in (out, sibling):
        for name in m.SHARED_FILES:
            _write(root / name, {"same": True})
        for folder in ("evidence", "controls", "quirk-keys"):
            _write(root / folder / "a.json", {"same": True})
    m.require_same_inputs(out, sibling)

    _write(out / "evidence" / "a.json", {"binary_manifest": {"__pycache__/x.pyc": {}}})
    with pytest.raises(ValueError, match="evidence/ differs"):
        m.require_same_inputs(out, sibling)
    _write(out / "evidence" / "a.json", {"same": True})
    _write(out / "private-manifest.json", {"same": False})
    with pytest.raises(ValueError, match=r"private-manifest\.json differs"):
        m.require_same_inputs(out, sibling)


def test_protocol_fields_must_match_the_sibling(m: ModuleType) -> None:
    base_protocol = {field: field for field in m.SHARED_PROTOCOL_FIELDS}
    m.require_same_protocol(dict(base_protocol), base_protocol)
    for field in ("rubric", "weights", "population", "quirk_key_hashes"):
        changed = {**base_protocol, field: "other"}
        with pytest.raises(ValueError, match=field):
            m.require_same_protocol(changed, base_protocol)


def test_doc_is_not_byte_compared_but_carries_the_amendment(m: ModuleType) -> None:
    assert "protocol-document.md" not in m.SHARED_FILES
    text = m.SOURCE_DOC.read_text()
    assert "## Amendment v3.26" in text and "code-quality-maintenance-v3.26" in text
    assert m.CLAUDE_SHA256 in text
    assert chr(0x2014) not in text and chr(0x2013) not in text


def test_population_check_matches_v325(m: ModuleType) -> None:
    record = json.loads(m.COMPARISON.read_text())
    _rows, excluded, missing, counts = m.check_population(record)
    assert dict(counts) == {("musewonderfull", "low"): 23, ("musewonderfull", "medium"): 21}
    assert len(missing) == 2 and not excluded


def test_top_up_is_v3261_gated_by_both_verdicts(m: ModuleType) -> None:
    assert m.TOP_UP["round"] == "v3.26.1"
    assert m.TOP_UP["directory"] == "runs-code-quality-maintenance-v3.26.1"
    assert "Opus verdict" in m.TOP_UP["rule"] and "Sol verdict" in m.TOP_UP["rule"]


@pytest.mark.skipif(
    not (v3.ROOT / "runs-code-quality-maintenance-v3.26/protocol.json").exists(),
    reason="the v3.26 record is not in this checkout",
)
def test_committed_record_names_the_sibling(m: ModuleType) -> None:
    protocol = v3.read(m.OUT / "protocol.json")
    assert protocol["scored_siblings"]["sol"]["protocol_id"] == "code-quality-maintenance-v3.25"
    assert protocol["reviewers"]["claude"]["model"] == "claude-opus-5-5"
