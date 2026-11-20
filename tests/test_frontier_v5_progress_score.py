"""scripts/frontier-v5/progress_score.py on synthetic Harbor trials.

Each release 1 verifier format is written here from the task's real
families.json, the way the verifier writes it (libtest logfiles, pytest JUnit
XML, jest JSON, surefire XML, Catch2 JUnit plus runs.tsv), so the reader is
checked against the ids the reward is graded on. No Docker, no model calls.
"""

from __future__ import annotations

import hashlib
import importlib.util
import json
import sys
from pathlib import Path
from xml.sax.saxutils import quoteattr

import pytest

REPO = Path(__file__).resolve().parents[1]
SCRIPT = REPO / "scripts" / "frontier-v5" / "progress_score.py"
SUITE = REPO / "tasks" / "frontier-v5"

_spec = importlib.util.spec_from_file_location("progress_score", SCRIPT)
assert _spec and _spec.loader
ps = importlib.util.module_from_spec(_spec)
sys.modules["progress_score"] = ps
_spec.loader.exec_module(ps)

TASKS = json.loads((SUITE / "suite.json").read_text())["tasks"]


def families(task: str) -> dict:
    return json.loads((SUITE / task / "tests" / "families.json").read_text())


# --- writers, one per verifier format ------------------------------------


def _xml_cases(cases: list[tuple[str, str, str]]) -> str:
    body = []
    for cls, name, st in cases:
        inner = {"failed": "<failure message='x'/>", "skipped": "<skipped/>"}.get(st, "")
        body.append(
            f"<testcase classname={quoteattr(cls)} name={quoteattr(name)}>{inner}</testcase>"
        )
    return "<testsuite>" + "".join(body) + "</testsuite>"


def write_libtest(vdir: Path, status: dict[str, str], fam: dict) -> None:
    by_bin: dict[str, list[str]] = {}
    word = {"passed": "ok", "failed": "failed", "skipped": "ignored"}
    for tid, st in status.items():
        b, name = tid.split("::", 1)
        by_bin.setdefault(b, []).append(f"{word[st]} {name}")
    for b, lines in by_bin.items():
        (vdir / f"{b}.log").write_text("\n".join(lines) + "\n")
    # The verifier also leaves these next to the logfiles; they must be ignored.
    (vdir / "build.log").write_text("ok compiling\nfailed nothing\n")
    (vdir / "tamper.log").write_text("tamper: clean\n")


def write_pytest(vdir: Path, status: dict[str, str], fam: dict) -> None:
    for label in ("fail_to_pass", "pass_to_pass"):
        cases = []
        for tid in fam[label]:
            if tid in status:
                cls, name = ps._junit_key(tid).split("::", 1)
                cases.append((cls, name, status[tid]))
        (vdir / f"{label}.xml").write_text(_xml_cases(cases))


def write_jest(vdir: Path, status: dict[str, str], fam: dict) -> None:
    files: dict[str, list[dict]] = {}
    for tid, st in sorted(
        status.items(), key=lambda kv: (kv[0].rsplit("#", 1)[0], int(kv[0].rsplit("#", 1)[1]))
    ):
        f, rest = tid.split("::", 1)
        title = rest.rsplit("#", 1)[0]
        files.setdefault(f, []).append({"ancestorTitles": [], "title": title, "status": st})
    res = [
        {
            "name": f"/repo/tests/{f}",
            "status": "failed" if any(a["status"] == "failed" for a in cases) else "passed",
            "assertionResults": cases,
        }
        for f, cases in files.items()
    ]
    (vdir / "jest.json").write_text(json.dumps({"testResults": res}))


def write_surefire(vdir: Path, status: dict[str, str], fam: dict) -> None:
    d = vdir / "surefire" / "javaparser-symbol-solver-testing"
    d.mkdir(parents=True)
    cases = [(*tid.split("::", 1), st) for tid, st in status.items()]
    (d / "TEST-all.xml").write_text(_xml_cases(cases))


def write_catch2(vdir: Path, status: dict[str, str], fam: dict) -> None:
    (vdir / "junit").mkdir()
    by_prog: dict[str, list[tuple[str, str, str]]] = {p: [] for p in fam["programs"]}
    for tid, st in status.items():
        prog, name = tid.split("::", 1)
        by_prog[prog].append((prog, name, st))
    runs = []
    for prog, cases in by_prog.items():
        cases.append((prog, ps.CATCH2_SENTINEL, "failed"))
        (vdir / "junit" / f"{prog}.xml").write_text(_xml_cases(cases))
        runs.append(f"{prog}\t1")
    (vdir / "runs.tsv").write_text("\n".join(runs) + "\n")


WRITERS = {
    "libtest": write_libtest,
    "pytest": write_pytest,
    "jest": write_jest,
    "surefire": write_surefire,
    "catch2": write_catch2,
}

#: reward.json flags each verifier writes besides reward (all 1 on a clean run).
FLAGS = {
    "treesitter-query-anchor-matching": [
        "artifact_present",
        "tamper_clean",
        "build_ok",
        "fail_to_pass",
        "pass_to_pass",
    ],
    "black-fmt-skip-region-handling": [
        "artifact_present",
        "tamper_clean",
        "sentinel",
        "import_ok",
        "fail_to_pass",
        "pass_to_pass",
    ],
    "prettier-comment-attachment-stability": [
        "artifact_present",
        "tamper_clean",
        "sentinel",
        "fail_to_pass",
        "pass_to_pass",
        "guard_suite",
    ],
    "javaparser-member-resolution-scope": [
        "artifact_present",
        "tamper_clean",
        "compile_ok",
        "fail_to_pass",
        "pass_to_pass",
        "guard_suite",
    ],
    "cli11-toml-config-round-trip": [
        "artifact_present",
        "tamper_clean",
        "compile_ok",
        "sentinel",
        "fail_to_pass",
        "pass_to_pass",
        "guard_suite",
    ],
    "pulldown-cmark-html-block-scanning": [
        "artifact_present",
        "tamper_clean",
        "build_ok",
        "fail_to_pass",
        "pass_to_pass",
    ],
}


def make_trial(
    tmp: Path,
    task: str,
    failing: set[str] = frozenset(),  # type: ignore[assignment]
    reward: int | None = None,
    **flag_overrides: int,
) -> Path:
    fam = families(task)
    rule = ps.load_rule()["tasks"][task]
    trial = tmp / f"{task[:32]}__T{len(list(tmp.iterdir()))}"
    vdir = trial / "verifier"
    vdir.mkdir(parents=True)
    (trial / "config.json").write_text(
        json.dumps({"task": {"path": f"/x/tasks/frontier-v5/{task}"}})
    )
    status = {
        tid: ("failed" if tid in failing else "passed")
        for tid in fam["fail_to_pass"] + fam["pass_to_pass"]
    }
    for tid in fam.get("must_fail", []):
        if rule["format"] == "jest":
            status[tid] = "failed"
    # Whole-suite tasks run tests outside the listed ids (min_total_tests
    # counts them); pad with passing unlisted cases the way a real run has them.
    filler = {
        "surefire": "vb.Filler::t{}",
        "jest": "unit/vb-filler.test.js::t{}#0",
        "catch2": fam.get("programs", ["x"])[0] + "::filler {}",
    }
    for i in range(max(0, int(fam.get("min_total_tests", 0)) - len(status))):
        status[filler[rule["format"]].format(i)] = "passed"
    WRITERS[rule["format"]](vdir, status, fam)
    f2p_ok = all(t not in failing for t in fam["fail_to_pass"])
    p2p_ok = all(t not in failing for t in fam["pass_to_pass"])
    flags = {k: 1 for k in FLAGS[task]}
    flags.update(fail_to_pass=int(f2p_ok), pass_to_pass=int(p2p_ok))
    if "guard_suite" in flags:
        flags["guard_suite"] = int(not failing)
    if "tests_seen" in fam or "min_total_tests" in fam:
        flags["tests_seen"] = len(status)
    flags.update(flag_overrides)
    if reward is None:
        gate = all(
            flags.get(k, 1) == 1
            for k in ("tamper_clean", "sentinel", "build_ok", "compile_ok", "import_ok")
        )
        reward = int(gate and not failing)
    (vdir / "reward.json").write_text(json.dumps({"reward": reward, **flags}))
    return trial


# --- the rule is frozen with the suite ------------------------------------


def test_rule_sha_is_pinned_in_the_suite_manifest_and_lock() -> None:
    pin = json.loads((SUITE / "suite.json").read_text())["pins"]["progress_rule"]
    assert pin["path"] == "scripts/frontier-v5/progress_rule.json"
    assert pin["sha256"] == hashlib.sha256((REPO / pin["path"]).read_bytes()).hexdigest()
    lock = json.loads((SUITE / "suite.lock.json").read_text())
    assert lock["pins"]["progress_rule"] == pin


def test_rule_covers_exactly_the_board_tasks() -> None:
    rule = ps.load_rule()
    assert sorted(rule["tasks"]) == sorted(TASKS)
    assert rule["held_out"] == "fail_to_pass"
    assert all(t["format"] in ps.READERS for t in rule["tasks"].values())


def test_suite_1_1_0_has_no_private_task() -> None:
    manifest = json.loads((SUITE / "suite.json").read_text())
    assert manifest["version"] == "1.1.0" and "private_tasks" not in manifest
    assert "ferrycore" not in json.dumps([manifest["tasks"], manifest["pins"]])
    assert len(TASKS) == 6


# --- every task, every format ---------------------------------------------


@pytest.mark.parametrize("task", TASKS)
def test_clean_pass_scores_one_and_agrees_with_reward(tmp_path: Path, task: str) -> None:
    s = ps.score_trial(make_trial(tmp_path, task), REPO)
    assert s.reward == 1 and s.passed and s.guard_ok, s.guard_failures[:5]
    assert s.progress == 1.0 and s.held_out_passed == s.held_out_total == len(
        families(task)["fail_to_pass"]
    )
    assert s.consistent


@pytest.mark.parametrize("task", TASKS)
def test_partial_progress_counts_held_out_checks(tmp_path: Path, task: str) -> None:
    fam = families(task)
    miss = set(fam["fail_to_pass"][:2])
    s = ps.score_trial(make_trial(tmp_path, task, failing=miss), REPO)
    n = len(fam["fail_to_pass"])
    assert s.reward == 0 and s.guard_ok, s.guard_failures[:5]
    assert s.held_out_passed == n - 2
    assert s.progress == pytest.approx((n - 2) / n, abs=1e-6)
    assert s.consistent
    assert sum(1 for t in miss if s.checks[t] == "failed") == 2


@pytest.mark.parametrize("task", TASKS)
def test_a_broken_guard_scores_zero(tmp_path: Path, task: str) -> None:
    fam = families(task)
    s = ps.score_trial(make_trial(tmp_path, task, failing={fam["pass_to_pass"][0]}), REPO)
    assert s.reward == 0 and not s.guard_ok and s.progress == 0.0
    assert s.held_out_passed == len(fam["fail_to_pass"])  # still reported
    assert any(fam["pass_to_pass"][0] in f for f in s.guard_failures)
    assert s.consistent


@pytest.mark.parametrize("task", TASKS)
def test_tamper_scan_hit_scores_zero(tmp_path: Path, task: str) -> None:
    s = ps.score_trial(make_trial(tmp_path, task, tamper_clean=0), REPO)
    assert s.reward == 0 and s.progress == 0.0 and "tamper_clean=0" in s.guard_failures
    assert s.consistent


@pytest.mark.parametrize("task", TASKS)
def test_no_record_scores_zero(tmp_path: Path, task: str) -> None:
    # A build failure: the verifier emits reward 0 before any test record.
    fam = families(task)
    trial = tmp_path / f"{task[:32]}__Tbuild"
    (trial / "verifier").mkdir(parents=True)
    (trial / "config.json").write_text(json.dumps({"task": {"path": f"/x/{task}"}}))
    flags = {k: 0 for k in FLAGS[task]}
    flags.update(artifact_present=1, tamper_clean=1)
    (trial / "verifier" / "reward.json").write_text(json.dumps({"reward": 0, **flags}))
    s = ps.score_trial(trial, REPO)
    assert s.progress == 0.0 and not s.guard_ok and s.held_out_passed == 0
    assert s.held_out_total == len(fam["fail_to_pass"]) and s.consistent


def test_disagreement_with_the_reward_is_flagged(tmp_path: Path) -> None:
    task = "treesitter-query-anchor-matching"
    s = ps.score_trial(make_trial(tmp_path, task, reward=0), REPO)
    assert not s.consistent and s.progress == 1.0


# --- format details -----------------------------------------------------------


def test_whole_suite_tasks_count_unlisted_failures_as_broken_guards(tmp_path: Path) -> None:
    task = "javaparser-member-resolution-scope"
    trial = make_trial(tmp_path, task)
    p = trial / "verifier" / "surefire" / "javaparser-core-testing"
    p.mkdir(parents=True)
    (p / "TEST-extra.xml").write_text(_xml_cases([("com.example.NotListed", "boom", "failed")]))
    s = ps.score_trial(trial, REPO, rule=ps.load_rule())
    assert not s.guard_ok and s.progress == 0.0
    assert "unexpected failure com.example.NotListed::boom" in s.guard_failures


def test_too_few_tests_seen_is_a_broken_guard(tmp_path: Path) -> None:
    task = "cli11-toml-config-round-trip"
    fam = families(task)
    trial = make_trial(tmp_path, task)
    # Drop one guard program's report: its cases do not count.
    victim = next(p for p in fam["programs"] if p != "ConfigFileTest")
    (trial / "verifier" / "junit" / f"{victim}.xml").unlink()
    s = ps.score_trial(trial, REPO)
    assert not s.guard_ok
    assert any(f.startswith(f"record: {victim}") for f in s.guard_failures)
    assert any(f.startswith("tests seen") for f in s.guard_failures)


def test_catch2_program_without_its_sentinel_does_not_count(tmp_path: Path) -> None:
    task = "cli11-toml-config-round-trip"
    trial = make_trial(tmp_path, task)
    runs = (
        (trial / "verifier" / "runs.tsv")
        .read_text()
        .replace("ConfigFileTest\t1", "ConfigFileTest\t0")
    )
    (trial / "verifier" / "runs.tsv").write_text(runs)
    s = ps.score_trial(trial, REPO)
    assert not s.guard_ok and s.held_out_passed == 0
    assert "record: ConfigFileTest: exit status 0" in s.guard_failures


def test_jest_repeated_names_get_counted_ids(tmp_path: Path) -> None:
    vdir = tmp_path / "verifier"
    vdir.mkdir()
    (vdir / "jest.json").write_text(
        json.dumps(
            {
                "testResults": [
                    {
                        "name": "/repo/tests/format/a/format.test.js",
                        "status": "failed",
                        "assertionResults": [
                            {"ancestorTitles": ["x.js"], "title": "format", "status": "passed"},
                            {"ancestorTitles": ["x.js"], "title": "format", "status": "failed"},
                            {"ancestorTitles": ["y.js"], "title": "format", "status": "pending"},
                        ],
                    },
                    {
                        "name": "/repo/tests/unit/broken.test.js",
                        "status": "failed",
                        "assertionResults": [],
                    },
                ]
            }
        )
    )
    c = ps.read_jest(vdir, {"fail_to_pass": [], "pass_to_pass": []})
    assert c.status == {
        "format/a/format.test.js::x.js > format#0": "passed",
        "format/a/format.test.js::x.js > format#1": "failed",
        "format/a/format.test.js::y.js > format#0": "skipped",
        "unit/broken.test.js::<file>#0": "failed",
    }


def test_javaparser_0_1_0_falls_back_to_check_err(tmp_path: Path) -> None:
    task = "javaparser-member-resolution-scope"
    fam = families(task)
    miss = fam["fail_to_pass"][0]
    trial = tmp_path / "javaparser-member-resolution-sco__old"
    vdir = trial / "verifier"
    vdir.mkdir(parents=True)
    (trial / "config.json").write_text(json.dumps({"task": {"path": f"/x/{task}"}}))
    total = fam["min_total_tests"]
    (vdir / "reward.json").write_text(
        json.dumps(
            {
                "reward": 0,
                "artifact_present": 1,
                "tamper_clean": 1,
                "compile_ok": 1,
                "fail_to_pass": 0,
                "pass_to_pass": 1,
                "guard_suite": 0,
                "tests_seen": total,
            }
        )
    )
    (vdir / "check.err").write_text(
        f"fail_to_pass: {miss}: failed\n"
        f"guard_suite: unexpected failure: {miss}\n"
        f"guard_suite: suiteErrors=0 total={total} (min {fam['min_total_tests']})\n"
    )
    s = ps.score_trial(trial, REPO)
    assert s.source == "check.err" and s.guard_ok and s.consistent
    assert s.held_out_passed == len(fam["fail_to_pass"]) - 1

    # A check.py that never finished (tests_seen 0) proves nothing.
    (vdir / "reward.json").write_text(json.dumps({"reward": 0, "tamper_clean": 1, "tests_seen": 0}))
    s = ps.score_trial(trial, REPO)
    assert not s.guard_ok and s.progress == 0.0


def test_libtest_ignores_non_logfile_logs(tmp_path: Path) -> None:
    vdir = tmp_path
    (vdir / "cli.log").write_text("ok a::b\nfailed a::c\nok a::c\nignored a::d\n")
    (vdir / "test.log").write_text("ok cli::a::b\n")
    c = ps.read_libtest(vdir, {"fail_to_pass": ["cli::a::b"], "pass_to_pass": ["cli::a::c"]})
    assert c.status == {"cli::a::b": "passed", "cli::a::c": "failed", "cli::a::d": "skipped"}
    assert c.source == "libtest:cli.log"


def test_cli_prints_one_line_per_trial_and_fails_on_disagreement(tmp_path: Path, capsys) -> None:
    job = tmp_path / "job"
    job.mkdir()
    make_trial(job, "black-fmt-skip-region-handling")
    make_trial(
        job,
        "black-fmt-skip-region-handling",
        failing={families("black-fmt-skip-region-handling")["fail_to_pass"][0]},
    )
    assert ps.main(["--json", str(job)]) == 0
    rows = [json.loads(x) for x in capsys.readouterr().out.splitlines()]
    assert sorted(r["progress"] for r in rows) == [pytest.approx(6 / 7, abs=1e-6), 1.0]
    make_trial(job, "black-fmt-skip-region-handling", reward=0)
    assert ps.main([str(job)]) == 1
    assert "INCONSISTENT" in capsys.readouterr().out
