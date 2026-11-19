from __future__ import annotations

from pathlib import Path

import pytest

_DOCS = Path(__file__).parents[2] / "docs"
_LEARN_PAGES = sorted((_DOCS / "learn").rglob("*.md"))


def _page_url(page: Path) -> str:
    relative = page.relative_to(_DOCS).with_suffix("")
    return f"/{relative.parent}/" if relative.name == "index" else f"/{relative}"


@pytest.mark.parametrize("page", _LEARN_PAGES, ids=lambda page: page.stem)
def test_code_is_always_visible(page: Path) -> None:
    """Learn snippets are never collapsed behind a disclosure widget."""
    content = page.read_text()
    assert "<details>" not in content
    assert "<summary>" not in content


@pytest.mark.parametrize("page", _LEARN_PAGES, ids=lambda page: page.stem)
def test_page_is_reachable_from_the_sidebar(page: Path) -> None:
    """Every learn page has a sidebar entry, so none is orphaned."""
    config = (_DOCS / ".vitepress" / "config.mts").read_text()
    assert f"'{_page_url(page)}'" in config
