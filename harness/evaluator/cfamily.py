"""C and C++ static analysis shared by the quality and security metrics.

Both metrics run clang-tidy (Homebrew LLVM, pinned by scripts/install_analyzers.sh)
with a fixed check set, and security adds cppcheck. Findings are kept only when
they point into one of the agent's changed files, the same scope as ruff and
bandit on Python. Compilation needs no build system: every workspace directory
that holds a header becomes an include path, with ``-std=c11`` or ``-std=c++17``
and, on macOS, the SDK sysroot. A translation unit that does not compile under
those flags reports its ``clang-diagnostic-error`` lines as ``compile_errors``;
they are recorded, not scored as findings.
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
import sys
from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path

CLANG_TIDY_FALLBACK = Path("/opt/homebrew/opt/llvm/bin/clang-tidy")

STD_FLAGS = {"c": "-std=c11", "cpp": "-std=c++17"}
HEADER_SUFFIXES = frozenset({".h", ".hh", ".hpp", ".hxx"})
_SKIP_DIRS = frozenset({".git", "node_modules", "target", "build", "zz_hidden_build"})

# Maintainability and likely-bug checks. Checks that fired repeatedly on
# sanitizer-clean reference code in the Routine v2 probe (2026-09-26) are off:
# swappable parameters, narrowing, implicit widening, throwing static
# initialisers, optional access the flow analysis cannot follow, non-std
# exception base classes and enum storage size.
QUALITY_CHECKS = ",".join(
    [
        "-*",
        "bugprone-*",
        "-bugprone-easily-swappable-parameters",
        "-bugprone-narrowing-conversions",
        "-bugprone-implicit-widening-of-multiplication-result",
        "-bugprone-assignment-in-if-condition",
        "-bugprone-throwing-static-initialization",
        "-bugprone-unchecked-optional-access",
        "-bugprone-std-exception-baseclass",
        "readability-function-cognitive-complexity",
        "readability-misleading-indentation",
        "readability-non-const-parameter",
        "readability-redundant-control-flow",
        "readability-redundant-declaration",
        "readability-else-after-return",
        "readability-inconsistent-declaration-parameter-name",
        "misc-redundant-expression",
        "misc-unused-parameters",
        "performance-*",
        "-performance-enum-size",
    ]
)
QUALITY_CHECKS_CPP_EXTRA = "modernize-use-nullptr,modernize-use-override"
QUALITY_CONFIG = "{CheckOptions: {readability-function-cognitive-complexity.Threshold: 25}}"

# Memory-safety and insecure-API checks from the clang static analyzer. The
# Annex K "DeprecatedOrUnsafeBufferHandling" check flags every memcpy and
# snprintf and is off; strcpy, which the analyzer leaves off by default, is on.
SECURITY_CHECKS = ",".join(
    [
        "-*",
        "clang-analyzer-core.*",
        "clang-analyzer-cplusplus.*",
        "clang-analyzer-unix.*",
        "clang-analyzer-security.*",
        "-clang-analyzer-security.insecureAPI.DeprecatedOrUnsafeBufferHandling",
        "clang-analyzer-security.insecureAPI.strcpy",
        "bugprone-unsafe-functions",
    ]
)

_DIAG_RE = re.compile(
    r"^(?P<file>.+?):(?P<line>\d+):(?P<col>\d+): (?P<level>warning|error): .*\[(?P<check>[^\]]+)\]$"
)


@dataclass(frozen=True)
class Finding:
    file: str
    line: int
    check: str


@dataclass
class TidyResult:
    findings: list[Finding]
    compile_errors: int
    version: str | None


def clang_tidy_path() -> Path | None:
    """clang-tidy from VULCANBENCH_CLANG_TIDY, PATH, or the Homebrew LLVM keg."""
    env = os.environ.get("VULCANBENCH_CLANG_TIDY")
    if env:
        return Path(env) if Path(env).exists() else None
    found = shutil.which("clang-tidy")
    if found:
        return Path(found)
    return CLANG_TIDY_FALLBACK if CLANG_TIDY_FALLBACK.exists() else None


@lru_cache(maxsize=4)
def tool_version(tool: str) -> str | None:
    """First version-looking token of ``<tool> --version``, for the run record."""
    try:
        out = subprocess.run(
            [tool, "--version"], capture_output=True, text=True, timeout=30, check=False
        ).stdout
    except (OSError, subprocess.TimeoutExpired):
        return None
    match = re.search(r"\d+\.\d+(?:\.\d+)?", out)
    return match.group(0) if match else None


@lru_cache(maxsize=1)
def _sysroot_args() -> tuple[str, ...]:
    if sys.platform != "darwin" or shutil.which("xcrun") is None:
        return ()
    try:
        sdk = subprocess.run(
            ["xcrun", "--show-sdk-path"], capture_output=True, text=True, timeout=30, check=False
        ).stdout.strip()
    except (OSError, subprocess.TimeoutExpired):
        return ()
    return ("-isysroot", sdk) if sdk else ()


def include_dirs(workspace: Path) -> list[str]:
    """Header directories and their ancestors up to the workspace, as ``-I`` arguments.

    Ancestors matter because projects include ``<project/header.h>`` from an
    ``include/`` root.
    """
    root = workspace.resolve()
    dirs: set[Path] = {root}
    for path in root.rglob("*"):
        rel = path.relative_to(root)
        if _SKIP_DIRS.intersection(rel.parts) or path.suffix.lower() not in HEADER_SUFFIXES:
            continue
        parent = path.parent
        while parent != root and parent not in dirs:
            dirs.add(parent)
            parent = parent.parent
    return [f"-I{d}" for d in sorted(dirs)]


def compile_args(workspace: Path, lang: str) -> list[str]:
    return [STD_FLAGS[lang], *_sysroot_args(), *include_dirs(workspace)]


def run_clang_tidy(
    workspace: Path,
    files: list[str],
    lang: str,
    checks: str,
    timeout: int,
    config: str | None = None,
) -> TidyResult | None:
    """Run clang-tidy over the changed files; ``None`` if it is unavailable or times out."""
    tidy = clang_tidy_path()
    if tidy is None:
        return None
    abs_files = [str((workspace / f).resolve()) for f in files if (workspace / f).exists()]
    if not abs_files:
        return TidyResult(findings=[], compile_errors=0, version=tool_version(str(tidy)))
    changed = set(abs_files)
    header_filter = "|".join(re.escape(f) for f in abs_files)
    cmd = [str(tidy), "--quiet", f"-checks={checks}", f"-header-filter=^({header_filter})$"]
    if config:
        cmd.append(f"-config={config}")
    if lang == "cpp":
        # Headers and sources alike parse as C++ in a C++ change.
        cmd.append("--extra-arg-before=-xc++")
    cmd += [*abs_files, "--", *compile_args(workspace, lang)]
    try:
        proc = subprocess.run(
            cmd, cwd=workspace, capture_output=True, text=True, timeout=timeout, check=False
        )
    except subprocess.TimeoutExpired:
        return None
    findings: set[Finding] = set()
    compile_errors = 0
    for raw in proc.stdout.splitlines():
        match = _DIAG_RE.match(raw.strip())
        if not match:
            continue
        check = match.group("check")
        if check.startswith("clang-diagnostic-"):
            if match.group("level") == "error":
                compile_errors += 1
            continue
        path = str(Path(match.group("file")).resolve())
        if path in changed:
            rel = str(Path(path).relative_to(workspace.resolve()))
            for single in check.split(","):
                findings.add(Finding(rel, int(match.group("line")), single))
    return TidyResult(
        findings=sorted(findings, key=lambda f: (f.file, f.line, f.check)),
        compile_errors=compile_errors,
        version=tool_version(str(tidy)),
    )


# cppcheck ids that describe the analysis itself, not the code.
_CPPCHECK_META_IDS = frozenset(
    {
        "missingInclude",
        "missingIncludeSystem",
        "unmatchedSuppression",
        "checkersReport",
        "toomanyconfigs",
        "normalCheckLevelMaxBranches",
        "syntaxError",
        "internalAstError",
    }
)


def run_cppcheck(
    workspace: Path, files: list[str], lang: str, timeout: int
) -> tuple[list[tuple[str, int, str, str]], str | None] | None:
    """cppcheck findings in the changed files as ``(file, line, severity, id)``."""
    if shutil.which("cppcheck") is None:
        return None
    sources = [
        str((workspace / f).resolve())
        for f in files
        if (workspace / f).exists() and Path(f).suffix.lower() not in HEADER_SUFFIXES
    ]
    changed = {str((workspace / f).resolve()) for f in files if (workspace / f).exists()}
    version = tool_version("cppcheck")
    if not sources:
        # Headers only: analyze every source that includes them is out of scope;
        # a header-only change is reported as scanned with no findings.
        return [], version
    cmd = [
        "cppcheck",
        "--quiet",
        "--enable=warning,portability",
        f"--language={'c++' if lang == 'cpp' else 'c'}",
        f"--std={STD_FLAGS[lang].removeprefix('-std=')}",
        "--template={file}\t{line}\t{severity}\t{id}",
        *include_dirs(workspace),
        *sources,
    ]
    try:
        proc = subprocess.run(
            cmd, cwd=workspace, capture_output=True, text=True, timeout=timeout, check=False
        )
    except subprocess.TimeoutExpired:
        return None
    seen: set[tuple[str, int, str, str]] = set()
    for raw in proc.stderr.splitlines():
        parts = raw.split("\t")
        if len(parts) != 4 or parts[3] in _CPPCHECK_META_IDS:
            continue
        path = str(Path(parts[0]).resolve()) if parts[0] else ""
        if path not in changed:
            continue
        rel = str(Path(path).relative_to(workspace.resolve()))
        seen.add((rel, int(parts[1] or 0), parts[2], parts[3]))
    return sorted(seen), version
