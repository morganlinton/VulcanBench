from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

from tests.conftest import _EXTRA_MODULES, EXTRAS_ENV_VAR, pytest_configure

if TYPE_CHECKING:
    from pytest import MonkeyPatch


def test_unknown_extra(monkeypatch: MonkeyPatch, pytestconfig: pytest.Config) -> None:
    monkeypatch.setenv(EXTRAS_ENV_VAR, "geo,typo")

    with pytest.raises(pytest.UsageError, match="unknown extras: typo"):
        pytest_configure(pytestconfig)


def test_missing_module(monkeypatch: MonkeyPatch, pytestconfig: pytest.Config) -> None:
    monkeypatch.setitem(_EXTRA_MODULES, "geo", ("not_an_installed_module",))
    monkeypatch.setenv(EXTRAS_ENV_VAR, "geo")

    with pytest.raises(pytest.UsageError, match="not_an_installed_module are not installed"):
        pytest_configure(pytestconfig)


def test_installed_module(monkeypatch: MonkeyPatch, pytestconfig: pytest.Config) -> None:
    monkeypatch.setitem(_EXTRA_MODULES, "geo", ("os",))
    monkeypatch.setenv(EXTRAS_ENV_VAR, "geo")

    pytest_configure(pytestconfig)


def test_no_extras_required(monkeypatch: MonkeyPatch, pytestconfig: pytest.Config) -> None:
    monkeypatch.delenv(EXTRAS_ENV_VAR, raising=False)

    pytest_configure(pytestconfig)
