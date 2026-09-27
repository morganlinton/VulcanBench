"""Code-quality metric: linting + complexity + maintainability on changed files.

Scoped to the agent's changed files (grouped by language). Python is analyzed
natively (ruff + radon). JavaScript runs a pinned ESLint install with the
harness's own config (``harness/data/eslint.config.mjs``); C and C++ run
clang-tidy with a fixed check set (``harness/evaluator/cfamily.py``); Rust runs
``cargo fmt --check`` and clippy. Each reports its tool version in its details.
A language whose tool is missing reports ``None`` with a recorded reason, never
a silent zero. The overall score averages whichever languages were analyzed.
"""

from __future__ import annotations

import json
import math
import os
import shutil
import subprocess
from collections.abc import Callable
from pathlib import Path
from typing import Any

from radon.complexity import cc_visit
from radon.metrics import mi_visit

from harness.evaluator import cfamily
from harness.evaluator.langs import MetricResult, group_by_language

# --- Tunable scoring constants -------------------------------------------------
# Lint: violations per line of changed code beyond which lint_score hits 0.
_LINT_VIOLATIONS_PER_LOC_FLOOR = 0.5
# Complexity: average cyclomatic complexity mapped to 1.0 at/below GOOD, 0.0 at BAD.
_CC_GOOD = 5.0
_CC_BAD = 25.0
# Sub-metric weights within a language's quality score.
_SUB_WEIGHTS = {"lint": 0.4, "complexity": 0.3, "maintainability": 0.3}

RemainingSeconds = Callable[[], float | None]


def assess_quality(
    workspace: Path, changed_files: list[str], remaining_s: RemainingSeconds | None = None
) -> MetricResult:
    """Assess code quality of the agent's changed files."""
    by_lang = group_by_language(changed_files, workspace)
    if not by_lang:
        return MetricResult(score=None, details={"reason": "no recognized source files changed"})

    per_lang: dict[str, Any] = {}
    scores: list[float] = []
    for lang, files in by_lang.items():
        if _budget_exhausted(remaining_s):
            result = MetricResult(score=None, details={"reason": "run budget exceeded"})
        else:
            result = _ANALYZERS.get(lang, _unsupported)(workspace, files, remaining_s)
        per_lang[lang] = result.details | {"score": result.score}
        if result.score is not None:
            scores.append(result.score)

    overall = round(sum(scores) / len(scores), 4) if scores else None
    reason = None if scores else "no quality analyzer available for changed languages"
    return MetricResult(
        score=overall,
        details={"languages": per_lang, **({"reason": reason} if reason else {})},
    )


def _clamp(x: float) -> float:
    return max(0.0, min(1.0, x))


def _timeout(default: int, remaining_s: RemainingSeconds | None) -> int | None:
    if remaining_s is None:
        return default
    remaining = remaining_s()
    if remaining is None:
        return default
    if remaining <= 0:
        return None
    return max(1, min(default, math.ceil(remaining)))


def _budget_exhausted(remaining_s: RemainingSeconds | None) -> bool:
    remaining = remaining_s() if remaining_s is not None else None
    return remaining is not None and remaining <= 0


def _python(
    workspace: Path, files: list[str], remaining_s: RemainingSeconds | None
) -> MetricResult:
    if shutil.which("ruff") is None:
        return MetricResult(score=None, details={"tool": "ruff", "reason": "ruff not on PATH"})

    abs_files = [str((workspace / f).resolve()) for f in files if (workspace / f).exists()]
    if not abs_files:
        return MetricResult(score=None, details={"reason": "changed files no longer exist"})

    loc = sum(_loc(Path(f)) for f in abs_files) or 1
    violations = _ruff_violations(workspace, abs_files, remaining_s)
    if violations is None:
        return MetricResult(
            score=None, details={"tool": "ruff+radon", "reason": "ruff timed out or budget expired"}
        )
    lint_score = _clamp(1.0 - (violations / loc) / _LINT_VIOLATIONS_PER_LOC_FLOOR)

    avg_cc, mi = _radon_metrics(abs_files)
    complexity_score = (
        _clamp((_CC_BAD - avg_cc) / (_CC_BAD - _CC_GOOD)) if avg_cc is not None else None
    )
    mi_score = _clamp(mi / 100.0) if mi is not None else None

    parts = {"lint": lint_score, "complexity": complexity_score, "maintainability": mi_score}
    present = {k: v for k, v in parts.items() if v is not None}
    weight_sum = sum(_SUB_WEIGHTS[k] for k in present) or 1.0
    score = round(sum(_SUB_WEIGHTS[k] * v for k, v in present.items()) / weight_sum, 4)
    return MetricResult(
        score=score,
        details={
            "tool": "ruff+radon",
            "loc": loc,
            "violations": violations,
            "avg_complexity": avg_cc,
            "maintainability_index": round(mi, 2) if mi is not None else None,
            "sub_scores": {k: round(v, 4) for k, v in present.items()},
        },
    )


def _ruff_violations(
    workspace: Path, abs_files: list[str], remaining_s: RemainingSeconds | None
) -> int | None:
    timeout = _timeout(120, remaining_s)
    if timeout is None:
        return None
    try:
        proc = subprocess.run(
            ["ruff", "check", "--output-format=json", "--no-cache", *abs_files],
            cwd=workspace,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired:
        return None
    try:
        return len(json.loads(proc.stdout or "[]"))
    except json.JSONDecodeError:
        return 0


def _radon_metrics(abs_files: list[str]) -> tuple[float | None, float | None]:
    """Average cyclomatic complexity and maintainability index via radon's API."""
    complexities: list[int] = []
    mis: list[float] = []
    for f in abs_files:
        try:
            src = Path(f).read_text(encoding="utf-8")
        except OSError:
            continue
        try:
            complexities.extend(block.complexity for block in cc_visit(src))
            mis.append(mi_visit(src, multi=True))
        except Exception:  # radon raises varied errors (SyntaxError, etc.) on unparsable input
            continue
    avg_cc = sum(complexities) / len(complexities) if complexities else 0.0
    mi = sum(mis) / len(mis) if mis else None
    return avg_cc, mi


def _loc(path: Path) -> int:
    try:
        return sum(1 for line in path.read_text(encoding="utf-8").splitlines() if line.strip())
    except OSError:
        return 0


ESLINT_DIR_DEFAULT = Path.home() / ".local" / "vulcanbench-eslint-10.11.0"
ESLINT_CONFIG = Path(__file__).resolve().parent.parent / "data" / "eslint.config.mjs"


def eslint_dir() -> Path:
    """The pinned ESLint install (VULCANBENCH_ESLINT_DIR, else the default prefix)."""
    return Path(os.environ.get("VULCANBENCH_ESLINT_DIR") or ESLINT_DIR_DEFAULT)


def run_eslint(
    workspace: Path, files: list[str], mode: str, timeout: int
) -> tuple[list[dict[str, Any]], str] | str:
    """Run the pinned ESLint in ``mode``; the reports and version, or a reason string."""
    binary = eslint_dir() / "node_modules" / ".bin" / "eslint"
    if not binary.exists():
        return f"pinned eslint not installed at {eslint_dir()} (scripts/install_analyzers.sh)"
    if shutil.which("node") is None:
        return "node not on PATH"
    existing = [f for f in files if (workspace / f).exists()]
    if not existing:
        return [], cfamily.tool_version(str(binary)) or "unknown"
    env = {
        **os.environ,
        "VULCANBENCH_ESLINT_DIR": str(eslint_dir()),
        "VULCANBENCH_ESLINT_MODE": mode,
    }
    try:
        proc = subprocess.run(
            [
                str(binary),
                "--config",
                str(ESLINT_CONFIG),
                "--format",
                "json",
                "--no-warn-ignored",
                *existing,
            ],
            cwd=workspace,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
            env=env,
        )
    except subprocess.TimeoutExpired:
        return "eslint timed out"
    try:
        reports = json.loads(proc.stdout or "[]")
    except json.JSONDecodeError:
        return f"could not parse eslint output: {proc.stderr.strip()[:200]}"
    return reports, cfamily.tool_version(str(binary)) or "unknown"


def _js_ts(workspace: Path, files: list[str], remaining_s: RemainingSeconds | None) -> MetricResult:
    timeout = _timeout(180, remaining_s)
    if timeout is None:
        return MetricResult(score=None, details={"tool": "eslint", "reason": "run budget exceeded"})
    js_files = [f for f in files if Path(f).suffix.lower() in {".js", ".mjs", ".cjs", ".jsx"}]
    if not js_files:
        return MetricResult(
            score=None, details={"tool": "eslint", "reason": "no JavaScript files changed"}
        )
    result = run_eslint(workspace, js_files, "quality", timeout)
    if isinstance(result, str):
        return MetricResult(score=None, details={"tool": "eslint", "reason": result})
    reports, version = result
    issues = sum(1 for r in reports for m in r.get("messages", []) if m.get("ruleId"))
    parse_errors = sum(
        1 for r in reports for m in r.get("messages", []) if m.get("fatal") or not m.get("ruleId")
    )
    loc = sum(_loc(workspace / f) for f in js_files) or 1
    score = _clamp(1.0 - (issues / loc) / _LINT_VIOLATIONS_PER_LOC_FLOOR)
    if parse_errors:
        score = _clamp(score - 0.3)
    return MetricResult(
        score=score,
        details={
            "tool": "eslint",
            "tool_version": version,
            "issues": issues,
            "parse_errors": parse_errors,
            "loc": loc,
        },
    )


def _typescript(
    workspace: Path, files: list[str], remaining_s: RemainingSeconds | None
) -> MetricResult:
    if shutil.which("npx") is None:
        return MetricResult(score=None, details={"tool": "eslint", "reason": "npx not on PATH"})
    timeout = _timeout(180, remaining_s)
    if timeout is None:
        return MetricResult(score=None, details={"tool": "eslint", "reason": "run budget exceeded"})
    try:
        proc = subprocess.run(
            ["npx", "--no-install", "eslint", "-f", "json", *files],
            cwd=workspace,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired:
        return MetricResult(score=None, details={"tool": "eslint", "reason": "timed out"})
    try:
        reports = json.loads(proc.stdout or "[]")
    except json.JSONDecodeError:
        return MetricResult(
            score=None, details={"tool": "eslint", "reason": "eslint unavailable or no config"}
        )
    errors = sum(r.get("errorCount", 0) + r.get("warningCount", 0) for r in reports)
    loc = sum(_loc(workspace / f) for f in files) or 1
    return MetricResult(
        score=_clamp(1.0 - (errors / loc) / _LINT_VIOLATIONS_PER_LOC_FLOOR),
        details={"tool": "eslint", "issues": errors, "loc": loc},
    )


def _c_family(lang: str) -> Callable[[Path, list[str], RemainingSeconds | None], MetricResult]:
    checks = cfamily.QUALITY_CHECKS + (
        "," + cfamily.QUALITY_CHECKS_CPP_EXTRA if lang == "cpp" else ""
    )

    def analyze(
        workspace: Path, files: list[str], remaining_s: RemainingSeconds | None
    ) -> MetricResult:
        timeout = _timeout(300, remaining_s)
        if timeout is None:
            return MetricResult(
                score=None, details={"tool": "clang-tidy", "reason": "run budget exceeded"}
            )
        result = cfamily.run_clang_tidy(
            workspace, files, lang, checks, timeout, cfamily.QUALITY_CONFIG
        )
        if result is None:
            return MetricResult(
                score=None,
                details={"tool": "clang-tidy", "reason": "clang-tidy not found or timed out"},
            )
        loc = sum(_loc(workspace / f) for f in files if (workspace / f).exists()) or 1
        issues = len(result.findings)
        score = _clamp(1.0 - (issues / loc) / _LINT_VIOLATIONS_PER_LOC_FLOOR)
        return MetricResult(
            score=score,
            details={
                "tool": "clang-tidy",
                "tool_version": result.version,
                "issues": issues,
                "checks": sorted({f.check for f in result.findings}),
                "compile_errors": result.compile_errors,
                "loc": loc,
            },
        )

    return analyze


def _go(workspace: Path, files: list[str], remaining_s: RemainingSeconds | None) -> MetricResult:
    if shutil.which("gofmt") is None:
        return MetricResult(
            score=None, details={"tool": "gofmt/go vet", "reason": "go toolchain not on PATH"}
        )
    timeout = _timeout(120, remaining_s)
    if timeout is None:
        return MetricResult(
            score=None, details={"tool": "gofmt/go vet", "reason": "run budget exceeded"}
        )
    try:
        unformatted = subprocess.run(
            ["gofmt", "-l", *files],
            cwd=workspace,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired:
        return MetricResult(score=None, details={"tool": "gofmt+go vet", "reason": "timed out"})
    bad = [ln for ln in unformatted.stdout.splitlines() if ln.strip()]
    vet_ok = True
    if shutil.which("go") is not None:
        timeout = _timeout(180, remaining_s)
        if timeout is None:
            return MetricResult(
                score=None, details={"tool": "gofmt+go vet", "reason": "run budget exceeded"}
            )
        try:
            vet = subprocess.run(
                ["go", "vet", "./..."],
                cwd=workspace,
                capture_output=True,
                text=True,
                timeout=timeout,
                check=False,
            )
        except subprocess.TimeoutExpired:
            return MetricResult(score=None, details={"tool": "gofmt+go vet", "reason": "timed out"})
        vet_ok = vet.returncode == 0
    score = 1.0 - (0.5 if bad else 0.0) - (0.0 if vet_ok else 0.5)
    return MetricResult(
        score=_clamp(score),
        details={"tool": "gofmt+go vet", "unformatted": len(bad), "vet_ok": vet_ok},
    )


def _java(workspace: Path, files: list[str], remaining_s: RemainingSeconds | None) -> MetricResult:
    if shutil.which("checkstyle") is None:
        return MetricResult(
            score=None, details={"tool": "checkstyle", "reason": "checkstyle not on PATH"}
        )
    timeout = _timeout(180, remaining_s)
    if timeout is None:
        return MetricResult(
            score=None, details={"tool": "checkstyle", "reason": "run budget exceeded"}
        )
    try:
        proc = subprocess.run(
            ["checkstyle", "-f", "xml", *files],
            cwd=workspace,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired:
        return MetricResult(score=None, details={"tool": "checkstyle", "reason": "timed out"})
    issues = proc.stdout.count("<error ")
    loc = sum(_loc(workspace / f) for f in files) or 1
    return MetricResult(
        score=_clamp(1.0 - (issues / loc) / _LINT_VIOLATIONS_PER_LOC_FLOOR),
        details={"tool": "checkstyle", "issues": issues, "loc": loc},
    )


def _rust(workspace: Path, files: list[str], remaining_s: RemainingSeconds | None) -> MetricResult:  # noqa: PLR0912
    if shutil.which("cargo") is None:
        return MetricResult(
            score=None, details={"tool": "cargo fmt+clippy", "reason": "cargo not on PATH"}
        )
    if _budget_exhausted(remaining_s):
        return MetricResult(
            score=None, details={"tool": "cargo fmt+clippy", "reason": "run budget exceeded"}
        )

    loc = sum(_loc(workspace / f) for f in files if (workspace / f).exists()) or 1

    changed_abs = {str((workspace / f).resolve()) for f in files if (workspace / f).exists()}

    # --- cargo fmt --check (formatting) ---
    fmt_timeout = _timeout(120, remaining_s)
    fmt_bad = 0
    if fmt_timeout is not None:
        try:
            fmt_proc = subprocess.run(
                ["cargo", "fmt", "--check", "--"],
                cwd=workspace,
                capture_output=True,
                text=True,
                timeout=fmt_timeout,
                check=False,
            )
            # cargo fmt --check exits 1 when files need formatting and prints one
            # "Diff in <abs path>:<line>:" header per hunk on stdout. Count the
            # hunks in the changed files.
            for ln in fmt_proc.stdout.splitlines():
                if ln.startswith("Diff in "):
                    path = ln[len("Diff in ") :].rsplit(":", 2)[0]
                    if str(Path(path).resolve()) in changed_abs:
                        fmt_bad += 1
        except subprocess.TimeoutExpired:
            return MetricResult(
                score=None, details={"tool": "cargo fmt+clippy", "reason": "cargo fmt timed out"}
            )

    # --- cargo clippy (lint) ---
    clippy_timeout = _timeout(300, remaining_s)
    clippy_warnings = 0
    clippy_ok = True
    if clippy_timeout is not None:
        try:
            clippy_proc = subprocess.run(
                ["cargo", "clippy", "--offline", "--message-format=json", "--no-deps", "--"],
                cwd=workspace,
                capture_output=True,
                text=True,
                timeout=clippy_timeout,
                check=False,
            )
        except subprocess.TimeoutExpired:
            return MetricResult(
                score=None, details={"tool": "cargo fmt+clippy", "reason": "cargo clippy timed out"}
            )
        # Cargo wraps each rustc/clippy diagnostic as reason "compiler-message";
        # span file names are relative to the crate root. Spanless messages are
        # summaries ("generated 3 warnings") and are not findings. An error-level
        # message means the crate did not build.
        try:
            for raw_line in clippy_proc.stdout.splitlines():
                stripped = raw_line.strip()
                if not stripped:
                    continue
                try:
                    msg = json.loads(stripped)
                except json.JSONDecodeError:
                    continue
                if msg.get("reason") != "compiler-message":
                    continue
                message = msg.get("message", {})
                if message.get("level") == "error":
                    clippy_ok = False
                    continue
                if message.get("level") != "warning":
                    continue
                spans = message.get("spans", [])
                if any(
                    str((workspace / s.get("file_name", "")).resolve()) in changed_abs
                    for s in spans
                ):
                    clippy_warnings += 1
        except Exception:
            clippy_ok = False
    else:
        clippy_ok = False

    violations = fmt_bad + clippy_warnings
    lint_score = _clamp(1.0 - (violations / loc) / _LINT_VIOLATIONS_PER_LOC_FLOOR)
    score = _clamp(lint_score - (0.0 if clippy_ok else 0.3))
    return MetricResult(
        score=score,
        details={
            "tool": "cargo fmt+clippy",
            "tool_version": cfamily.tool_version("cargo-clippy"),
            "loc": loc,
            "unformatted": fmt_bad,
            "clippy_warnings": clippy_warnings,
            "clippy_ok": clippy_ok,
        },
    )


def _unsupported(
    workspace: Path, files: list[str], remaining_s: RemainingSeconds | None
) -> MetricResult:
    del workspace, files, remaining_s
    return MetricResult(score=None, details={"reason": "no quality analyzer for this language"})


# Per-language analyzer registry.
_ANALYZERS = {
    "python": _python,
    # TypeScript keeps the workspace's own ESLint (the pinned config has no TS parser).
    "typescript": _typescript,
    "javascript": _js_ts,
    "go": _go,
    "java": _java,
    "rust": _rust,
    "c": _c_family("c"),
    "cpp": _c_family("cpp"),
}
