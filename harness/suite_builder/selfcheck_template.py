"""The stdlib-only self-check written into every scaffolded task as ``build/selfcheck.py``.

The authoring session cannot run VulcanBench (its environment has no API keys
and no path to this checkout), so it gets this script instead. It mirrors the
verifier: copy ``repo/`` to a scratch workspace, overlay ``tests/`` at its
root, run each ``fail_to_pass`` and ``pass_to_pass`` command with the shell
from the workspace root, once at the base commit and once with
``gold_patch.diff`` applied, three rounds. The task is ready when every
fail-to-pass test fails at base and passes with the gold patch, every
pass-to-pass test passes at both, and the three rounds agree.
"""

SELFCHECK = r'''#!/usr/bin/env python3
"""Self-check for this VulcanBench task. Run from anywhere: python build/selfcheck.py

Prints one JSON verdict and exits 0 when the task is ready: every
fail_to_pass test fails at the base commit and passes with gold_patch.diff,
every pass_to_pass test passes at both, three rounds in a row. Written by
`vulcanbench suite task scaffold`; it mirrors the VulcanBench verifier.
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

TASK = Path(__file__).resolve().parents[1]
ROUNDS = 3


def load():
    meta = json.loads((TASK / "metadata.json").read_text(encoding="utf-8"))
    spec = meta.get("tests") or {}
    return meta, list(spec.get("fail_to_pass") or []), list(spec.get("pass_to_pass") or [])


def workspace(meta, with_gold):
    root = Path(tempfile.mkdtemp(prefix="vb-selfcheck-"))
    ws = root / "workspace"
    shutil.copytree(TASK / "repo", ws, symlinks=True)
    if (TASK / "tests").is_dir():
        shutil.copytree(TASK / "tests", ws, dirs_exist_ok=True, symlinks=True)
    env = dict(os.environ, GIT_CEILING_DIRECTORIES=str(root))
    if with_gold:
        proc = subprocess.run(
            ["git", "apply", "--binary", str(TASK / "gold_patch.diff")],
            cwd=ws, capture_output=True, text=True, env=env, check=False,
        )
        if proc.returncode != 0:
            raise SystemExit(json.dumps({"ready": False, "error": "gold_patch.diff does not apply",
                                         "detail": proc.stderr.strip()[-400:]}))
    for step in meta.get("setup") or []:
        subprocess.run(step["cmd"], shell=True, cwd=ws, check=False,
                       timeout=int(meta.get("setup_timeout_s") or 600))
    return root, ws


def run_tests(ws, tests, timeout):
    out = {}
    for t in tests:
        try:
            proc = subprocess.run(t["cmd"], shell=True, cwd=ws, capture_output=True,
                                  text=True, timeout=timeout, check=False)
            out[t["name"]] = {"passed": proc.returncode == 0,
                              "tail": (proc.stdout + proc.stderr).strip()[-300:]}
        except subprocess.TimeoutExpired:
            out[t["name"]] = {"passed": False, "tail": f"timed out after {timeout} s"}
    return out


def main():
    meta, f2p, p2p = load()
    if not f2p:
        print(json.dumps({"ready": False, "error": "metadata.json tests.fail_to_pass is empty"}))
        return 1
    timeout = int(meta.get("test_timeout_s") or 300)
    rounds = []
    for _ in range(ROUNDS):
        result = {}
        for label, gold in (("base", False), ("gold", True)):
            root, ws = workspace(meta, gold)
            try:
                result[label] = run_tests(ws, f2p + p2p, timeout)
            finally:
                shutil.rmtree(root, ignore_errors=True)
        rounds.append(result)
    first = rounds[0]
    f2p_names = [t["name"] for t in f2p]
    p2p_names = [t["name"] for t in p2p]
    problems = []
    for n in f2p_names:
        if first["base"][n]["passed"]:
            problems.append(f"fail_to_pass {n} already passes at the base commit")
        if not first["gold"][n]["passed"]:
            problems.append(f"fail_to_pass {n} fails with the gold patch: {first['gold'][n]['tail']}")
    for n in p2p_names:
        for label in ("base", "gold"):
            if not first[label][n]["passed"]:
                problems.append(f"pass_to_pass {n} fails at {label}: {first[label][n]['tail']}")
    flat = [{k: {n: r["passed"] for n, r in v.items()} for k, v in rnd.items()} for rnd in rounds]
    deterministic = all(f == flat[0] for f in flat)
    if not deterministic:
        problems.append("results differ between rounds (flaky test or order dependence)")
    verdict = {
        "ready": not problems,
        "base_fails": all(not first["base"][n]["passed"] for n in f2p_names),
        "gold_passes": all(first["gold"][n]["passed"] for n in f2p_names + p2p_names),
        "deterministic": deterministic,
        "rounds": ROUNDS,
        "problems": problems,
        "base": flat[0]["base"],
        "gold": flat[0]["gold"],
    }
    print(json.dumps(verdict, indent=2))
    return 0 if not problems else 1


if __name__ == "__main__":
    sys.exit(main())
'''
