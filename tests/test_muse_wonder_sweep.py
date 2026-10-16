"""Regression coverage for the Muse Wonder Frontier v4 sweep launcher."""

import importlib.util
import json
import os
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
    assert [(m, o.name) for m, o, *_ in called] == [
        ("muse-code:muse-wonder-1.0-contributor", "wonder"),
        ("muse-code:muse-wonder-full-1.0-contributor", "wonder-full"),
    ]
    assert all(levels == ["low", "medium", "high", "extra-high"] for _, _, levels, *_ in called)
    # Defaults: the 1.4.6 pin and no continued root.
    assert all(args[3:] == ("1.4.6", None) for args in called)


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
    assert chr(0x2014) not in json.dumps(protocol)  # no em-dash in the protocol


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


def test_protocol_pins_muse_settings(tmp_path, monkeypatch):
    settings = tmp_path / "settings.json"
    settings.write_text('{"plugins": {"skill-reminder": {"enabled": false}}}')
    monkeypatch.setattr(sweep, "MUSE_SETTINGS", settings)
    protocol = sweep.build_protocol(
        sweep.MODELS["wonder-full"],
        sweep.BINARY,
        sweep.BINARY_SHA256,
        sweep.VERSION,
        {"t": "h"},
        {"t": 1},
    )
    assert protocol["muse_settings_plugins"] == {"skill-reminder": {"enabled": False}}
    before = protocol["muse_settings_sha256"]
    settings.write_text('{"plugins": {"skill-reminder": {"enabled": true}}}')
    assert sweep.muse_settings()[1] != before
    monkeypatch.setattr(sweep, "MUSE_SETTINGS", tmp_path / "missing.json")
    assert sweep.muse_settings()[0] == {}


def test_version_pins():
    assert set(sweep.PINS) == {"1.4.6", "1.4.9"}
    assert sweep.PINS["1.4.6"] == (sweep.BINARY, sweep.BINARY_SHA256, sweep.VERSION)
    binary, digest, version = sweep.PINS["1.4.9"]
    assert binary.name == "muse-bin-1.4.9-R6297.1"
    assert digest == "2b856618ae3de3a63956e6d510b849ad707b16e5d2ed196299f3fe5ef2e3ff5e"
    assert version == "Muse Code 1.4.9 (1.4.9-R6297.1)"


def test_cli_passes_version_and_continued_root(tmp_path, monkeypatch):
    called = []
    monkeypatch.setattr(sweep, "run", lambda *args: called.append(args))
    earlier = tmp_path / "v2"
    sweep.main(
        [
            "--model",
            "wonder-full",
            "--out",
            str(tmp_path / "v2-149"),
            "--efforts",
            "medium",
            "high",
            "--muse-version",
            "1.4.9",
            "--continue-from",
            str(earlier),
            "--accept-contributor-training",
        ]
    )
    ((_model, out, levels, release, cont),) = called
    assert (out.name, levels, release, cont) == (
        "v2-149",
        ["medium", "high"],
        "1.4.9",
        earlier.resolve(),
    )
    with pytest.raises(SystemExit):
        sweep.main(
            [
                "--model",
                "wonder",
                "--out",
                str(tmp_path),
                "--muse-version",
                "1.4.7",
                "--accept-contributor-training",
            ]
        )


def _summary(root, level, task, model, task_hash="h"):
    run = root / level / f"{task}-abc"
    run.mkdir(parents=True)
    (run / "summary.json").write_text(
        json.dumps(
            {
                "task_id": task,
                "model": model,
                "task_hash": task_hash,
                "effort": {"requested": level},
            }
        )
    )


def test_completed_in_reads_the_earlier_root(tmp_path):
    model = sweep.MODELS["wonder-full"]
    hashes = {"a": "h", "b": "h", "c": "h"}
    _summary(tmp_path, "medium", "a", model)
    _summary(tmp_path, "medium", "b", model)
    assert sweep.completed_in(tmp_path, "medium", model, hashes) == {"a", "b"}
    assert sweep.completed_in(tmp_path, "high", model, hashes) == set()
    with pytest.raises(AssertionError, match="task changed"):
        sweep.completed_in(tmp_path, "medium", model, {"a": "h", "b": "other"})
    with pytest.raises(AssertionError):
        sweep.completed_in(tmp_path, "medium", sweep.MODELS["wonder"], hashes)


def test_continuation_records_the_earlier_root(tmp_path):
    earlier = tmp_path / "v2"
    earlier.mkdir()
    model = sweep.MODELS["wonder-full"]
    (earlier / "protocol.json").write_text(
        json.dumps({"model": model, "harness_version": "Muse Code 1.4.6 (1.4.6-R6000.1)"})
    )
    assert sweep.continuation(None, model, tmp_path / "new") is None
    record = sweep.continuation(earlier, model, tmp_path / "new")
    assert record["harness_version"] == "Muse Code 1.4.6 (1.4.6-R6000.1)"
    assert record["root"] == str(earlier) and len(record["protocol_sha256"]) == 64
    with pytest.raises(AssertionError, match="same model"):
        sweep.continuation(earlier, sweep.MODELS["wonder"], tmp_path / "new")
    with pytest.raises(AssertionError, match="same model"):
        sweep.continuation(earlier, model, earlier)
    protocol = sweep.build_protocol(
        model, *sweep.PINS["1.4.9"], {"t": "h"}, {"t": 1}, continues=record
    )
    assert protocol["continues"] == record and protocol["harness_version"].startswith(
        "Muse Code 1.4.9"
    )


def test_reminder_allowance_follows_the_release(monkeypatch):
    monkeypatch.setattr(sweep, "pinned_executable", lambda: ("bin", "sha"))
    for name in ("BINARY", "SHA256", "REMINDER_MODELS"):  # restored after the test
        monkeypatch.setenv(f"VULCANBENCH_MUSE_{name}", "")
    sweep.configure_binary("1.4.9")
    assert os.environ["VULCANBENCH_MUSE_REMINDER_MODELS"] == "muse-spark-1.3"
    sweep.configure_binary("1.4.6")
    assert os.environ["VULCANBENCH_MUSE_REMINDER_MODELS"] == ""
    model = sweep.MODELS["wonder-full"]
    strict = sweep.build_protocol(model, *sweep.PINS["1.4.6"], {"t": "h"}, {"t": 1})
    assert "reminder_observer_models" not in strict
    allowed = sweep.build_protocol(
        model,
        *sweep.PINS["1.4.9"],
        {"t": "h"},
        {"t": 1},
        reminder_models=sweep.REMINDER_MODELS["1.4.9"],
    )
    assert allowed["reminder_observer_models"] == ["muse-spark-1.3"]
