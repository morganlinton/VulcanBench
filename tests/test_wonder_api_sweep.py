"""Muse Wonder 1.0 API sweep: one level per launch, approval gate, pairing and budget stop."""

import importlib.util
import json
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts/cii-v4-board"))
spec = importlib.util.spec_from_file_location(
    "wonder_api_sweep", ROOT / "scripts/cii-v4-board/run_wonder_api_sweep.py"
)
sweep = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sweep)


def test_each_level_needs_explicit_approval(tmp_path, monkeypatch):
    called = []
    monkeypatch.setattr(sweep, "run", lambda *a: called.append(a))
    with pytest.raises(SystemExit):
        sweep.main(["--effort", "low", "--budget", "10", "--out", str(tmp_path)])
    assert not called
    sweep.main(["--effort", "low", "--budget", "10", "--out", str(tmp_path), "--accept-api-spend"])
    assert called[0][1:3] == ("low", 10.0)


def test_levels_are_low_to_max_and_ultra_is_refused(tmp_path):
    assert sweep.LEVELS == ["low", "medium", "high", "extra-high", "max"]
    for bad in ("ultra", "minimal"):
        with pytest.raises(SystemExit):
            sweep.main(
                ["--effort", bad, "--budget", "1", "--out", str(tmp_path), "--accept-api-spend"]
            )


def test_both_arms_bill_the_api_on_wonder_10():
    assert sweep.ARMS == {
        "pi": "pi:meta/muse-wonder-1.0",
        "muse-code": "muse-code:muse-wonder-1.0-api",
    }
    for arm in sweep.ARMS:
        protocol = sweep.build_protocol(arm, sweep.VERSIONS[arm], {"t": "h"}, {"t": 1})
        assert protocol["concurrency"] == 1 and protocol["served_model"] == "muse-wonder-1.0"
        assert set(protocol["source_hashes"]) == set(sweep.SOURCES)
        pins = protocol["pins"]
        assert all(k.startswith("VULCANBENCH_PI") == (arm == "pi") for k in pins)


def test_spend_reads_metered_receipts(tmp_path):
    for arm, usd in (("pi", 0.5), ("muse-code", 1.5)):
        run = tmp_path / arm / "low" / "t-1"
        run.mkdir(parents=True)
        (run / "summary.json").write_text(
            json.dumps({"economics": {"marginal_cash_usd": usd}, "scores": {"functional": 1.0}})
        )
    spend = sweep.level_spend(tmp_path, "low")
    assert spend["pi"]["usd"] == 0.5 and spend["muse-code"]["usd_per_task"] == 1.5
    assert sweep.total_spend(tmp_path) == 2.0


def test_refuses_beside_another_sweep(tmp_path, monkeypatch):
    monkeypatch.setattr(sweep, "other_sweep_active", lambda: ["4242"])
    monkeypatch.setattr(sweep, "ROOT", tmp_path)
    monkeypatch.chdir(tmp_path)
    with pytest.raises(AssertionError, match="sweeps stay serial"):
        sweep.run(tmp_path / "out", "low", 10.0, 5.0)
