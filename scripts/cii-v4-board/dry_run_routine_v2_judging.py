"""Dry run of the Routine v2 Code quality runner on reference fixes, with fake scores.

Builds a throwaway population from the Routine v2 gold patches (one model, two
levels; every other cell recorded as missing), runs the real ``prepare``
freeze, writes fake calibration verdicts and fake primary reviews instead of
judge calls, and runs the real ``summarize``. It checks the freeze (evidence in
each language, the size cap, controls for every language, planned calls) and
the publication arithmetic (per-language eligibility, single-judge cells, a
language with no Code quality). No judge is called. Everything is written to a
temporary directory that is deleted afterwards (set VB_DRY_RUN_KEEP=1 to keep
it), and only aggregates are printed, because the frozen evidence holds
private task content.

    VULCANROUTINE_ROOT=../VulcanRoutine-v2 python scripts/cii-v4-board/dry_run_routine_v2_judging.py
"""

from __future__ import annotations

import json
import os
import shutil
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MAIN_CHECKOUT = Path(os.environ.get("VB_JUDGE_PROTOCOL_ROOT", ROOT.parent / "VulcanBench"))
MODEL, LEVELS = "astra", ("low", "max")
# Fake exam outcome: Grok fails C, both judges fail Rust. Everything else passes.
FAILED = {("grok", "c"), ("muse", "rust"), ("grok", "rust")}
FAKE_SCORE = {"muse": 80.0, "grok": 70.0}


def main() -> None:  # noqa: PLR0912, one linear script
    temp = Path(tempfile.mkdtemp(prefix="vb-rv2-dryrun-"))
    os.environ["VB_ROUTINE_V2_JUDGING_OUT"] = str(temp / "out")
    os.environ["VB_ROUTINE_V2_COMPARISON"] = str(temp / "comparison.json")
    os.environ.setdefault("VB_JUDGE_PROTOCOL_ROOT", str(MAIN_CHECKOUT))
    sys.path.insert(0, str(ROOT))
    from harness import maintenance_review_routine_v2 as rv2  # noqa: PLC0415
    from harness import maintenance_review_v3 as v3  # noqa: PLC0415
    from harness import retrospective_judging as base  # noqa: PLC0415

    suite = json.loads((rv2.TASKS / "suite.json").read_text())["tasks"]
    chosen: list[str] = []
    for language in rv2.LANGUAGES:
        chosen += [t for t in suite if rv2.task_language(t) == language][: rv2.TASKS_PER_LANGUAGE]
    rows, missing = [], []
    for level in LEVELS:
        for task in chosen:
            run = temp / "runs" / level / task
            run.mkdir(parents=True)
            loaded = base.load_task(task, rv2.TASKS)
            summary = {"task_id": task, "task_hash": base.task_hash(loaded), "finished": True}
            (run / "summary.json").write_text(json.dumps(summary))
            (run / "final.patch").write_text((rv2.TASKS / task / "gold_patch.diff").read_text())
            data = base.inputs(run, rv2.TASKS)
            rows.append(
                {
                    "model": MODEL,
                    "effort": level,
                    "task": task,
                    "run_id": f"{level}-{task}",
                    "functional": 1.0,
                    "quality": 0.95,
                    "security": 1.0,
                    "source_directory": str(run),
                    "source_hashes": data["source_hashes"],
                }
            )
    for model, (levels, _lab) in rv2.MODELS.items():
        for level in levels:
            if model == MODEL and level in LEVELS:
                continue
            missing += [
                {"model": model, "effort": level, "task": t, "reason": "dry run"} for t in chosen
            ]
    (temp / "comparison.json").write_text(
        json.dumps({"suite": "routine-v2", "rows": rows, "excluded": [], "missing": missing})
    )

    rv2.prepare()
    protocol_sha = v3.sha(rv2.OUT / "protocol.json")
    for panel in rv2.PANELS:
        for language in rv2.LANGUAGES:
            passed = (panel, language) not in FAILED
            rv2.calibration_path(panel, language).write_text(
                json.dumps({"passed": passed, "protocol_sha256": protocol_sha})
            )
    manifest = json.loads((rv2.OUT / "private-manifest.json").read_text())
    for row in manifest:
        for panel in rv2.PANELS:
            if (panel, row["language"]) in FAILED:
                continue
            folder = rv2.OUT / "calls" / panel / "primary" / row["id"]
            folder.mkdir(parents=True)
            (folder / "selected.json").write_text(json.dumps({"score": FAKE_SCORE[panel]}))
    summary = rv2.summarize()

    preflight = json.loads((rv2.OUT / "preflight.json").read_text())
    by_language: dict[str, dict] = {}
    for r in summary["rows"]:
        pub = r["published"]
        entry = by_language.setdefault(
            r["language"], {"rows": 0, "code_quality": set(), "single": 0}
        )
        entry["rows"] += 1
        entry["code_quality"].add(pub["code_quality"])
        entry["single"] += bool(pub.get("single_judge"))
    checks = {
        "all rows published": summary["published_submissions"]
        == summary["expected_submissions"]
        == len(rows),
        "python two judges (75)": by_language["python"]["code_quality"] == {75.0},
        "javascript two judges (75)": by_language["javascript"]["code_quality"] == {75.0},
        "cpp two judges (75)": by_language["cpp"]["code_quality"] == {75.0},
        "c single judge, Muse (80)": by_language["c"]["code_quality"] == {80.0}
        and by_language["c"]["single"] == by_language["c"]["rows"],
        "rust has no Code quality": by_language["rust"]["code_quality"] == {None},
        "rust listed without Code quality": summary["languages_without_code_quality"] == ["rust"],
        "draft is not ready for publication": summary["ready_for_publication"] is False,
        "controls frozen for four new languages": all(
            len(list((rv2.OUT / "controls" / lang).glob("*.json"))) == 10
            for lang in rv2.NEW_LANGUAGES
        ),
        "calibration plan is 320 calls per judge": preflight["planned_calls"][
            "calibration_per_panel"
        ]
        == 320,
    }
    print(json.dumps({"evidence_bytes": preflight["evidence_bytes"], "submissions": len(rows)}))
    for name, ok in checks.items():
        print(f"{'ok  ' if ok else 'FAIL'} {name}")
    if os.environ.get("VB_DRY_RUN_KEEP"):
        print(f"(dry-run record kept in {temp}; it holds private task content)")
    else:
        shutil.rmtree(temp)
    sys.exit(0 if all(checks.values()) else 1)


if __name__ == "__main__":
    main()
