"""Code quality v3.25 (Muse Wonder Full judged by Grok 4.7 and GPT-6.1 Sol): offline checks only.

Importing a round module rebinds the frozen v3 implementation, so every test
here imports it through a fixture that restores v3 afterwards; the v3.4 roster
test in test_maintenance_review_v3.py must still see v3's own constants.
"""

from __future__ import annotations

import hashlib
import importlib
import json
import sys
from collections.abc import Iterator
from pathlib import Path
from types import ModuleType

import pytest

from harness import maintenance_review_v3 as v3

ROOT = Path(__file__).resolve().parents[1]
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
LOW_TASKS = [f"legacy-t{i:02d}" for i in range(23)]


@pytest.fixture
def isolate_v3() -> Iterator[None]:
    saved = {name: getattr(v3, name) for name in REBOUND}
    yield
    for name, value in saved.items():
        setattr(v3, name, value)


@pytest.fixture
def m(isolate_v3: None) -> ModuleType:
    module = importlib.import_module("harness.maintenance_review_v325")
    module._bind()
    return module


def _row(task: str, effort: str) -> dict:
    return {"model": "musewonderfull", "effort": effort, "task": task}


def _record(medium_missing: tuple[str, ...] = ("legacy-t03", "legacy-t07")) -> dict:
    rows = [_row(t, "low") for t in LOW_TASKS]
    rows += [_row(t, "medium") for t in LOW_TASKS if t not in medium_missing]
    missing = [{**_row(t, "medium"), "reason": "quota"} for t in medium_missing]
    return {"rows": rows, "excluded": [], "missing": missing}


# --- roster, judges and exclusion of Muse -------------------------------------


def test_round_identity_and_population_constants(m: ModuleType) -> None:
    assert m.PROTOCOL_ID == "code-quality-maintenance-v3.25"
    assert m.OUT == ROOT / "runs-code-quality-maintenance-v3.25"
    assert m.COMPARISON == ROOT / "docs/results/swe-v4-musewonder-full-2026-10/comparison.json"
    assert m.MODELS == {"musewonderfull": ("low", "medium")}


def test_panels_are_grok_and_sol_and_muse_is_excluded(m: ModuleType) -> None:
    assert m.PANELS == ("grok", "sol")
    assert "muse" not in m.PANELS and "muse" in m.EXCLUDED_JUDGES
    assert "not neutral for a Meta submission" in m.EXCLUDED_JUDGES["muse"]
    assert "2026-10-15" in m.EXCLUDED_JUDGES["muse"]


def test_bind_rebinds_the_frozen_core(m: ModuleType) -> None:
    assert v3.OUT == m.OUT and v3.COMPARISON == m.COMPARISON
    assert v3.PROTOCOL_ID == m.PROTOCOL_ID
    assert v3.PANELS == ("grok", "sol") and v3.SCORED_PANELS == ("grok", "sol")
    assert v3.SENSITIVITY_PANELS == () and v3.SCORED_SIBLING_OUT == {}
    assert v3.DOC == m.OUT / "protocol-document.md"
    assert v3.call is m.call and v3.prepare is m.prepare
    assert v3.parse_stream_for is m.parse_stream_for


def test_bind_refuses_a_muse_panel(m: ModuleType, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(m, "PANELS", ("muse", "sol"))
    with pytest.raises(ValueError, match="excluded"):
        m._bind()


def test_cli_rejects_the_muse_panel(m: ModuleType, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(sys, "argv", ["v325", "calibrate", "--panel", "muse"])
    with pytest.raises(SystemExit):
        m.main()


def test_grok_47_settings_and_two_space_display_name(m: ModuleType) -> None:
    settings = m.grok_settings({"launcher": "/x/2026.10.01-e373342/cursor-agent"})
    assert settings["model"] == "grok-4.7-medium"
    assert (
        settings["display_name"] == "Grok 4.7 256K Medium"
    )  # the label Cursor reports at run time
    assert settings["effort"] == "medium" and settings["lab"] == "xAI"
    assert settings["cursor"] == "/x/2026.10.01-e373342/cursor-agent"
    assert "first time" in settings["seat"]


def test_cursor_transport_accepts_only_the_exact_display_name(
    m: ModuleType, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    settings = m.grok_settings({"launcher": "/x/cursor-agent"})

    def fake_popen(label: str) -> type:
        stream = "\n".join(
            json.dumps(e)
            for e in (
                {"type": "system", "subtype": "init", "model": label, "apiKeySource": "login"},
                {
                    "type": "result",
                    "subtype": "success",
                    "is_error": False,
                    "result": '{"answers": []}',
                    "session_id": "c1",
                    "usage": {"inputTokens": 9, "outputTokens": 3},
                },
            )
        )

        class Proc:
            returncode = 0
            pid = 0

            def __init__(self, argv: list[str], **_: object) -> None:
                assert argv[argv.index("--model") + 1] == "grok-4.7-medium"

            def communicate(self, *_: object, **__: object) -> tuple[str, str]:
                return stream, ""

        return Proc

    monkeypatch.setattr(v3.subprocess, "Popen", fake_popen("Grok 4.7 256K Medium"))
    vote = v3.cursor_vote("text", tmp_path, "a1", settings, {})
    assert vote["model_reported"] == "Grok 4.7 256K Medium"
    monkeypatch.setattr(v3.subprocess, "Popen", fake_popen("Grok 4.7  Medium"))
    with pytest.raises(RuntimeError, match="Judge model changed"):
        v3.cursor_vote("text", tmp_path, "a0", settings, {})
    monkeypatch.setattr(v3.subprocess, "Popen", fake_popen("Grok 4.7 Medium"))
    with pytest.raises(RuntimeError, match="Judge model changed"):
        v3.cursor_vote("text", tmp_path, "a2", settings, {})


def test_sol_seat_settings(m: ModuleType) -> None:
    codex = {
        "launcher": {"path": "/c/codex.js", "sha256": m.CODEX_SHA256},
        "native": {"path": "/c/codex", "sha256": m.CODEX_NATIVE_SHA256},
    }
    settings = m.sol_settings(codex)
    assert settings["model"] == "gpt-6.1-sol" and settings["effort"] == "medium"
    assert settings["codex"] == "/c/codex.js" and settings["codex_version"] == "0.159.0"
    assert settings["binary_sha256"] == m.CODEX_SHA256
    assert settings["native_binary_sha256"] == m.CODEX_NATIVE_SHA256
    assert settings["lab"] == "OpenAI"


# --- pins ---------------------------------------------------------------------


def test_codex_paths_and_hashes_are_the_reinstalled_0159(m: ModuleType) -> None:
    assert m.CODEX_SHA256 == "61b0194f3bb6534439c8d26a3ed57d0805f84b884588b761795323eeb92fcf70"
    assert (
        m.CODEX_NATIVE_SHA256 == "e89718aa1969bfc4a471277bdc4679a3a3529293de0a309909822dfd67ddb77a"
    )
    assert str(m.CODEX).endswith(
        ".local/share/vulcanbench/codex-0.159.0/node_modules/@openai/codex/bin/codex.js"
    )
    assert str(m.CODEX_NATIVE).endswith(
        "codex-0.159.0/node_modules/@openai/codex-darwin-arm64/vendor/aarch64-apple-darwin/bin/codex"
    )
    assert "morganlinton" not in str(m.CODEX) + str(m.CODEX_NATIVE)


def test_codex_double_pin(m: ModuleType, tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    launcher, native = tmp_path / "codex.js", tmp_path / "codex"
    launcher.write_text("launcher")
    native.write_bytes(b"native")
    monkeypatch.setattr(m, "CODEX_SHA256", hashlib.sha256(b"launcher").hexdigest())
    monkeypatch.setattr(m, "CODEX_NATIVE_SHA256", hashlib.sha256(b"native").hexdigest())
    record = m.pinned_codex(launcher, native)
    assert record["launcher"]["path"] == str(launcher)
    assert record["native"]["sha256"] == hashlib.sha256(b"native").hexdigest()
    native.write_bytes(b"patched")
    with pytest.raises(ValueError, match="native"):
        m.pinned_codex(launcher, native)
    with pytest.raises(ValueError, match="launcher not found"):
        m.pinned_codex(tmp_path / "absent.js", native)


def _cursor_tree(root: Path, version: str) -> Path:
    version_dir = root / "versions" / version
    (version_dir / "lib").mkdir(parents=True)
    (version_dir / "cursor-agent").write_text("#!/bin/sh\n")
    (version_dir / "lib" / "index.js").write_text("console.log(1)\n")
    (version_dir / ".running").mkdir()
    (version_dir / ".running" / "lock").write_text("1")
    link = root / "cursor-agent"
    link.symlink_to(version_dir / "cursor-agent")
    return link


def test_cursor_pin_by_version_directory(
    m: ModuleType, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    assert m.CURSOR_VERSION == "2026.10.01-e373342"
    assert m.CURSOR_DIGEST == "dedbb877a8405b20e38a3dc32f38bf614bcd96d6e557f73eb6e99ff8c87b8a73"
    link = _cursor_tree(tmp_path, m.CURSOR_VERSION)
    version_dir = link.resolve().parent
    monkeypatch.setattr(m, "CURSOR_DIGEST", m.cursor_version_digest(version_dir))
    record = m.pinned_cursor(link)
    assert record["version"] == m.CURSOR_VERSION and record["same_as"].endswith("v3.24")
    (version_dir / ".running" / "lock").write_text("2")  # transient lock folder is skipped
    assert m.pinned_cursor(link)["digest"] == record["digest"]
    (version_dir / "lib" / "index.js").write_text("console.log(2)\n")
    with pytest.raises(ValueError, match="differs from the pin"):
        m.pinned_cursor(link)
    other = _cursor_tree(tmp_path / "other", "2026.10.09-0000000")
    with pytest.raises(ValueError, match="not the pinned"):
        m.pinned_cursor(other)


def test_cursor_digest_matches_v324(
    isolate_v3: None, m: ModuleType, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    v324 = importlib.import_module("harness.maintenance_review_v324")
    link = _cursor_tree(tmp_path, "2026.10.01-e373342")
    version_dir = link.resolve().parent
    assert m.cursor_version_digest(version_dir) == v324.cursor_version_digest(version_dir)


def test_every_stage_rechecks_cursor_and_both_codex_hashes(
    m: ModuleType, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    link = _cursor_tree(tmp_path, m.CURSOR_VERSION)
    version_dir = link.resolve().parent
    launcher, native = tmp_path / "codex.js", tmp_path / "codex"
    launcher.write_text("launcher")
    native.write_bytes(b"native")
    out = tmp_path / "out"
    out.mkdir()
    pins = {
        "cursor_version": {
            "version": m.CURSOR_VERSION,
            "directory": str(version_dir),
            "digest": m.cursor_version_digest(version_dir),
        },
        "codex": {
            "launcher": {"path": str(launcher), "sha256": v3.sha(launcher)},
            "native": {"path": str(native), "sha256": v3.sha(native)},
        },
    }
    (out / "protocol.json").write_text(json.dumps({"judge_pins": pins}))
    monkeypatch.setattr(m, "OUT", out)
    m._check_pins()
    native.write_bytes(b"swapped")
    with pytest.raises(RuntimeError, match="Codex native"):
        m._check_pins()
    native.write_bytes(b"native")
    launcher.write_text("swapped")
    with pytest.raises(RuntimeError, match="Codex launcher"):
        m._check_pins()
    launcher.write_text("launcher")
    (version_dir / "cursor-agent").write_text("#!/bin/sh\necho changed\n")
    with pytest.raises(RuntimeError, match="Cursor"):
        m._check_pins()


def test_pins_are_not_checked_before_prepare(
    m: ModuleType, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(m, "OUT", tmp_path / "not-prepared")
    m._check_pins()


def test_reviewers_must_be_neutral(m: ModuleType, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(m, "pinned_cursor", lambda: {"launcher": "/x/cursor-agent"})
    codex = {
        "launcher": {"path": "/c/codex.js", "sha256": "a"},
        "native": {"path": "/c/codex", "sha256": "b"},
    }
    monkeypatch.setattr(m, "pinned_codex", lambda: codex)
    monkeypatch.setattr(v3, "sha", lambda path: "launcher-sha")
    reviewers, _pins, binaries = m.pinned_reviewers()
    assert set(reviewers) == {"grok", "sol"}
    assert set(binaries) == {"/x/cursor-agent", "/c/codex.js", "/c/codex"}
    assert binaries["/c/codex"]["sha256"] == "b"
    monkeypatch.setattr(m, "SUBMISSION_LAB", "xAI")
    with pytest.raises(ValueError, match="neutral"):
        m.pinned_reviewers()


# --- transport ----------------------------------------------------------------


def _review_vote(scores: int = 3) -> dict:
    return {
        "score": 0,
        "rationale": "Clear.",
        "dimensions": {
            d: {"score": scores, "excerpt": "def ", "consequence": "A reader can follow it."}
            for d in v3.DIMENSIONS
        },
    }


def test_sol_panel_routes_through_codex(
    m: ModuleType, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(v3, "OUT", tmp_path)
    (tmp_path / "protocol.json").write_text("{}")
    protocol = {"reviewers": {"sol": {"model": "gpt-6.1-sol"}}}
    seen = {}

    def fake_codex(text: str, folder: Path, name: str, settings: dict, schema: dict) -> dict:
        seen.update(settings=settings, name=name)
        return _review_vote()

    monkeypatch.setattr(v3, "codex_vote", fake_codex)
    evidence = v3.controls()[0]
    vote = m.call("sol", "calibration", "control-0-r1", "review", evidence, protocol)
    assert seen == {"settings": {"model": "gpt-6.1-sol"}, "name": "attempt-1"}
    assert vote["panel"] == "sol" and vote["status"] == "complete" and vote["score"] == 75
    assert (tmp_path / "calls/sol/calibration/control-0-r1/selected.json").exists()
    monkeypatch.setattr(v3, "codex_vote", lambda *a: pytest.fail("cached vote must be reused"))
    assert m.call("sol", "calibration", "control-0-r1", "review", evidence, protocol) == vote


def test_grok_panel_uses_the_frozen_call(m: ModuleType, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(m, "_frozen_call", lambda *args: {"routed": args[0]})
    assert m.call("grok", "primary", "submission-001", "review", {}, {}) == {"routed": "grok"}


def test_sol_stream_parses_with_the_codex_parser(m: ModuleType) -> None:
    events = [
        {"type": "thread.started", "thread_id": "t1"},
        {"type": "item.completed", "item": {"type": "agent_message", "text": '{"answers": []}'}},
        {
            "type": "turn.completed",
            "usage": {"input_tokens": 10, "cached_input_tokens": 0, "output_tokens": 3},
        },
    ]
    vote = m.parse_stream_for("sol", "\n".join(json.dumps(e) for e in events))
    assert vote["session_id"] == "t1"


# --- population and diagnostics -----------------------------------------------


def test_population_check_accepts_21_medium_with_two_missing(m: ModuleType) -> None:
    _rows, excluded, missing, counts = m.check_population(_record())
    assert counts[("musewonderfull", "low")] == 23 and counts[("musewonderfull", "medium")] == 21
    assert len(missing) == 2 and not excluded


def test_population_check_refuses_gaps_without_reasons_or_wrong_cells(m: ModuleType) -> None:
    record = _record()
    record["missing"][0]["reason"] = ""
    with pytest.raises(ValueError, match="reason"):
        m.check_population(record)
    record = _record()
    record["missing"].pop()
    with pytest.raises(ValueError, match="not 23"):
        m.check_population(record)
    record = _record()
    record["rows"].append({"model": "musewonderfull", "effort": "high", "task": "legacy-t00"})
    with pytest.raises(ValueError, match="differ from expected"):
        m.check_population(record)


def test_diagnostic_pairs_skip_tasks_without_both_levels(m: ModuleType) -> None:
    rows = _record()["rows"]
    manifest = [{"id": f"submission-{i + 1:03d}", **r} for i, r in enumerate(rows)]
    tasks = {r["task"] for r in rows}
    selection = m.diagnostic_selection(manifest, tasks)
    by_id = {r["id"]: r for r in manifest}
    assert [by_id[i]["effort"] for i in selection["repeats"]] == ["low", "medium"]
    assert len(selection["pairs"]) == 2
    efforts = [[by_id[i]["effort"] for i in pair] for pair in selection["pairs"]]
    assert efforts == [["low", "medium"], ["medium", "low"]]
    for pair in selection["pairs"]:
        a, b = (by_id[i] for i in pair)
        assert a["task"] == b["task"] and a["task"] not in {"legacy-t03", "legacy-t07"}


@pytest.mark.skipif(
    not (ROOT / "docs/results/swe-v4-musewonder-full-2026-10/comparison.json").exists(),
    reason="the population record lands with the results PR (#197), not this engine PR",
)
def test_committed_population_record(m: ModuleType) -> None:
    record = json.loads(m.COMPARISON.read_text())
    rows, excluded, missing, counts = m.check_population(record)
    assert dict(counts) == {("musewonderfull", "low"): 23, ("musewonderfull", "medium"): 21}
    assert not excluded
    assert sorted((x["effort"], x["task"]) for x in missing) == [
        ("medium", "legacy-cellarcore-binary-parity"),
        ("medium", "legacy-paddockcore-binary-parity"),
    ]
    assert all("2026-10-19" in x["reason"] and "v3.25.1" in x["reason"] for x in missing)
    assert {r["solver_cli_version"] for r in rows} == {"Muse Code 1.4.6 (1.4.6-R6000.1)"}
    assert record["solver"]["model"] == "muse-code:muse-wonder-full-1.0-contributor"
    assert record["solver"]["muse_settings_plugins"] == {"skill-reminder": {"enabled": False}}
    assert all("error" not in r["solver_receipt"] for r in rows)


def test_protocol_document_carries_the_amendment(m: ModuleType) -> None:
    text = m.SOURCE_DOC.read_text()
    assert "## Amendment v3.25" in text and "code-quality-maintenance-v3.25" in text
    assert chr(0x2014) not in text and chr(0x2013) not in text


def test_cursor_pin_ignores_the_self_updating_shared_link(
    m: ModuleType, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Cursor repoints ~/.local/bin/cursor-agent when it updates; the pin uses the versioned launcher."""
    assert m.CURSOR_PINNED_LAUNCHER.parent.name == m.CURSOR_VERSION
    shared = tmp_path / "cursor-agent"
    newer = tmp_path / "versions/2099.01.01-newer/cursor-agent"
    newer.parent.mkdir(parents=True)
    newer.write_text("#!/bin/sh\n")
    shared.symlink_to(newer)
    monkeypatch.setattr(m.v3, "CURSOR", shared)
    pinned = tmp_path / "versions" / m.CURSOR_VERSION / "cursor-agent"
    pinned.parent.mkdir(parents=True)
    pinned.write_text("#!/bin/sh\n")
    monkeypatch.setattr(m, "CURSOR_PINNED_LAUNCHER", pinned)
    monkeypatch.setattr(m, "cursor_version_digest", lambda _d: m.CURSOR_DIGEST)
    record = m.pinned_cursor()
    assert record["version"] == m.CURSOR_VERSION
    assert record["shared_link_points_to"] == str(newer.resolve())
