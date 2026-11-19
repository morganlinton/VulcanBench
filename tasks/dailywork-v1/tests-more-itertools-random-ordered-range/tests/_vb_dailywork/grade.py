"""DailyWork tests-category grader (verification only, never shown to the agent).

Each check is one declarative test command:

    python _vb_dailywork/grade.py original          # agent's tests pass on the real code
    python _vb_dailywork/grade.py variant-unparse   # ... on a reformatted, comment-free copy
    python _vb_dailywork/grade.py variant-shift     # ... with the code moved down a few lines
    python _vb_dailywork/grade.py tamper            # added test code does not read source or bytecode
    python _vb_dailywork/grade.py mutant m007       # exits 0 only if the agent's tests FAIL on m007

Every run copies the workspace to a scratch directory, restores the package
under test from a pristine copy (so changes the agent made to library code
never count), applies the variant or mutant, and runs the test command from
config.json. The mutants are single edits to the code the change added,
pre-selected because the upstream project's own tests for the change catch
them and the tests that existed before the change do not.
"""

from __future__ import annotations

import ast
import difflib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
WORKSPACE = HERE.parent
CONFIG = json.loads((HERE / "config.json").read_text())
MUTANTS = json.loads((HERE / "mutants.json").read_text())

# Test code may not inspect the implementation's text, bytecode or file:
# such a test fails on any edit (so it "kills" every mutant) without checking
# behavior. The variant checks catch most of these; this catches the rest.
FORBIDDEN = re.compile(
    r"__code__|co_code|co_consts|\bdis\b\.|getsource|getsourcelines|hashlib|"
    r"\bmd5\b|\bsha\d|zlib\.crc32|__file__|read_text\(|read_bytes\(|"
    r"importlib\.reload|sys\.settrace|tracemalloc"
)


def _copy_workspace(dest: Path) -> None:
    def ignore(directory: str, names: list[str]) -> set[str]:
        skip = {".git", "_vb_dailywork", "__pycache__", ".pytest_cache"}
        return {n for n in names if n in skip}

    shutil.copytree(WORKSPACE, dest, ignore=ignore, symlinks=True)
    package = dest / CONFIG["package"]
    shutil.rmtree(package, ignore_errors=True)
    shutil.copytree(HERE / "pristine" / CONFIG["package"], package)


def _target(root: Path) -> Path:
    return root / CONFIG["module"]


def _apply_mutant(root: Path, mutant: dict) -> None:
    path = _target(root)
    lines = path.read_text().splitlines(keepends=True)
    start = mutant["start"]
    old = mutant["old"]
    if "".join(lines[start : start + len(old)]) != "".join(old):
        raise SystemExit(f"mutant {mutant['id']} does not match the pristine module")
    lines[start : start + len(old)] = mutant["new"]
    path.write_text("".join(lines))


def _variant_unparse(root: Path) -> None:
    path = _target(root)
    path.write_text(ast.unparse(ast.parse(path.read_text())) + "\n")


def _variant_shift(root: Path) -> None:
    path = _target(root)
    lines = path.read_text().splitlines(keepends=True)
    anchor = CONFIG["shift_before"]
    index = next(i for i, line in enumerate(lines) if line.startswith(anchor))
    filler = ["# (verification copy: lines inserted to move the code below)\n"] * 7
    lines[index:index] = filler
    path.write_text("".join(lines))


def _run_tests(root: Path, stop_first: bool) -> tuple[int, bool]:
    cmd = [sys.executable, "-m", "pytest", "-q", "-p", "no:cacheprovider", *CONFIG["pytest_args"]]
    if stop_first:
        cmd.append("-x")
    env = {**os.environ, "PYTHONPATH": str(root), "PYTHONHASHSEED": "0", "PYTHONDONTWRITEBYTECODE": "1"}
    try:
        proc = subprocess.run(cmd, cwd=root, env=env, timeout=CONFIG["timeout_s"], check=False,
                              stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    except subprocess.TimeoutExpired:
        print(f"tests timed out after {CONFIG['timeout_s']} s")
        return 1, True
    print("\n".join(proc.stdout.splitlines()[-15:]))
    return proc.returncode, False


def _added_test_lines() -> list[tuple[str, str]]:
    base_dir = HERE / "base_tests"
    added: list[tuple[str, str]] = []
    for path in sorted((WORKSPACE / "tests").rglob("*.py")):
        rel = path.relative_to(WORKSPACE / "tests")
        new = path.read_text(errors="replace").splitlines()
        base_file = base_dir / rel
        old = base_file.read_text().splitlines() if base_file.exists() else []
        for line in difflib.ndiff(old, new):
            if line.startswith("+ "):
                added.append((str(rel), line[2:]))
    for extra in ("conftest.py", "pyproject.toml", "setup.cfg", "tox.ini", "pytest.ini"):
        path = WORKSPACE / extra
        pristine = HERE / "base_root" / extra
        if path.exists() and (not pristine.exists() or path.read_bytes() != pristine.read_bytes()):
            added.append((extra, "(test configuration changed)"))
    return added


def main(argv: list[str]) -> int:
    check = argv[0]
    if check == "tamper":
        hits = [(f, l) for f, l in _added_test_lines() if FORBIDDEN.search(l) or l == "(test configuration changed)"]
        for f, l in hits[:20]:
            print(f"{f}: {l.strip()}")
        return 1 if hits else 0
    with tempfile.TemporaryDirectory(prefix="vb-grade-") as tmp:
        root = Path(tmp) / "ws"
        _copy_workspace(root)
        if check == "original":
            return _run_tests(root, stop_first=False)[0]
        if check == "variant-unparse":
            _variant_unparse(root)
            return _run_tests(root, stop_first=False)[0]
        if check == "variant-shift":
            _variant_shift(root)
            return _run_tests(root, stop_first=False)[0]
        if check == "mutant":
            mutant = next(m for m in MUTANTS if m["id"] == argv[1])
            _apply_mutant(root, mutant)
            code, _ = _run_tests(root, stop_first=True)
            print("caught" if code != 0 else "missed")
            return 0 if code != 0 else 1
    raise SystemExit(f"unknown check {check!r}")


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
