#!/usr/bin/env python3
"""Run single-fix and leave-one-out controls for a Frontier v5 task.

usage: scripts/frontier-v5/run_controls.py <task-dir> [--work DIR] [--keep-images]
                                           [--skip-build] [--timeout SEC] [--only NAME,...]

F5 (concentrated multi-bug correctness, docs/frontier-v5/PLAN-v2.md) admits a
task only if controls show its causes interact, every cause is necessary and
no single fix dominates. This script measures that with the task's real
verifier. The task declares its gold split into per-cause parts in
tests/controls.json:

  {"artifact_root": "app",            # patch paths are relative to this
                                       # directory inside the artifact tree
   "parts": [                          # in upstream order; base + all = gold
     {"name": "#1769", "patch": "tests/controls/1769.diff"},
     {"name": "docs", "patch": "tests/controls/docs.diff", "glue": true},
     ...],
   "coupled": [["#1784", "#1787"]]}    # optional: a fallback group, see loo

A "glue" part carries no cause (for example an upstream Javadoc commit inside
the range): it is applied wherever it applies and never controlled itself.
Variants, each graded by the real verifier on a fresh artifact:

  single:<name>   base + that part alone (plus glue that applies)
  loo:<name>      every part in order except that one; if a later part does
                  not apply without it and it is in a "coupled" group, the
                  whole group is left out instead (recorded as a fallback)
  gold-from-parts base + every part: must score reward 1, which proves the
                  parts compose to the gold
  causes-only     base + every cause, no glue (only when glue exists): reward
                  1 shows no glue part is needed to pass

A variant whose patches do not apply is recorded "n/a" with the reason; that
is itself evidence of source-level coupling when the parts are upstream
commits.

When the gold is one upstream change split by hand into per-cause parts, the
parts' contexts chain and most single and leave-one-out variants would not
apply, which says nothing about the code. Such a task lists its variants
explicitly instead, each a patch from base written for that one variant:

  "variants": [{"name": "single:bug1", "patch": "tests/controls/single-bug1.diff"},
               {"name": "loo:bug1", "patch": "tests/controls/loo-bug1.diff"}, ...]

Explicit variants replace the generated single and leave-one-out variants;
gold-from-parts is still run from "parts". Prints a table of held-out
(fail_to_pass) checks passing per variant and writes <work>/controls-report.json.
Reuses scripts/frontier-v5/probe_verifier.py for images, artifacts and runs.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from probe_verifier import extract, proxy_args, run_verifier, sh  # noqa: E402

F2P_LINE = re.compile(r"^fail_to_pass: (\S+?): (\S+)", re.M)


def apply(root: Path, patch: Path, reverse: bool = False) -> str | None:
    """Apply patch inside root; return None on success, else the error text."""
    cmd = ["git", "apply", "--whitespace=nowarn"] + (["-R"] if reverse else []) + [str(patch)]
    # Inside a git work tree, git apply resolves paths against that tree's
    # top level; the ceiling keeps it from discovering any repository above
    # the artifact, so paths are always relative to root.
    env = dict(os.environ, GIT_CEILING_DIRECTORIES=str(root.resolve().parent))
    r = subprocess.run(cmd, cwd=root, capture_output=True, text=True, check=False, env=env)
    return None if r.returncode == 0 else (r.stderr.strip().splitlines() or ["failed"])[0]


def build_variant(base: Path, dest: Path, root: str, parts: list[dict], task: Path) -> str | None:
    if dest.exists():
        shutil.rmtree(dest)
    shutil.copytree(base, dest, symlinks=True)
    for p in parts:
        err = apply(dest / root, task / p["patch"])
        if err:
            if p.get("glue"):
                continue
            return f"{p['name']} does not apply: {err}"
    return None


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("task")
    ap.add_argument("--work", default=None)
    ap.add_argument("--keep-images", action="store_true")
    ap.add_argument("--skip-build", action="store_true")
    ap.add_argument("--timeout", type=int, default=1800)
    ap.add_argument("--only", default="", help="comma-separated variant names")
    a = ap.parse_args()

    task = Path(a.task).resolve()
    name = task.name
    spec = json.loads((task / "tests" / "controls.json").read_text())
    root, parts = spec["artifact_root"], spec["parts"]
    causes = [p for p in parts if not p.get("glue")]
    coupled = [set(g) for g in spec.get("coupled", [])]
    fam = json.loads((task / "tests" / "families.json").read_text())
    f2p_all = list(fam["fail_to_pass"])
    paths = [x["source"] for x in tomllib.loads((task / "task.toml").read_text())["artifacts"]]
    work = Path(a.work or tempfile.mkdtemp(prefix=f"controls-{name}-")).resolve()
    env_img, ver_img = f"vbprobe-{name}-env", f"vbprobe-{name}-verifier"
    print(f"run_controls: {name}, {len(causes)} causes, work dir {work}", flush=True)

    if not a.skip_build:
        for img, ctx in ((env_img, "environment"), (ver_img, "tests")):
            print(f"== building {img}", flush=True)
            sh(["docker", "build", "-q", *proxy_args(), "-t", img, str(task / ctx)],
               stdout=subprocess.DEVNULL)
    arts = work / "art"
    if (arts / "base").exists():
        shutil.rmtree(arts / "base")
    extract(env_img, paths, arts / "base")

    # Each variant is a list of part lists to try in order: leave-one-out
    # first leaves out only that part, and falls back to leaving out its
    # coupled group when a later part does not apply without it.
    variants: dict[str, list[list[dict]]] = {"gold-from-parts": [parts]}
    if len(causes) < len(parts):
        variants["causes-only"] = [causes]
    explicit = spec.get("variants")
    for v in explicit or []:
        variants[v["name"]] = [[{"name": v["name"], "patch": v["patch"]}]]
    for c in [] if explicit else causes:
        group = next((g for g in coupled if c["name"] in g), {c["name"]})
        variants[f"single:{c['name']}"] = [[p for p in parts if p.get("glue") or p["name"] == c["name"]]]
        tries = [[p for p in parts if p["name"] != c["name"]]]
        if group != {c["name"]}:
            tries.append([p for p in parts if p["name"] not in group])
        variants[f"loo:{c['name']}"] = tries
    only = {x for x in a.only.split(",") if x}

    report = {}
    for vname, tries in variants.items():
        if only and vname not in only:
            continue
        safe = re.sub(r"[^A-Za-z0-9_.-]", "_", vname)
        errs, used = [], None
        for vparts in tries:
            err = build_variant(arts / "base", arts / safe, root, vparts, task)
            if not err:
                used = vparts
                break
            errs.append(err)
        if used is None:
            report[vname] = {"status": "n/a", "reason": "; ".join(errs)}
            print(f"n/a  {vname:28s} {'; '.join(errs)}", flush=True)
            continue
        left_out = sorted(p["name"] for p in causes if p not in used) if not explicit else []
        out = work / "out" / safe
        r = run_verifier(ver_img, arts / safe, paths, out, a.timeout)
        text = ""
        for f in ("stderr", "check.err"):
            if (out / f).is_file():
                text += (out / f).read_text(errors="replace")
        failing = sorted({m.group(1) for m in F2P_LINE.finditer(text) if m.group(2) != "passed"})
        passing = len(f2p_all) - len(failing)
        report[vname] = {"status": "ran", "causes_absent": left_out,
                         "fallback": errs[0] if errs else None,
                         "reward": r.get("reward"), "f2p_passing": passing,
                         "f2p_total": len(f2p_all), "f2p_failing": failing, "metrics": r}
        note = f" (absent: {','.join(left_out)})" if errs else ""
        print(f"ran  {vname:28s} reward={r.get('reward')} f2p {passing}/{len(f2p_all)}{note} "
              f"failing={failing}", flush=True)

    (work / "controls-report.json").write_text(json.dumps(report, indent=1, sort_keys=True))
    if not a.keep_images:
        subprocess.run(["docker", "rmi", env_img, ver_img], check=False,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    g = report.get("gold-from-parts", {})
    if not only and g.get("reward") != 1:
        print("run_controls: the parts do not compose to a passing gold", flush=True)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
