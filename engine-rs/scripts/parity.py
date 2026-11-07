#!/usr/bin/env python3
"""Acceptance test for the Rust engine spike: run both engines on one task and
diff the normalised artifacts.

Runs ``vulcanbench run`` (Python, from the repo's .venv) and
``vulcanbench-engine run`` (Rust, built with cargo) on the same task with the
mock model in the local sandbox, then compares ``summary.json``,
``trace.jsonl`` and ``final.patch`` after replacing the volatile fields:

- run ids (``<task>-<8 hex>``) and the output directories they appear in,
- every ``ts`` / ``started_at`` / ``finished_at`` timestamp,
- ``duration_s``,
- pytest's elapsed time inside the ``run_tests`` observation (``in 0.00s``).

Everything else, including object key order, must match byte for byte.
Exit status 0 means parity; 1 means a diff was printed; 2 means a run failed.

Usage: python3 engine-rs/scripts/parity.py [--task hello-world] [--keep]
       [--release] [--skip-build] [--out DIR]
       [--effort LEVEL] [--max-steps N] [--timeout S]   (passed to both engines)
"""

from __future__ import annotations

import argparse
import difflib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ENGINE_DIR = Path(__file__).resolve().parents[1]
REPO_ROOT = ENGINE_DIR.parent
RUN_ID_RE = re.compile(r"^(?P<task>.+)-[0-9a-f]{8}$")
PYTEST_TIME_RE = re.compile(r" in \d+\.\d+s")


def venv_bin() -> Path:
    override = os.environ.get("VULCANBENCH_VENV")
    if override:
        return Path(override) / "bin"
    return REPO_ROOT / ".venv" / "bin"


def run_env() -> dict:
    env = dict(os.environ)
    env["PATH"] = f"{venv_bin()}{os.pathsep}{env.get('PATH', '')}"
    # The venv's editable finder is the normal import path for ``harness``;
    # the repo root on PYTHONPATH is the same code and survives a venv whose
    # .pth files are unreadable (seen on macOS with a hidden flag on them).
    env["PYTHONPATH"] = os.pathsep.join(p for p in (str(REPO_ROOT), env.get("PYTHONPATH", "")) if p)
    return env


def build_rust(release: bool) -> Path:
    cmd = ["cargo", "build", "--manifest-path", str(ENGINE_DIR / "Cargo.toml")]
    if release:
        cmd.append("--release")
    subprocess.run(cmd, check=True, cwd=REPO_ROOT, env=run_env())
    return ENGINE_DIR / "target" / ("release" if release else "debug") / "vulcanbench-engine"


def run_engine(label: str, argv: list, out_dir: Path) -> Path:
    out_dir.mkdir(parents=True, exist_ok=True)
    cmd = [*argv, "--output-dir", str(out_dir)]
    print(f"[{label}] {' '.join(cmd)}")
    proc = subprocess.run(
        cmd, check=False, cwd=REPO_ROOT, env=run_env(), capture_output=True, text=True
    )
    sys.stdout.write(proc.stdout)
    if proc.returncode != 0:
        sys.stderr.write(proc.stderr)
        raise SystemExit(f"[{label}] exited with {proc.returncode}")
    run_dirs = [p for p in out_dir.iterdir() if p.is_dir()]
    if len(run_dirs) != 1:
        raise SystemExit(f"[{label}] expected one run dir under {out_dir}, found {run_dirs}")
    return run_dirs[0]


def normalise_strings(value, run_id: str, out_dir: Path):
    """Replace run ids, output-dir paths and pytest timings inside every string."""
    if isinstance(value, str):
        value = value.replace(run_id, "<RUN_ID>")
        value = value.replace(str(out_dir), "<OUT_DIR>")
        value = PYTEST_TIME_RE.sub(" in <T>s", value)
        return value
    if isinstance(value, list):
        return [normalise_strings(v, run_id, out_dir) for v in value]
    if isinstance(value, dict):
        return {k: normalise_strings(v, run_id, out_dir) for k, v in value.items()}
    return value


def normalise_summary(summary: dict, run_id: str, out_dir: Path) -> dict:
    summary = normalise_strings(summary, run_id, out_dir)
    for key in ("started_at", "finished_at"):
        if key in summary:
            summary[key] = "<TS>"
    if "duration_s" in summary:
        summary["duration_s"] = "<DURATION>"
    return summary


def normalise_trace(lines: list, run_id: str, out_dir: Path) -> list:
    events = []
    for line in lines:
        if not line.strip():
            continue
        event = json.loads(line)
        event = normalise_strings(event, run_id, out_dir)
        if "ts" in event:
            event["ts"] = "<TS>"
        events.append(event)
    return events


def load_run(run_dir: Path) -> dict:
    run_id = run_dir.name
    if not RUN_ID_RE.match(run_id):
        raise SystemExit(f"unexpected run dir name {run_id!r}")
    out_dir = run_dir.parent
    summary = json.loads((run_dir / "summary.json").read_text(encoding="utf-8"))
    trace = (run_dir / "trace.jsonl").read_text(encoding="utf-8").splitlines()
    patch = (run_dir / "final.patch").read_text(encoding="utf-8")
    return {
        "summary.json": json.dumps(normalise_summary(summary, run_id, out_dir), indent=2),
        "trace.jsonl": "\n".join(json.dumps(e) for e in normalise_trace(trace, run_id, out_dir)),
        "final.patch": patch,
        "raw_summary": (run_dir / "summary.json").read_bytes(),
    }


def compare(python_run: dict, rust_run: dict) -> bool:
    ok = True
    for artifact in ("summary.json", "trace.jsonl", "final.patch"):
        expected = python_run[artifact]
        actual = rust_run[artifact]
        if expected == actual:
            print(f"PASS {artifact}")
            continue
        ok = False
        print(f"FAIL {artifact}")
        diff = difflib.unified_diff(
            expected.splitlines(keepends=True),
            actual.splitlines(keepends=True),
            fromfile=f"python/{artifact}",
            tofile=f"rust/{artifact}",
        )
        sys.stdout.writelines(diff)
        if not expected.endswith("\n") or not actual.endswith("\n"):
            print()
    # Key order and layout: the raw summary bytes must agree once the same
    # normalisation is applied textually (indent, separators, trailing newline).
    return ok


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("--task", default="hello-world")
    parser.add_argument("--keep", action="store_true", help="keep the temporary run directories")
    parser.add_argument(
        "--release", action="store_true", help="build the Rust engine in release mode"
    )
    parser.add_argument("--skip-build", action="store_true", help="use the existing cargo build")
    parser.add_argument("--out", type=Path, default=None, help="directory for the two run outputs")
    parser.add_argument("--effort", default=None, help="pass --effort to both engines")
    parser.add_argument("--max-steps", default=None, help="pass --max-steps to both engines")
    parser.add_argument("--timeout", default=None, help="pass --timeout to both engines")
    args = parser.parse_args()

    if not (venv_bin() / "python").exists():
        print(
            f"no vulcanbench in {venv_bin()}; create the venv first (see engine-rs/README.md)",
            file=sys.stderr,
        )
        return 2

    if args.skip_build:
        binary = (
            ENGINE_DIR / "target" / ("release" if args.release else "debug") / "vulcanbench-engine"
        )
    else:
        binary = build_rust(args.release)

    base = args.out or Path(tempfile.mkdtemp(prefix="vulcanbench-parity-"))
    base.mkdir(parents=True, exist_ok=True)
    common = [
        "run",
        "--task",
        args.task,
        "--model",
        "mock:synthetic",
        "--sandbox",
        "local",
        "--no-judges",
    ]
    for flag in ("effort", "max_steps", "timeout"):
        value = getattr(args, flag)
        if value is not None:
            common += [f"--{flag.replace('_', '-')}", str(value)]
    try:
        python_cli = [str(venv_bin() / "python"), "-c", "from harness.cli import main; main()"]
        python_dir = run_engine("python", [*python_cli, *common], base / "python")
        rust_dir = run_engine("rust", [str(binary), *common], base / "rust")
        ok = compare(load_run(python_dir), load_run(rust_dir))
    finally:
        if args.keep or args.out:
            print(f"runs kept under {base}")
        else:
            shutil.rmtree(base, ignore_errors=True)
    print("PARITY" if ok else "NO PARITY")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
