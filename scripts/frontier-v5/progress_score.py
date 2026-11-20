#!/usr/bin/env python3
"""Per-check results and the progress score for Frontier v5 release 1 trials.

usage: scripts/frontier-v5/progress_score.py [--repo DIR] [--json] <job-or-trial-dir> [...]

The Frontier v5 release 1 board (docs/frontier-v5/PLAN-v5.md, decision R3 in
docs/DECISIONS.md 2026-11-19) scores two numbers per attempt:

- **pass**: the verifier's own reward (1 or 0).
- **progress**: the fraction of held-out checks the attempt passes, counted
  only when the tamper scan is clean and every guard check holds; otherwise 0.

Which checks are held-out and which are guards is the rule in
``progress_rule.json`` next to this script. Its sha256 is pinned in
``tasks/frontier-v5/suite.json`` (``pins.progress_rule``), so the suite lock
freezes the rule with the tasks: changing it is a suite version bump.

Every release 1 verifier leaves a per-check record in the trial's
``verifier/`` directory, in its own runner's format; this module reads it the
way the task's own check script does, so the per-check statuses here are the
ones the reward was computed from:

  libtest   ``<binary>.log`` libtest logfiles (tree-sitter, pulldown-cmark)
  pytest    ``fail_to_pass.xml`` and ``pass_to_pass.xml`` JUnit reports (black)
  jest      ``jest.json`` (prettier)
  surefire  ``surefire/<module>/TEST-*.xml`` (javaparser 0.2.0); javaparser
            0.1.0 kept its reports in the verifier container, so its trials
            fall back to ``check.err``, where check.py names every listed id
            that did not pass (every other listed id passed)
  catch2    ``runs.tsv`` and ``junit/<program>.xml`` (CLI11)

Guard checks are: the tamper scan (``tamper_clean`` in reward.json), the
build or import step, the must-fail sentinels where the task has them, every
guard id passing, and, for tasks that grade the whole suite
(``min_total_tests`` in families.json), no failure outside the held-out ids
and at least ``min_total_tests`` tests seen. A held-out check that fails is
progress lost, never a broken guard.

Each scored trial is also checked against its reward: an attempt that passes
every held-out check with every guard holding must have reward 1, and any
other attempt reward 0. A disagreement is reported (``consistent: false``)
and is a bug in this module or a verifier, never a score to publish.
"""

from __future__ import annotations

import argparse
import glob
import hashlib
import json
import re
import sys
import xml.etree.ElementTree as ET
from collections import Counter
from collections.abc import Iterable
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
RULE_PATH = HERE / "progress_rule.json"
SUITE = "frontier-v5"

PASSED, FAILED, SKIPPED, MISSING = "passed", "failed", "skipped", "missing"
#: reward.json flags that must be 1 for any check to count, when present.
GATE_FLAGS = ("artifact_present", "tamper_clean", "build_ok", "compile_ok", "import_ok", "sentinel")


def rule_sha256(path: Path = RULE_PATH) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_rule(path: Path = RULE_PATH) -> dict[str, Any]:
    rule: dict[str, Any] = json.loads(path.read_text(encoding="utf-8"))
    return rule


# ---------------------------------------------------------------------------
# Per-check extraction, one reader per verifier format. Each returns
# (status by id, facts) where facts carries what a whole-suite guard needs:
# "total" (tests seen) and "errors" (unreadable reports, programs whose run
# did not count), plus "source" naming the record that was read.


@dataclass
class Checks:
    status: dict[str, str] = field(default_factory=dict)
    total: int = 0
    errors: list[str] = field(default_factory=list)
    source: str = ""


def _merge(status: dict[str, str], key: str, st: str) -> None:
    # A repeated or parameterised id counts as failed if any run of it failed.
    if status.get(key) != FAILED:
        status[key] = st


def read_libtest(vdir: Path, fam: dict[str, Any]) -> Checks:
    """libtest --logfile lines "ok <name>" / "failed <name>" / "ignored <name>"."""
    ids = list(fam["fail_to_pass"]) + list(fam["pass_to_pass"])
    binaries = {i.split("::", 1)[0] for i in ids}
    out = Checks(source="libtest")
    found = []
    for b in sorted(binaries):
        p = vdir / f"{b}.log"
        if not p.is_file():
            continue
        found.append(p.name)
        for line in p.read_text(errors="replace").splitlines():
            word, _, name = line.strip().partition(" ")
            if not name:
                continue
            st = {"ok": PASSED, "failed": FAILED, "ignored": SKIPPED}.get(word, word)
            _merge(out.status, f"{b}::{name}", st)
    out.total = len(out.status)
    out.source = "libtest:" + ",".join(found) if found else "libtest:none"
    return out


def _junit_key(node_id: str) -> str:
    # path/to/test_x.py::Class::name[param] -> path.to.test_x.Class::name[param]
    parts = node_id.split("::")
    module = parts[0][:-3].replace("/", ".") if parts[0].endswith(".py") else parts[0]
    return ".".join([module, *parts[1:-1]]) + "::" + parts[-1]


def _testcases(path: Path) -> dict[str, str] | None:
    try:
        root = ET.parse(path).getroot()
    except (OSError, ET.ParseError):
        return None
    got: dict[str, str] = {}
    for tc in root.iter("testcase"):
        k = f"{tc.get('classname')}::{tc.get('name')}"
        if tc.find("failure") is not None or tc.find("error") is not None:
            st = FAILED
        elif tc.find("skipped") is not None:
            st = SKIPPED
        else:
            st = PASSED
        _merge(got, k, st)
    return got


def read_pytest(vdir: Path, fam: dict[str, Any]) -> Checks:
    """pytest --junitxml reports, one per id set, as check_junit.py reads them."""
    out = Checks(source="pytest")
    by_junit: dict[str, str] = {}
    for label in ("fail_to_pass", "pass_to_pass"):
        got = _testcases(vdir / f"{label}.xml")
        if got is None:
            out.errors.append(f"{label}.xml unreadable")
            continue
        for k, st in got.items():
            _merge(by_junit, k, st)
    for tid in list(fam["fail_to_pass"]) + list(fam["pass_to_pass"]):
        got_st = by_junit.get(_junit_key(tid))
        if got_st is not None:
            out.status[tid] = got_st
    out.total = len(by_junit)
    return out


def read_jest(vdir: Path, fam: dict[str, Any]) -> Checks:
    """jest --json report, with check_jest.py's ids ("<file>::<titles>#<k>")."""
    out = Checks(source="jest")
    try:
        rep = json.loads((vdir / "jest.json").read_text(errors="replace"))
    except (OSError, json.JSONDecodeError):
        out.errors.append("jest.json unreadable")
        return out
    seen: Counter[str] = Counter()
    for res in rep.get("testResults", []):
        f = res.get("name", "").split("/tests/", 1)[-1]
        cases = res.get("assertionResults", [])
        for a in cases:
            base = f"{f}::{' > '.join([*a.get('ancestorTitles', []), a.get('title', '')])}"
            st = a.get("status")
            out.status[f"{base}#{seen[base]}"] = {"pending": SKIPPED, "skipped": SKIPPED}.get(
                st, st
            )
            seen[base] += 1
        if res.get("status") == "failed" and not any(a.get("status") == "failed" for a in cases):
            out.status[f"{f}::<file>#0"] = FAILED
    out.total = len(out.status)
    return out


def read_surefire(vdir: Path, fam: dict[str, Any]) -> Checks:
    """surefire TEST-*.xml copied by javaparser 0.2.0; check.err for 0.1.0."""
    paths = sorted(glob.glob(str(vdir / "surefire" / "*" / "TEST-*.xml")))
    if not paths:
        return read_check_err(vdir, fam)
    out = Checks(source="surefire")
    for p in paths:
        got = _testcases(Path(p))
        if got is None:
            out.errors.append(f"unreadable {Path(p).name}")
            continue
        for k, st in got.items():
            _merge(out.status, k, st)
    out.total = len(out.status)
    return out


CATCH2_SENTINEL = "vb sentinel 4f1c: must fail"


def read_catch2(vdir: Path, fam: dict[str, Any]) -> Checks:
    """Catch2 JUnit reports and runs.tsv, as CLI11's check.py reads them.

    A program counts only if its report parses, it reports the sentinel case
    as failed, and it exited non-zero without a timeout or a signal; otherwise
    none of its cases count and the program is a guard error.
    """
    out = Checks(source="catch2")
    runs: dict[str, str] = {}
    try:
        for line in (vdir / "runs.tsv").read_text().splitlines():
            if "\t" in line:
                prog, code = line.split("\t", 1)
                runs[prog] = code
    except OSError:
        out.errors.append("runs.tsv missing")
    for prog in fam["programs"]:
        code = runs.get(prog, "missing")
        try:
            raw = (vdir / "junit" / f"{prog}.xml").read_bytes().decode("utf-8", "replace")
        except OSError:
            out.errors.append(f"{prog}: no report")
            continue
        raw = re.sub(r"[\x00-\x08\x0b\x0c\x0e-\x1f]", "?", raw)
        raw = re.sub(
            r"&#(x0*[0-8bcef]|x0*1[0-9a-f]|0*[0-8]|0*1[1-9]|0*2[0-9]|0*3[01]);",
            "?",
            raw,
            flags=re.I,
        )
        try:
            root = ET.fromstring(raw)
        except ET.ParseError:
            out.errors.append(f"{prog}: unreadable report")
            continue
        got: dict[str, bool] = {}
        for tc in root.iter("testcase"):
            name = tc.get("name") or ""
            bad = tc.find("failure") is not None or tc.find("error") is not None
            got[name] = got.get(name, False) or bad
        if got.pop(CATCH2_SENTINEL, None) is not True:
            out.errors.append(f"{prog}: sentinel case not reported as failed")
            continue
        if not code.isdigit() or code == "0" or int(code) >= 124:
            out.errors.append(f"{prog}: exit status {code}")
            continue
        for name, failed in got.items():
            out.status[f"{prog}::{name}"] = FAILED if failed else PASSED
    out.total = len(out.status)
    return out


_ERR_LINE = re.compile(r"^(fail_to_pass|pass_to_pass): (.+): (\w+)(?: \(want \w+\))?$")
_UNEXPECTED = re.compile(r"^guard_suite: unexpected failure: (.+)$")
_SUITE_ERRORS = re.compile(r"^guard_suite: suiteErrors=(\d+) total=(\d+)")


def read_check_err(vdir: Path, fam: dict[str, Any]) -> Checks:
    """Fallback: check.py's stderr names every listed id that is not passed.

    Valid only when check.py ran to the end, which reward.json shows by a
    ``tests_seen`` count above zero; then every listed id it did not name
    passed. Used for javaparser 0.1.0 trials only.
    """
    out = Checks(source="check.err")
    reward = _reward(vdir)
    seen = int(reward.get("tests_seen", 0) or 0)
    try:
        text = (vdir / "check.err").read_text(errors="replace")
    except OSError:
        text = None
    if text is None or seen <= 0:
        out.errors.append("check.err missing or check.py did not finish")
        out.source = "check.err:none"
        return out
    named: dict[str, str] = {}
    for line in text.splitlines():
        m = _ERR_LINE.match(line)
        if m:
            named[m.group(2)] = FAILED if m.group(3) == "failed" else m.group(3)
            continue
        m = _UNEXPECTED.match(line)
        if m:
            named[m.group(1)] = FAILED
            continue
        m = _SUITE_ERRORS.match(line)
        if m and int(m.group(1)):
            out.errors.append(f"suiteErrors={m.group(1)}")
    for tid in list(fam["fail_to_pass"]) + list(fam["pass_to_pass"]):
        out.status[tid] = named.get(tid, PASSED)
    for tid, st in named.items():
        out.status.setdefault(tid, st)
    out.total = seen
    return out


READERS = {
    "libtest": read_libtest,
    "pytest": read_pytest,
    "jest": read_jest,
    "surefire": read_surefire,
    "catch2": read_catch2,
}


# ---------------------------------------------------------------------------
# Scoring


def _reward(vdir: Path) -> dict[str, Any]:
    try:
        d = json.loads((vdir / "reward.json").read_text())
    except (OSError, json.JSONDecodeError):
        return {}
    return d if isinstance(d, dict) else {}


def task_from_trial(trial: Path) -> str | None:
    """The release 1 task a Harbor trial ran, from its config.json task path."""
    for name in ("config.json", "result.json"):
        try:
            d = json.loads((trial / name).read_text())
        except (OSError, json.JSONDecodeError):
            continue
        path = (d.get("task") or {}).get("path") or ((d.get("config") or {}).get("task") or {}).get(
            "path"
        )
        if path:
            return Path(path).name
        tn = d.get("task_name")
        if tn:
            return str(tn).split("/")[-1]
    return None


@dataclass
class TrialScore:
    trial: str
    task: str
    reward: int | None
    passed: bool
    progress: float
    held_out_total: int
    held_out_passed: int
    guard_ok: bool
    guard_failures: list[str]
    consistent: bool
    source: str
    checks: dict[str, str] = field(repr=False, default_factory=dict)

    def summary(self) -> dict[str, Any]:
        d = asdict(self)
        d.pop("checks")
        d["guard_failures"] = self.guard_failures[:10]
        d["guard_failure_count"] = len(self.guard_failures)
        return d


def held_out_ids(task_rule: dict[str, Any], fam: dict[str, Any], mode: str) -> list[str]:
    ids = list(fam["fail_to_pass"])
    if mode == "fail_to_pass+held_out_guards":
        ids += [i for i in task_rule.get("held_out_guards", []) if i not in ids]
    return ids


def score_trial(
    trial: Path, repo: Path = REPO, rule: dict[str, Any] | None = None, task: str | None = None
) -> TrialScore:
    rule = rule if rule is not None else load_rule()
    task = task or task_from_trial(trial)
    if not task or task not in rule["tasks"]:
        raise ValueError(f"{trial}: not a release 1 board task ({task})")
    task_rule = rule["tasks"][task]
    fam = json.loads((repo / "tasks" / SUITE / task / "tests" / "families.json").read_text())
    vdir = trial / "verifier"
    reward = _reward(vdir)
    rv = reward.get("reward")
    rint = int(rv) if isinstance(rv, (int, float)) else None

    checks = READERS[task_rule["format"]](vdir, fam)
    held = held_out_ids(task_rule, fam, rule["held_out"])
    held_set = set(held)
    guard_ids = [i for i in fam["pass_to_pass"] if i not in held_set]
    guard_set = set(guard_ids)

    failures: list[str] = []
    for flag in GATE_FLAGS:
        if flag in reward and reward.get(flag) != 1:
            failures.append(f"{flag}=0")
    if "tamper_clean" not in reward:
        failures.append("tamper_clean missing")
    for gid in guard_ids:
        st = checks.status.get(gid, MISSING)
        if st != PASSED:
            failures.append(f"guard {gid}: {st}")
    failures += [f"record: {e}" for e in checks.errors]
    if "min_total_tests" in fam:
        allowed = held_set | set(fam.get("known_env_failures", [])) | set(fam.get("must_fail", []))
        for tid, st in sorted(checks.status.items()):
            if st == FAILED and tid not in allowed and tid not in guard_set:
                failures.append(f"unexpected failure {tid}")
        if checks.total < int(fam["min_total_tests"]):
            failures.append(f"tests seen {checks.total} < {fam['min_total_tests']}")

    n_held = sum(1 for h in held if checks.status.get(h) == PASSED)
    guard_ok = not failures
    progress = n_held / len(held) if guard_ok and held else 0.0
    # Every fail_to_pass and pass_to_pass id passing, guards holding, is what
    # reward 1 means for every release 1 verifier.
    full = guard_ok and all(
        checks.status.get(i) == PASSED
        for i in list(fam["fail_to_pass"]) + list(fam["pass_to_pass"])
    )
    consistent = rint is not None and full == (rint == 1)
    return TrialScore(
        trial=trial.name,
        task=task,
        reward=rint,
        passed=rint == 1,
        progress=round(progress, 6),
        held_out_total=len(held),
        held_out_passed=n_held,
        guard_ok=guard_ok,
        guard_failures=failures,
        consistent=consistent,
        source=checks.source,
        checks={i: checks.status.get(i, MISSING) for i in held + guard_ids},
    )


def trial_dirs(paths: Iterable[Path]) -> list[Path]:
    """Expand job directories into their trial directories (``*__*``)."""
    out: list[Path] = []
    for p in paths:
        if (p / "verifier").is_dir() or ((p / "config.json").is_file() and "__" in p.name):
            out.append(p)
        else:
            out += sorted(d for d in p.glob("*__*") if d.is_dir())
    return out


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("paths", nargs="+", type=Path, help="Harbor job or trial directories")
    ap.add_argument("--repo", type=Path, default=REPO, help="checkout whose tasks/ the trials ran")
    ap.add_argument("--json", action="store_true", help="one JSON object per trial")
    ap.add_argument("--checks", action="store_true", help="with --json, include per-check statuses")
    args = ap.parse_args(argv)
    rule = load_rule()
    bad = 0
    for tr in trial_dirs(args.paths):
        try:
            s = score_trial(tr, args.repo, rule)
        except ValueError as e:
            print(f"skip: {e}", file=sys.stderr)
            continue
        bad += not s.consistent
        if args.json:
            d = s.summary()
            if args.checks:
                d["checks"] = s.checks
            print(json.dumps(d, sort_keys=True))
        else:
            print(
                f"{s.trial}: reward={s.reward} progress={s.progress:.3f} "
                f"({s.held_out_passed}/{s.held_out_total}) guard={'ok' if s.guard_ok else 'broken'}"
                f"{'' if s.consistent else ' INCONSISTENT'} [{s.source}]"
            )
            for f in s.guard_failures[:5]:
                print(f"    {f}")
    if bad:
        print(f"progress_score: {bad} trial(s) disagree with their reward", file=sys.stderr)
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
