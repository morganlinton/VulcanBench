"""Hidden test for oss-pytest-approx-public-type.

pytest must expose ``pytest.Approx`` as a public type: the common base class of
every object ``pytest.approx(...)`` can return (scalars, mappings, sequences,
Decimal, and, when available, numpy arrays). ``isinstance(pytest.approx(x),
pytest.Approx)`` must hold for each supported input shape, and the approx
comparison / rich-diff machinery must recognise objects via this public type.
Expected behavior captured from the upstream fix (pytest-dev/pytest PR #14647).

Run with PYTHONPATH=src so the workspace's src/pytest is the pytest under test.
"""
from __future__ import annotations

from decimal import Decimal

import pytest


def test_approx_is_public_attribute():
    assert hasattr(pytest, "Approx"), "pytest.Approx is not exposed"
    assert isinstance(pytest.Approx, type)
    assert "Approx" in getattr(pytest, "__all__", [])


def test_approx_return_values_are_instances():
    # Every shape approx() supports must be an instance of the public base type.
    for value in (1.0, {"a": 1.0}, [1.0, 2.0], (1.0, 2.0), Decimal("1.0")):
        inst = pytest.approx(value)
        assert isinstance(inst, pytest.Approx), f"{value!r} -> {type(inst).__name__} is not pytest.Approx"


def test_public_type_drives_comparison():
    # The public type is what the equality machinery keys on; comparisons still work.
    assert 2.0 == pytest.approx(2.0)
    assert {"a": 1.0} == pytest.approx({"a": 1.0000001})
    assert [1.0, 2.0] == pytest.approx([1.0, 2.0])


def test_repr_compare_recognises_public_type():
    # A failing approx comparison still produces a rich diff, keyed off the public type.
    inst = pytest.approx([1.0, 2.0])
    lines = inst._repr_compare([1.0, 9.0])
    assert any("Index" in line or "approx" in line.lower() for line in lines)
