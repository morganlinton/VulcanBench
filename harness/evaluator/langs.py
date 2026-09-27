"""Shared evaluator value types and language detection.

This is the lowest-level evaluator module (no intra-package imports) so the
per-metric analyzers and the orchestrator can all depend on it without cycles.
"""

from __future__ import annotations

from collections import defaultdict
from pathlib import Path
from typing import Any

from pydantic import BaseModel, Field


class MetricResult(BaseModel):
    """Result of one metric assessment.

    ``score`` is on a 0.0-1.0 scale, or ``None`` when the metric could not be
    computed (e.g. no analyzer available for the changed languages). ``details``
    carries the transparent breakdown -- tools used, per-language sub-scores,
    counts, and a ``reason`` whenever something was skipped.
    """

    score: float | None = None
    details: dict[str, Any] = Field(default_factory=dict)


# File extension -> canonical language name.
EXT_TO_LANG: dict[str, str] = {
    ".py": "python",
    ".pyi": "python",
    ".ts": "typescript",
    ".tsx": "typescript",
    ".js": "javascript",
    ".jsx": "javascript",
    ".mjs": "javascript",
    ".cjs": "javascript",
    ".go": "go",
    ".java": "java",
    ".rs": "rust",
    ".c": "c",
    # ``.h`` is ambiguous; it reads as C unless the change or the workspace is C++
    # (see group_by_language).
    ".h": "c",
    ".cc": "cpp",
    ".cpp": "cpp",
    ".cxx": "cpp",
    ".hh": "cpp",
    ".hpp": "cpp",
    ".hxx": "cpp",
}

_CPP_SOURCE_SUFFIXES = frozenset({".cc", ".cpp", ".cxx"})
_SKIP_DIRS = frozenset({".git", "node_modules", "target", "build", "zz_hidden_build"})


def detect_language(path: str) -> str | None:
    """Return the canonical language for a file path, or ``None`` if unknown."""
    return EXT_TO_LANG.get(Path(path).suffix.lower())


def group_by_language(files: list[str], workspace: Path | None = None) -> dict[str, list[str]]:
    """Group file paths by detected language, dropping unrecognized extensions.

    A ``.h`` header is C++ when the same change touches C++ sources, or when the
    workspace (if given) holds C++ sources; otherwise it is C.
    """
    grouped: dict[str, list[str]] = defaultdict(list)
    headers: list[str] = []
    for f in files:
        if Path(f).suffix.lower() == ".h":
            headers.append(f)
            continue
        lang = detect_language(f)
        if lang is not None:
            grouped[lang].append(f)
    if headers:
        header_lang = "cpp" if "cpp" in grouped or _has_cpp_sources(workspace) else "c"
        grouped[header_lang].extend(headers)
    return dict(grouped)


def _has_cpp_sources(workspace: Path | None) -> bool:
    if workspace is None or not workspace.is_dir():
        return False
    for path in workspace.rglob("*"):
        if _SKIP_DIRS.intersection(path.relative_to(workspace).parts):
            continue
        if path.suffix.lower() in _CPP_SOURCE_SUFFIXES and path.is_file():
            return True
    return False
