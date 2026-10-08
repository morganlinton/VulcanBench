#!/usr/bin/env python3
"""Attack a Frontier v5 task's verifier with its declared cheating probes.

usage: scripts/frontier-v5/probe_verifier.py <task-dir> [--work DIR] [--keep-images]
                                             [--only NAME[,NAME...]] [--skip-build]

A verifier is only trustworthy if graded code that fixes nothing cannot score.
Each task declares, in tests/probes.json, a few such "probes": edits applied to
the unfixed base artifact that try to win without fixing the bug (exit the test
process early with status 0, plant a conftest.py, shadow the test framework,
neuter assertions in-process, ...). This script

  1. builds the task's environment and verifier images (on a cloud VM it routes
     the builds through the session proxy, like setup-host.sh);
  2. extracts the base artifact from the environment image and the gold
     artifact by running solution/solve.sh in a fresh environment container;
  3. builds one artifact per probe by applying its edits to a copy of base;
  4. runs the real verifier on every artifact exactly as Harbor would grade it
     (separate container, no network, artifacts mounted read-only);
  5. passes only if base scores 0, gold scores 1 and every probe scores 0.

It prints one line per artifact with the verifier's metrics, writes
<work>/probe-report.json, and exits 1 on any violation. See
docs/frontier-v5/PHASE1.md, "Verifier audit 2026-10-08".

probes.json format (paths are relative to the artifact root, so a probe for
artifact /app/src edits "app/src/..."):

  {"probes": [
    {"name": "exit", "why": "...",
     "edits": [{"file": "app/pkg/__init__.py", "append": "..."},
               {"file": "app/pkg/conftest.py", "create": "..."},
               {"file": "app/src/lib.rs", "after": "<regex>", "insert": "..."},
               {"file": "app/src/lib.rs", "before": "<regex>", "insert": "..."}]}
  ]}

"append" adds text at the end of an existing file, "create" writes a new file
(directories included), "after" and "before" insert text next to the first
line matching the regex. A probe that cannot be applied is an error, not a pass.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
from pathlib import Path


def sh(cmd: list[str], **kw) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, check=True, **kw)


def proxy_args() -> list[str]:
    proxy = os.environ.get("HTTPS_PROXY")
    if proxy and Path("/root/.ccr").is_dir():
        return [
            "--network",
            "host",
            "--build-arg",
            f"HTTPS_PROXY={proxy}",
            "--build-arg",
            f"https_proxy={proxy}",
        ]
    return []


def extract(
    image: str, paths: list[str], dest: Path, command: str = "true", mounts: list[str] | None = None
) -> None:
    dest.mkdir(parents=True, exist_ok=True)
    rel = " ".join(p.lstrip("/") for p in paths)
    run = ["docker", "run", "--rm"]
    for m in mounts or []:
        run += ["-v", m]
    run += [image, "bash", "-c", f"{command} >&2 && tar -C / -c {rel}"]
    with tempfile.TemporaryFile() as buf:
        subprocess.run(run, check=True, stdout=buf)
        buf.seek(0)
        with tarfile.open(fileobj=buf) as tar:
            tar.extractall(dest, filter="tar")


def apply_edit(root: Path, edit: dict) -> None:
    target = root / edit["file"]
    if "create" in edit:
        if target.exists():
            raise ValueError(f"create: {edit['file']} already exists")
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(edit["create"])
    elif "append" in edit:
        if not target.is_file():
            raise ValueError(f"append: {edit['file']} not found")
        with target.open("a") as f:
            f.write(edit["append"])
    elif "after" in edit or "before" in edit:
        kind = "after" if "after" in edit else "before"
        lines = target.read_text().splitlines(keepends=True)
        pat = re.compile(edit[kind])
        text = edit["insert"] if edit["insert"].endswith("\n") else edit["insert"] + "\n"
        for i, line in enumerate(lines):
            if pat.search(line):
                lines.insert(i + 1 if kind == "after" else i, text)
                break
        else:
            raise ValueError(f"{kind}: no line in {edit['file']} matches {edit[kind]!r}")
        target.write_text("".join(lines))
    else:
        raise ValueError(f"unknown edit kind in {edit}")


def run_verifier(image: str, art: Path, paths: list[str], out: Path, timeout: int) -> dict:
    out.mkdir(parents=True, exist_ok=True)
    cmd = ["docker", "run", "--rm", "--network", "none", "--cpus", "2", "--memory", "4g"]
    for p in paths:
        cmd += ["-v", f"{art}{p}:{p}:ro"]
    cmd += ["-v", f"{out}:/logs/verifier", image, "bash", "/test.sh"]
    cname = f"vbprobe-run-{os.getpid()}-{out.name}"
    cmd[3:3] = ["--name", cname]
    with (out / "stdout").open("w") as so, (out / "stderr").open("w") as se:
        try:
            subprocess.run(cmd, stdout=so, stderr=se, check=False, timeout=timeout)
        except subprocess.TimeoutExpired:
            subprocess.run(
                ["docker", "kill", cname],
                check=False,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
            return {"reward": None, "timed_out": 1}
    try:
        return json.loads((out / "reward.json").read_text())
    except (OSError, json.JSONDecodeError):
        return {"reward": None}


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("task")
    ap.add_argument("--work", default=None, help="scratch directory (default: a new temp dir)")
    ap.add_argument("--keep-images", action="store_true")
    ap.add_argument(
        "--skip-build", action="store_true", help="reuse images built by an earlier run"
    )
    ap.add_argument("--only", default="", help="comma-separated probe names to run")
    ap.add_argument(
        "--timeout",
        type=int,
        default=1800,
        help="seconds per verifier run (default 1800, Harbor's verifier timeout in task.toml)",
    )
    a = ap.parse_args()

    if subprocess.run(
        ["docker", "info"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False
    ).returncode:
        print(
            "probe_verifier: docker daemon not reachable (on a cloud VM: start dockerd)",
            file=sys.stderr,
        )
        return 2
    task = Path(a.task).resolve()
    name = task.name
    spec = json.loads((task / "tests" / "probes.json").read_text())
    paths = [x["source"] for x in tomllib.loads((task / "task.toml").read_text())["artifacts"]]
    work = Path(a.work or tempfile.mkdtemp(prefix=f"probe-{name}-")).resolve()
    env_img, ver_img = f"vbprobe-{name}-env", f"vbprobe-{name}-verifier"
    print(f"probe_verifier: {name}, artifacts {paths}, work dir {work}", flush=True)

    if not a.skip_build:
        for img, ctx in ((env_img, "environment"), (ver_img, "tests")):
            print(f"== building {img}", flush=True)
            sh(
                ["docker", "build", "-q", *proxy_args(), "-t", img, str(task / ctx)],
                stdout=subprocess.DEVNULL,
            )

    arts = work / "art"
    for kind in ("base", "gold"):
        if (arts / kind).exists():
            shutil.rmtree(arts / kind)
    extract(env_img, paths, arts / "base")
    extract(
        env_img,
        paths,
        arts / "gold",
        "bash /solution/solve.sh",
        [f"{task / 'solution'}:/solution:ro"],
    )

    only = {x for x in a.only.split(",") if x}
    probes = [p for p in spec["probes"] if not only or p["name"] in only]
    for p in probes:
        dest = arts / f"probe-{p['name']}"
        if dest.exists():
            shutil.rmtree(dest)
        shutil.copytree(arts / "base", dest, symlinks=True)
        for e in p["edits"]:
            apply_edit(dest, e)

    expected = {"base": 0, "gold": 1} | {f"probe-{p['name']}": 0 for p in probes}
    report, bad = {}, []
    for art, want in expected.items():
        r = run_verifier(ver_img, arts / art, paths, work / "out" / art, a.timeout)
        report[art] = r
        ok = r.get("reward") == want
        if not ok:
            bad.append(art)
        metrics = ", ".join(f"{k}={v}" for k, v in sorted(r.items()) if k != "reward")
        print(
            f"{'ok  ' if ok else 'FAIL'} {art:28s} reward={r.get('reward')} (want {want})  {metrics}",
            flush=True,
        )

    (work / "probe-report.json").write_text(json.dumps(report, indent=1, sort_keys=True))
    if not a.keep_images:
        subprocess.run(
            ["docker", "rmi", env_img, ver_img],
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
    if bad:
        print(f"probe_verifier: {name}: FAILED for {', '.join(bad)}", flush=True)
        return 1
    print(f"probe_verifier: {name}: base 0, gold 1, {len(probes)} probes all 0", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
