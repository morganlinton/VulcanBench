"""Security metric: static-analysis scanners on changed files, severity-weighted.

Python uses bandit natively. JS/TS use ``npm audit`` (needs a manifest), Go uses
gosec, Java uses spotbugs, Rust uses ``cargo audit`` -- each only when the tool
is present, otherwise the language reports ``None`` with a reason. Score per
language is ``1 - (0.4*high + 0.15*med + 0.05*low)`` clamped to [0, 1]; the
overall score averages whichever languages were scanned.

For Rust, an additional "unsafe delta" penalty applies: 0.05 is subtracted per
net-new ``unsafe`` keyword in the agent patch, ``max(0, added - removed)`` over the
scored ``.rs`` files (clamped to [0, 1]). Details report ``unsafe_added``,
``unsafe_removed``, ``unsafe_delta``, ``unsafe_penalty`` and ``unsafe_basis``. A
caller with no patch (the live ``security_scan`` tool) gets the whole-file count of
the files it names instead, reported as ``unsafe_basis = "workspace_count"``.
"""

from __future__ import annotations

import json
import math
import re
import shutil
import subprocess
from collections.abc import Callable
from pathlib import Path
from typing import Any

from harness.evaluator.langs import MetricResult, group_by_language

_SEVERITY_PENALTY = {"high": 0.4, "medium": 0.15, "low": 0.05}

RemainingSeconds = Callable[[], float | None]


def assess_security(
    workspace: Path,
    changed_files: list[str],
    remaining_s: RemainingSeconds | None = None,
    *,
    patch: str | None = None,
) -> MetricResult:
    """Assess security of the agent's changed files."""
    by_lang = group_by_language(changed_files)
    if not by_lang:
        return MetricResult(score=None, details={"reason": "no recognized source files changed"})

    per_lang: dict[str, Any] = {}
    scores: list[float] = []
    for lang, files in by_lang.items():
        if _budget_exhausted(remaining_s):
            result = MetricResult(score=None, details={"reason": "run budget exceeded"})
        elif lang == "rust":
            result = _rust(workspace, files, remaining_s, patch=patch)
        else:
            result = _SCANNERS.get(lang, _unsupported)(workspace, files, remaining_s)
        per_lang[lang] = result.details | {"score": result.score}
        if result.score is not None:
            scores.append(result.score)

    overall = round(sum(scores) / len(scores), 4) if scores else None
    reason = None if scores else "no security scanner available for changed languages"
    return MetricResult(
        score=overall,
        details={"languages": per_lang, **({"reason": reason} if reason else {})},
    )


def score_from_counts(high: int, medium: int, low: int) -> float:
    """Severity-weighted score in [0, 1] from issue counts."""
    penalty = (
        _SEVERITY_PENALTY["high"] * high
        + _SEVERITY_PENALTY["medium"] * medium
        + _SEVERITY_PENALTY["low"] * low
    )
    return round(max(0.0, min(1.0, 1.0 - penalty)), 4)


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
    if shutil.which("bandit") is None:
        return MetricResult(score=None, details={"tool": "bandit", "reason": "bandit not on PATH"})
    abs_files = [str((workspace / f).resolve()) for f in files if (workspace / f).exists()]
    if not abs_files:
        return MetricResult(score=None, details={"reason": "changed files no longer exist"})
    timeout = _timeout(180, remaining_s)
    if timeout is None:
        return MetricResult(score=None, details={"tool": "bandit", "reason": "run budget exceeded"})
    try:
        proc = subprocess.run(
            ["bandit", "-f", "json", "-q", *abs_files],
            cwd=workspace,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired:
        return MetricResult(score=None, details={"tool": "bandit", "reason": "timed out"})
    try:
        results = json.loads(proc.stdout or "{}").get("results", [])
    except json.JSONDecodeError:
        return MetricResult(
            score=None, details={"tool": "bandit", "reason": "could not parse bandit output"}
        )
    counts = {"HIGH": 0, "MEDIUM": 0, "LOW": 0}
    skipped_test_asserts = 0
    for finding in results:
        if finding.get("test_id") == "B101" and _is_test_file(finding.get("filename", "")):
            skipped_test_asserts += 1  # an assert in a test is the test, not a weakness
            continue
        severity = str(finding.get("issue_severity", "")).upper()
        if severity in counts:
            counts[severity] += 1
    high, medium, low = counts["HIGH"], counts["MEDIUM"], counts["LOW"]
    details: dict[str, Any] = {"tool": "bandit", "high": high, "medium": medium, "low": low}
    if skipped_test_asserts:
        details["skipped_test_asserts"] = skipped_test_asserts
    return MetricResult(score=score_from_counts(high, medium, low), details=details)


def _is_test_file(filename: str) -> bool:
    """Test code by the usual conventions: a tests/ directory or a test_*.py / *_test.py name."""
    path = Path(filename)
    name = path.name
    return (
        "tests" in path.parts[:-1]
        or "test" in path.parts[:-1]
        or name.startswith("test_")
        or name.endswith("_test.py")
        or name == "conftest.py"
    )


def _js_ts(workspace: Path, files: list[str], remaining_s: RemainingSeconds | None) -> MetricResult:
    if shutil.which("npm") is None:
        return MetricResult(score=None, details={"tool": "npm audit", "reason": "npm not on PATH"})
    if not (workspace / "package.json").exists():
        return MetricResult(
            score=None, details={"tool": "npm audit", "reason": "no package.json in workspace"}
        )
    timeout = _timeout(180, remaining_s)
    if timeout is None:
        return MetricResult(
            score=None, details={"tool": "npm audit", "reason": "run budget exceeded"}
        )
    try:
        proc = subprocess.run(
            ["npm", "audit", "--json"],
            cwd=workspace,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired:
        return MetricResult(score=None, details={"tool": "npm audit", "reason": "timed out"})
    try:
        vulns = json.loads(proc.stdout or "{}").get("metadata", {}).get("vulnerabilities", {})
    except json.JSONDecodeError:
        return MetricResult(
            score=None, details={"tool": "npm audit", "reason": "could not parse npm audit output"}
        )
    high = int(vulns.get("high", 0)) + int(vulns.get("critical", 0))
    medium = int(vulns.get("moderate", 0))
    low = int(vulns.get("low", 0)) + int(vulns.get("info", 0))
    return MetricResult(
        score=score_from_counts(high, medium, low),
        details={"tool": "npm audit", "high": high, "medium": medium, "low": low},
    )


def _go(workspace: Path, files: list[str], remaining_s: RemainingSeconds | None) -> MetricResult:
    if shutil.which("gosec") is None:
        return MetricResult(score=None, details={"tool": "gosec", "reason": "gosec not on PATH"})
    timeout = _timeout(180, remaining_s)
    if timeout is None:
        return MetricResult(score=None, details={"tool": "gosec", "reason": "run budget exceeded"})
    try:
        proc = subprocess.run(
            ["gosec", "-fmt=json", "./..."],
            cwd=workspace,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired:
        return MetricResult(score=None, details={"tool": "gosec", "reason": "timed out"})
    try:
        issues = json.loads(proc.stdout or "{}").get("Issues", [])
    except json.JSONDecodeError:
        return MetricResult(
            score=None, details={"tool": "gosec", "reason": "could not parse gosec output"}
        )
    high = sum(1 for i in issues if i.get("severity") == "HIGH")
    medium = sum(1 for i in issues if i.get("severity") == "MEDIUM")
    low = sum(1 for i in issues if i.get("severity") == "LOW")
    return MetricResult(
        score=score_from_counts(high, medium, low),
        details={"tool": "gosec", "high": high, "medium": medium, "low": low},
    )


def _java(workspace: Path, files: list[str], remaining_s: RemainingSeconds | None) -> MetricResult:
    if shutil.which("spotbugs") is None:
        return MetricResult(
            score=None, details={"tool": "spotbugs", "reason": "spotbugs not on PATH"}
        )
    timeout = _timeout(240, remaining_s)
    if timeout is None:
        return MetricResult(
            score=None, details={"tool": "spotbugs", "reason": "run budget exceeded"}
        )
    try:
        proc = subprocess.run(
            ["spotbugs", "-textui", "-xml:withMessages", *files],
            cwd=workspace,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired:
        return MetricResult(score=None, details={"tool": "spotbugs", "reason": "timed out"})
    # SpotBugs ranks 1-20; treat as one bucket of "medium" findings for v1.
    findings = proc.stdout.count("<BugInstance ")
    return MetricResult(
        score=score_from_counts(0, findings, 0),
        details={"tool": "spotbugs", "findings": findings},
    )


def _rust(  # noqa: PLR0911
    workspace: Path,
    files: list[str],
    remaining_s: RemainingSeconds | None,
    *,
    patch: str | None = None,
) -> MetricResult:
    if shutil.which("cargo") is None:
        return MetricResult(
            score=None, details={"tool": "cargo audit", "reason": "cargo not on PATH"}
        )
    if not (workspace / "Cargo.lock").exists():
        return MetricResult(
            score=None, details={"tool": "cargo audit", "reason": "no Cargo.lock in workspace"}
        )
    if _budget_exhausted(remaining_s):
        return MetricResult(
            score=None, details={"tool": "cargo audit", "reason": "run budget exceeded"}
        )
    timeout = _timeout(180, remaining_s)
    if timeout is None:
        return MetricResult(
            score=None, details={"tool": "cargo audit", "reason": "run budget exceeded"}
        )
    try:
        proc = subprocess.run(
            ["cargo", "audit", "--json"],
            cwd=workspace,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired:
        return MetricResult(score=None, details={"tool": "cargo audit", "reason": "timed out"})

    # cargo audit exits non-zero when vulnerabilities are found; JSON is on stdout.
    try:
        report = json.loads(proc.stdout or "{}")
    except json.JSONDecodeError:
        return MetricResult(
            score=None,
            details={"tool": "cargo audit", "reason": "could not parse cargo audit output"},
        )

    vulnerabilities = report.get("vulnerabilities", {})
    counts = vulnerabilities.get("count", 0) if isinstance(vulnerabilities, dict) else 0
    # Iterate individual vulnerability entries to map severities.
    vuln_list = vulnerabilities.get("list", []) if isinstance(vulnerabilities, dict) else []
    high = 0
    medium = 0
    low = 0
    for v in vuln_list:
        severity = str(v.get("severity", "")).lower()
        advisory = v.get("advisory", {})
        if not severity and isinstance(advisory, dict):
            severity = str(advisory.get("severity", "")).lower()
        if severity in ("critical", "high"):
            high += 1
        elif severity == "medium":
            medium += 1
        else:
            low += 1

    base_score = score_from_counts(high, medium, low)

    unsafe_details = _unsafe_details(workspace, files, patch)
    unsafe_penalty = round(min(1.0, 0.05 * unsafe_details["unsafe_delta"]), 4)
    final_score = (
        round(max(0.0, base_score - unsafe_penalty), 4) if base_score is not None else None
    )

    return MetricResult(
        score=final_score,
        details={
            "tool": "cargo audit",
            "vulnerabilities": counts,
            "high": high,
            "medium": medium,
            "low": low,
            **unsafe_details,
            "unsafe_penalty": unsafe_penalty,
        },
    )


_UNSAFE_RE = re.compile(r"\bunsafe\b")


def _unsafe_details(workspace: Path, files: list[str], patch: str | None) -> dict[str, Any]:
    # Benchmark grading has the exact agent patch; the live security_scan tool
    # does not, so it keeps the workspace count and says so.
    if patch is None:
        return {
            "unsafe_delta": _count_unsafe_delta(workspace, files),
            "unsafe_basis": "workspace_count",
        }
    return {**_unsafe_delta_details(patch, files), "unsafe_basis": "patch_net_delta"}


def _unsafe_delta_details(patch: str, files: list[str]) -> dict[str, int]:
    """Count the positive net ``unsafe`` change in the requested Rust files."""
    rust_files = {Path(f).as_posix() for f in files if f.endswith(".rs")}
    unsafe_added = 0
    unsafe_removed = 0
    for section in re.split(r"(?m)(?=^diff --git )", patch):
        if not rust_files.intersection(_diff_section_paths(section)):
            continue
        in_hunk = False
        for line in section.splitlines():
            if line.startswith("@@"):
                in_hunk = True
                continue
            if not in_hunk:
                continue
            if line.startswith("+"):
                unsafe_added += len(_UNSAFE_RE.findall(line[1:]))
            elif line.startswith("-"):
                unsafe_removed += len(_UNSAFE_RE.findall(line[1:]))
    return {
        "unsafe_added": unsafe_added,
        "unsafe_removed": unsafe_removed,
        "unsafe_delta": max(0, unsafe_added - unsafe_removed),
    }


def _diff_section_paths(section: str) -> set[str]:
    """Return normalized old/new file paths from one unified-diff section."""
    paths: set[str] = set()
    for line in section.splitlines():
        if line.startswith("@@"):
            break
        if not line.startswith(("--- ", "+++ ")):
            continue
        path = _normalize_diff_path(line[4:])
        if path is not None:
            paths.add(path)
    return paths


def _normalize_diff_path(raw: str) -> str | None:
    """Normalize a ``---``/``+++`` Git diff path for changed-file matching."""
    path = raw.strip().split("\t", 1)[0]
    if path == "/dev/null":
        return None
    if path.startswith('"') and path.endswith('"'):
        path = path[1:-1]
    if path.startswith(("a/", "b/")):
        path = path[2:]
    return Path(path).as_posix()


def _count_unsafe_delta(workspace: Path, files: list[str]) -> int:
    """Count ``unsafe`` keywords in changed Rust files for patchless live scans."""
    count = 0
    for f in files:
        if not f.endswith(".rs"):
            continue
        path = workspace / f
        try:
            src = path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        count += len(_UNSAFE_RE.findall(src))
    return count


def _unsupported(
    workspace: Path, files: list[str], remaining_s: RemainingSeconds | None
) -> MetricResult:
    del workspace, files, remaining_s
    return MetricResult(score=None, details={"reason": "no security scanner for this language"})


# Per-language scanner registry.
_SCANNERS = {
    "python": _python,
    "typescript": _js_ts,
    "javascript": _js_ts,
    "go": _go,
    "java": _java,
    "rust": _rust,
}
