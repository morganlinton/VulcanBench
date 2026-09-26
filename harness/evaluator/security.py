"""Security metric: static-analysis scanners on changed files, severity-weighted.

Python uses bandit natively over the whole of each changed file. Go uses gosec
and Java spotbugs. JavaScript runs ``eslint-plugin-security`` through the pinned
ESLint install, plus ``npm audit`` when the workspace declares dependencies.
C and C++ run the clang static analyzer's memory-safety and insecure-API checks
(through clang-tidy) and cppcheck. Rust runs ``cargo audit`` when the crate has
external dependencies. Each runs only when its tool is present, otherwise the
language reports ``None`` with a reason. Score per language is
``1 - (0.4*high + 0.15*med + 0.05*low)`` clamped to [0, 1]; the overall score
averages whichever languages were scanned.

The JavaScript, C and C++ scanners count only findings on lines the change
added (``harness/evaluator/diffscope.py``): the C and C++ analyzers report false
positives on untouched, sanitizer-clean code, and a whole-file count would
charge every run for them alike. Outside a git workspace they count every
finding in the changed files.

For Rust, an additional "unsafe delta" penalty applies: 0.05 is subtracted per
``unsafe`` keyword on an added, non-comment line of a changed file (clamped to
[0, 1]). This is reported in details as ``unsafe_delta`` count and
``unsafe_penalty``.
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

from harness.evaluator import cfamily, diffscope
from harness.evaluator.langs import MetricResult, group_by_language
from harness.evaluator.quality import run_eslint

_SEVERITY_PENALTY = {"high": 0.4, "medium": 0.15, "low": 0.05}

RemainingSeconds = Callable[[], float | None]


def assess_security(
    workspace: Path, changed_files: list[str], remaining_s: RemainingSeconds | None = None
) -> MetricResult:
    """Assess security of the agent's changed files."""
    by_lang = group_by_language(changed_files, workspace)
    if not by_lang:
        return MetricResult(score=None, details={"reason": "no recognized source files changed"})

    per_lang: dict[str, Any] = {}
    scores: list[float] = []
    for lang, files in by_lang.items():
        if _budget_exhausted(remaining_s):
            result = MetricResult(score=None, details={"reason": "run budget exceeded"})
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


# eslint-plugin-security and core eval-family rules by severity: code execution
# is high, injection-prone patterns medium, weak randomness and timing low.
_JS_HIGH = frozenset(
    {
        "security/detect-eval-with-expression",
        "security/detect-child-process",
        "security/detect-disable-mustache-escape",
        "security/detect-no-csrf-before-method-override",
        "no-eval",
        "no-implied-eval",
        "no-new-func",
    }
)
_JS_LOW = frozenset(
    {
        "security/detect-possible-timing-attacks",
        "security/detect-pseudoRandomBytes",
        "security/detect-non-literal-fs-filename",
    }
)


def _js_ts(workspace: Path, files: list[str], remaining_s: RemainingSeconds | None) -> MetricResult:
    timeout = _timeout(180, remaining_s)
    if timeout is None:
        return MetricResult(
            score=None, details={"tool": "eslint-plugin-security", "reason": "run budget exceeded"}
        )
    js_files = [f for f in files if Path(f).suffix.lower() in {".js", ".mjs", ".cjs", ".jsx"}]
    if not js_files:
        return MetricResult(
            score=None,
            details={"tool": "eslint-plugin-security", "reason": "no JavaScript files changed"},
        )
    result = run_eslint(workspace, js_files, "security", timeout)
    if isinstance(result, str):
        return MetricResult(
            score=None, details={"tool": "eslint-plugin-security", "reason": result}
        )
    reports, version = result
    scope = diffscope.added_lines(workspace, js_files)
    root = workspace.resolve()
    high = medium = low = 0
    rules: set[str] = set()
    for report in reports:
        try:
            rel = str(Path(report.get("filePath", "")).resolve().relative_to(root))
        except ValueError:
            continue
        for message in report.get("messages", []):
            rule = message.get("ruleId")
            if not rule or not diffscope.is_added(scope, rel, int(message.get("line") or 0)):
                continue
            rules.add(rule)
            if rule in _JS_HIGH:
                high += 1
            elif rule in _JS_LOW:
                low += 1
            else:
                medium += 1
    details: dict[str, Any] = {
        "tool": "eslint-plugin-security",
        "tool_version": version,
        "high": high,
        "medium": medium,
        "low": low,
        "rules": sorted(rules),
        "scope": "whole files" if scope is None else "added lines",
    }
    score = score_from_counts(high, medium, low)
    if _declares_dependencies(workspace):
        audit = _npm_audit(workspace, remaining_s)
        details["npm_audit"] = audit.details
        if audit.score is not None:
            score = round(min(score, audit.score), 4)
    return MetricResult(score=score, details=details)


def _typescript(
    workspace: Path, files: list[str], remaining_s: RemainingSeconds | None
) -> MetricResult:
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


def _declares_dependencies(workspace: Path) -> bool:
    try:
        manifest = json.loads((workspace / "package.json").read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return False
    return bool(manifest.get("dependencies") or manifest.get("devDependencies"))


def _npm_audit(workspace: Path, remaining_s: RemainingSeconds | None) -> MetricResult:
    if shutil.which("npm") is None:
        return MetricResult(score=None, details={"tool": "npm audit", "reason": "npm not on PATH"})
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


# clang static analyzer groups by severity: memory errors (null dereference,
# use-after-free, double free, uninitialized use) are high, insecure APIs medium.
_C_HIGH_PREFIXES = (
    "clang-analyzer-core.",
    "clang-analyzer-cplusplus.",
    "clang-analyzer-unix.Malloc",
)
_C_MEDIUM_PREFIXES = ("clang-analyzer-security.", "bugprone-unsafe-functions")
_CPPCHECK_SEVERITY = {"error": "high", "warning": "medium", "portability": "low"}


def _c_family(lang: str) -> Callable[[Path, list[str], RemainingSeconds | None], MetricResult]:
    def scan(
        workspace: Path, files: list[str], remaining_s: RemainingSeconds | None
    ) -> MetricResult:
        timeout = _timeout(300, remaining_s)
        if timeout is None:
            return MetricResult(
                score=None,
                details={"tool": "clang-analyzer+cppcheck", "reason": "run budget exceeded"},
            )
        tidy = cfamily.run_clang_tidy(workspace, files, lang, cfamily.SECURITY_CHECKS, timeout)
        if tidy is None:
            return MetricResult(
                score=None,
                details={
                    "tool": "clang-analyzer+cppcheck",
                    "reason": "clang-tidy not found or timed out",
                },
            )
        scope = diffscope.added_lines(workspace, files)
        counts = {"high": 0, "medium": 0, "low": 0}
        checks: set[str] = set()
        for finding in tidy.findings:
            if not diffscope.is_added(scope, finding.file, finding.line):
                continue
            checks.add(finding.check)
            if finding.check.startswith(_C_HIGH_PREFIXES):
                counts["high"] += 1
            elif finding.check.startswith(_C_MEDIUM_PREFIXES):
                counts["medium"] += 1
            else:
                counts["low"] += 1
        cppcheck_version = None
        cppcheck = cfamily.run_cppcheck(workspace, files, lang, timeout)
        if cppcheck is not None:
            found, cppcheck_version = cppcheck
            for file, line, severity, check_id in found:
                if not diffscope.is_added(scope, file, line):
                    continue
                bucket = _CPPCHECK_SEVERITY.get(severity)
                if bucket:
                    counts[bucket] += 1
                    checks.add(f"cppcheck:{check_id}")
        return MetricResult(
            score=score_from_counts(counts["high"], counts["medium"], counts["low"]),
            details={
                "tool": "clang-analyzer+cppcheck",
                "tool_version": {"clang-tidy": tidy.version, "cppcheck": cppcheck_version},
                **counts,
                "checks": sorted(checks),
                "compile_errors": tidy.compile_errors,
                "scope": "whole files" if scope is None else "added lines",
            },
        )

    return scan


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


def _rust(workspace: Path, files: list[str], remaining_s: RemainingSeconds | None) -> MetricResult:  # noqa: PLR0911
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
    if not _has_external_crates(workspace / "Cargo.lock"):
        # Nothing to audit, and cargo audit would fetch the advisory database
        # over the network just to report zero.
        unsafe_delta = _count_unsafe_delta(workspace, files)
        unsafe_penalty = round(min(1.0, 0.05 * unsafe_delta), 4)
        return MetricResult(
            score=round(max(0.0, 1.0 - unsafe_penalty), 4),
            details={
                "tool": "cargo audit",
                "audit": "skipped: no external dependencies",
                "vulnerabilities": 0,
                "unsafe_delta": unsafe_delta,
                "unsafe_penalty": unsafe_penalty,
            },
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

    # Unsafe delta: count "unsafe" keywords in changed .rs files.
    unsafe_delta = _count_unsafe_delta(workspace, files)
    unsafe_penalty = round(min(1.0, 0.05 * unsafe_delta), 4)
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
            "unsafe_delta": unsafe_delta,
            "unsafe_penalty": unsafe_penalty,
        },
    )


_UNSAFE_RE = re.compile(r"\bunsafe\b")


def _has_external_crates(lockfile: Path) -> bool:
    """True when Cargo.lock lists a package fetched from a registry or git."""
    try:
        return "\nsource = " in lockfile.read_text(encoding="utf-8")
    except OSError:
        return True


def _count_unsafe_delta(workspace: Path, files: list[str]) -> int:
    """Count ``unsafe`` keywords on added, non-comment lines of the changed Rust files.

    Outside a git workspace the scope is unknown and every non-comment line of
    the changed files counts.
    """
    rust_files = [f for f in files if f.endswith(".rs")]
    scope = diffscope.added_lines(workspace, rust_files)
    count = 0
    for f in rust_files:
        try:
            lines = (workspace / f).read_text(encoding="utf-8", errors="replace").splitlines()
        except OSError:
            continue
        for number, line in enumerate(lines, start=1):
            code = line.split("//", 1)[0]
            if diffscope.is_added(scope, f, number):
                count += len(_UNSAFE_RE.findall(code))
    return count


def _unsupported(
    workspace: Path, files: list[str], remaining_s: RemainingSeconds | None
) -> MetricResult:
    del workspace, files, remaining_s
    return MetricResult(score=None, details={"reason": "no security scanner for this language"})


# Per-language scanner registry.
_SCANNERS = {
    "python": _python,
    # TypeScript keeps npm audit only (the pinned ESLint config has no TS parser).
    "typescript": _typescript,
    "javascript": _js_ts,
    "go": _go,
    "java": _java,
    "rust": _rust,
    "c": _c_family("c"),
    "cpp": _c_family("cpp"),
}
