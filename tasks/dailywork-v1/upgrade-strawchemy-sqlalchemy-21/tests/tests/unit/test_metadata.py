from __future__ import annotations

from importlib.metadata import metadata, version

from strawchemy.__metadata__ import __project__, __version__


def test_version() -> None:
    assert version("strawchemy") == __version__


def test_project() -> None:
    assert metadata("strawchemy")["Name"] == __project__
