"""Tests for _untrack_vendor_in_gitignore in slice_repo.py.

Regression guard for the VulcanCyber Go-task defect: slices inherited an
upstream .gitignore with a ``vendor`` rule, so ``go mod vendor`` output was
silently never committed and a clean clone could not build offline.
"""

from __future__ import annotations

from pathlib import Path

import pytest

from scripts.slice_repo import _untrack_vendor_in_gitignore


def _write_gitignore(root: Path, lines: list[str]) -> Path:
    gi = root / ".gitignore"
    gi.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return gi


@pytest.mark.parametrize(
    "rule", ["vendor", "/vendor", "vendor/", "/vendor/", "**/vendor", "**/vendor/", "  vendor  "]
)
def test_strips_vendor_rule_variants(tmp_path: Path, rule: str) -> None:
    gi = _write_gitignore(tmp_path, ["*.log", rule, "dist/"])
    assert _untrack_vendor_in_gitignore(tmp_path) is True
    assert gi.read_text(encoding="utf-8") == "*.log\ndist/\n"


def test_preserves_comments_and_unrelated_rules(tmp_path: Path) -> None:
    lines = [
        "# vendor is ignored upstream",
        "vendor",
        "vendored_docs/",
        "!vendor.keep",
        "node_modules/",
    ]
    gi = _write_gitignore(tmp_path, lines)
    assert _untrack_vendor_in_gitignore(tmp_path) is True
    assert gi.read_text(encoding="utf-8").splitlines() == [
        "# vendor is ignored upstream",
        "vendored_docs/",
        "!vendor.keep",
        "node_modules/",
    ]


def test_no_vendor_rule_leaves_file_untouched(tmp_path: Path) -> None:
    gi = _write_gitignore(tmp_path, ["*.log", "dist/"])
    before = gi.read_text(encoding="utf-8")
    assert _untrack_vendor_in_gitignore(tmp_path) is False
    assert gi.read_text(encoding="utf-8") == before


def test_missing_gitignore_returns_false(tmp_path: Path) -> None:
    assert _untrack_vendor_in_gitignore(tmp_path) is False


def test_nested_gitignore_untouched(tmp_path: Path) -> None:
    _write_gitignore(tmp_path, ["vendor"])
    nested = tmp_path / "vendor" / "github.com" / "dep"
    nested.mkdir(parents=True)
    nested_gi = _write_gitignore(nested, ["vendor"])
    assert _untrack_vendor_in_gitignore(tmp_path) is True
    assert nested_gi.read_text(encoding="utf-8") == "vendor\n"


def test_vendor_only_gitignore_becomes_empty(tmp_path: Path) -> None:
    gi = _write_gitignore(tmp_path, ["vendor"])
    assert _untrack_vendor_in_gitignore(tmp_path) is True
    assert gi.read_text(encoding="utf-8") == ""
