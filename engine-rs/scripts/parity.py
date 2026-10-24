#!/usr/bin/env python3
"""Acceptance test for the Rust engine spike: run both engines on one task and
diff the normalised artifacts.

Runs ``vulcanbench run`` (Python, from the repo's .venv) and
``vulcanbench-engine run`` (Rust, built with cargo) on the same task with the
mock model, then compares ``summary.json``, ``trace.jsonl`` and
``final.patch`` after replacing the volatile fields:

- run ids (``<task>-<8 hex>``) and the output directories they appear in,
- every ``ts`` / ``started_at`` / ``finished_at`` timestamp,
- ``duration_s``,
- pytest's elapsed time inside the ``run_tests`` observation (``in 0.00s``).

Everything else, including object key order, must match byte for byte.
Exit status 0 means parity; 1 means a diff was printed; 2 means a run failed.

Live mode (``--model anthropic:<model>``) compares two real runs. Model output
is not deterministic, so the comparison is structural: every scalar leaf is
replaced by its type name and lists keep only their first element's shape, which
checks that both engines record the same fields, in the same order, with the
same kinds of values (cost_detail, economics, manifest, source, verifier,
effort), and that the trace carries the same event types with the same data
keys. A live run spends API quota, so the script refuses to start one unless
``--approve-spend`` is passed; without it, it prints what the run would be
(model, task, effort, estimated cost at list price) and exits 3.

Usage: python3 engine-rs/scripts/parity.py [--task hello-world] [--keep]
       [--release] [--skip-build] [--out DIR] [--model SPEC] [--approve-spend]
       [--sandbox local|docker] [--effort LEVEL] [--max-steps N] [--timeout S]
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
ARTIFACTS = ("summary.json", "trace.jsonl", "final.patch")


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


SCALAR_NAMES = ((type(None), "null"), (bool, "bool"), (int, "int"), (float, "float"))


def shape(value):
    """Scalars become their type name, so two live runs compare structurally."""
    if isinstance(value, dict):
        return {k: shape(v) for k, v in value.items()}
    if isinstance(value, list):
        return [shape(value[0])] if value else []
    return next((name for kind, name in SCALAR_NAMES if isinstance(value, kind)), "str")


def load_run(run_dir: Path) -> dict:
    run_id = run_dir.name
    if not RUN_ID_RE.match(run_id):
        raise SystemExit(f"unexpected run dir name {run_id!r}")
    out_dir = run_dir.parent
    summary = json.loads((run_dir / "summary.json").read_text(encoding="utf-8"))
    trace = (run_dir / "trace.jsonl").read_text(encoding="utf-8").splitlines()
    patch = (run_dir / "final.patch").read_text(encoding="utf-8")
    events = normalise_trace(trace, run_id, out_dir)
    data_keys: dict = {}
    for event in events:
        data = event.get("data")
        keys = sorted(data.keys()) if isinstance(data, dict) else type(data).__name__
        data_keys.setdefault(event.get("type"), keys)
    return {
        "summary.json": json.dumps(normalise_summary(summary, run_id, out_dir), indent=2),
        "trace.jsonl": "\n".join(json.dumps(e) for e in events),
        "final.patch": patch,
        "summary.json (shape)": json.dumps(shape(summary), indent=2),
        "trace.jsonl (event types and data keys)": json.dumps(data_keys, indent=2, sort_keys=True),
    }


def compare(python_run: dict, rust_run: dict, live: bool) -> bool:
    names = (
        ("summary.json (shape)", "trace.jsonl (event types and data keys)") if live else ARTIFACTS
    )
    ok = True
    for artifact in names:
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
    return ok


def estimate_cost(model: str, input_tokens: int = 6000, output_tokens: int = 1200):
    """List-price estimate for one engine's hello-world run, via the venv's harness.pricing."""
    code = (
        "from harness.pricing import cost_usd; "
        f"print(cost_usd({model!r}, {input_tokens}, {output_tokens}))"
    )
    proc = subprocess.run(
        [str(venv_bin() / "python"), "-c", code],
        check=False,
        cwd=REPO_ROOT,
        env=run_env(),
        capture_output=True,
        text=True,
    )
    text = proc.stdout.strip()
    return None if proc.returncode != 0 or text in ("", "None") else float(text)


def refuse_unapproved_spend(args: argparse.Namespace) -> int:
    per_engine = estimate_cost(args.model)
    if per_engine is None:
        cost_note = "unpriced model: cost unknown"
    else:
        cost_note = (
            f"about ${per_engine:.2f} per engine at list price, ${2 * per_engine:.2f} for both "
            "(assuming 6,000 input and 1,200 output tokens per run)"
        )
    print("A live parity run spends API quota and needs the owner's approval. It would run:")
    print(f"  model:   {args.model}")
    print(f"  task:    {args.task}")
    print(f"  sandbox: {args.sandbox}")
    print(f"  effort:  {args.effort or '(default)'}")
    print(f"  cost:    {cost_note}")
    print("Re-run with --approve-spend to start it. Nothing was run.")
    return 3


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
    parser.add_argument(
        "--model", default="mock:synthetic", help="model spec for both engines (default mock)"
    )
    parser.add_argument(
        "--approve-spend",
        action="store_true",
        help="allow a live run that spends API quota (the owner's explicit approval)",
    )
    parser.add_argument("--sandbox", default="local", help="sandbox for both engines: local|docker")
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

    live = not args.model.startswith("mock:")
    if live and not args.approve_spend:
        return refuse_unapproved_spend(args)

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
        args.model,
        "--sandbox",
        args.sandbox,
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
        ok = compare(load_run(python_dir), load_run(rust_dir), live)
    finally:
        if args.keep or args.out:
            print(f"runs kept under {base}")
        else:
            shutil.rmtree(base, ignore_errors=True)
    verdict = "STRUCTURAL PARITY" if live else "PARITY"
    print(verdict if ok else f"NO {verdict}")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
