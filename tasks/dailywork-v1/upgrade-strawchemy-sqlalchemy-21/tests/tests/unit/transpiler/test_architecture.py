"""Tests of the dependency rule between the transpiler core and its passes."""

from __future__ import annotations

import ast
from pathlib import Path

import pytest

import strawchemy.transpiler

TRANSPILER = Path(strawchemy.transpiler.__file__).parent
CORE = "strawchemy.transpiler._core"
PASSES = "strawchemy.transpiler._passes"


def _module_name(path: Path) -> str:
    parts = path.relative_to(TRANSPILER.parent.parent).with_suffix("").parts
    return ".".join(parts[:-1] if parts[-1] == "__init__" else parts)


def _imports(path: Path) -> set[str]:
    """Returns every module ``path`` imports, relative imports resolved, ``TYPE_CHECKING`` ones included."""
    package = _module_name(path) if path.name == "__init__.py" else _module_name(path).rpartition(".")[0]
    imported: set[str] = set()
    for node in ast.walk(ast.parse(path.read_text(encoding="utf-8"))):
        if isinstance(node, ast.Import):
            imported.update(alias.name for alias in node.names)
        elif isinstance(node, ast.ImportFrom):
            base = package.rsplit(".", node.level - 1)[0] if node.level else ""
            module = ".".join(part for part in (base, node.module) if part)
            imported.add(module)
            imported.update(f"{module}.{alias.name}" for alias in node.names)
    return imported


def _reaches(imported: set[str], package: str) -> bool:
    return any(name == package or name.startswith(f"{package}.") for name in imported)


@pytest.mark.parametrize(
    "path",
    [path for path in sorted((TRANSPILER / "_passes").glob("*.py")) if path.name != "__init__.py"],
    ids=lambda path: path.stem,
)
def test_passes_import_only_core(path: Path) -> None:
    """A pass module imports no other pass: two passes interact only through a ``Level`` service."""
    assert not _reaches(_imports(path), PASSES)


@pytest.mark.parametrize("path", sorted((TRANSPILER / "_core").glob("*.py")), ids=lambda path: path.stem)
def test_core_imports_no_pass(path: Path) -> None:
    """A core module imports no pass."""
    assert not _reaches(_imports(path), PASSES)


def test_only_transpiler_imports_both() -> None:
    """Outside the two packages, ``_transpiler.py`` is the only transpiler module importing both."""
    importers = [
        path.name
        for path in sorted(TRANSPILER.glob("*.py"))
        if _reaches(imported := _imports(path), CORE) and _reaches(imported, PASSES)
    ]

    assert importers == ["_transpiler.py"]
