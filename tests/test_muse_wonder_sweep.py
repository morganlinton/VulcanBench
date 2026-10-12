"""Regression coverage for the Muse Wonder Frontier v4 sweep launcher."""

import importlib.util
import json
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts/cii-v4-board"))
spec = importlib.util.spec_from_file_location(
    "muse_wonder_sweep", ROOT / "scripts/cii-v4-board/run_muse_wonder_sweep.py"
)
sweep = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sweep)


def test_contributor_approval_required_before_any_work(tmp_path, monkeypatch):
    called = []
    monkeypatch.setattr(sweep, "run", lambda *args: called.append(args))
    with pytest.raises(SystemExit):
        sweep.main(["--model", "wonder", "--out", str(tmp_path)])
    assert not called


def test_cli_routes_each_model_to_its_own_root(tmp_path, monkeypatch):
    called = []
    monkeypatch.setattr(sweep, "run", lambda *args: called.append(args))
    for name in ("wonder", "wonder-full"):
        out = tmp_path / name
        sweep.main(["--model", name, "--out", str(out), "--accept-contributor-training"])
    assert [(m, o.name) for m, o, _ in called] == [
        ("muse-code:muse-wonder-1.0-contributor", "wonder"),
        ("muse-code:muse-wonder-full-1.0-contributor", "wonder-full"),
    ]
    assert all(levels == ["low", "medium", "high", "extra-high"] for *_, levels in called)


def test_max_and_ultra_never_selectable(tmp_path):
    assert "max" not in sweep.LEVELS and "ultra" not in sweep.LEVELS
    with pytest.raises(SystemExit):
        sweep.main(
            [
                "--model",
                "wonder",
                "--out",
                str(tmp_path),
                "--efforts",
                "max",
                "--accept-contributor-training",
            ]
        )


@pytest.mark.parametrize(
    ("name", "cli_model"),
    [("wonder", "muse-wonder-1.0"), ("wonder-full", "muse-wonder-full-1.0")],
)
def test_protocol_records_contributor_mode_and_pins(name, cli_model):
    protocol = sweep.build_protocol(
        sweep.MODELS[name], sweep.BINARY, sweep.BINARY_SHA256, sweep.VERSION, {"t": "h"}, {"t": 1}
    )
    assert protocol["cli_model"] == cli_model and protocol["contributor_mode"] == "on"
    assert protocol["concurrency"] == 1 and protocol["binary_sha256"] == sweep.BINARY_SHA256
    assert set(protocol["source_hashes"]) == set(sweep.SOURCES)
    assert "—" not in json.dumps(protocol)


def test_changed_protocol_fails_closed(tmp_path):
    path = tmp_path / "protocol.json"
    sweep.save_protocol(path, {"model": "a"})
    sweep.save_protocol(path, {"model": "a"})
    with pytest.raises(AssertionError, match="Protocol changed"):
        sweep.save_protocol(path, {"model": "b"})


def test_refuses_to_start_beside_another_sweep(tmp_path, monkeypatch):
    monkeypatch.setattr(sweep, "other_sweep_active", lambda: ["4242"])
    monkeypatch.setattr(sweep, "ROOT", tmp_path)
    monkeypatch.chdir(tmp_path)  # run() chdirs to ROOT; restore cwd afterwards
    with pytest.raises(AssertionError, match="sweeps stay serial"):
        sweep.run(sweep.MODELS["wonder"], tmp_path / "out", sweep.LEVELS)


def test_record_result_is_idempotent(tmp_path):
    summary = {
        "run_id": "r1",
        "task_id": "t1",
        "scores": {"functional": 1.0},
        "total_tokens": 5,
        "duration_s": 2,
    }
    sweep.record_result(tmp_path, summary, "low")
    sweep.record_result(tmp_path, summary, "low")
    assert len((tmp_path / "results.jsonl").read_text().splitlines()) == 1
