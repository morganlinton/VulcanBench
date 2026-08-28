"""Regression guards: everywhere the written spec IS accurate, behavior
must not change."""

from conftest import assert_family


def test_crud():
    assert_family("guard_crud")


def test_find_exact_case():
    assert_family("guard_find_exact_case")


def test_range_interior():
    assert_family("guard_range_interior")


def test_fmt_errors():
    assert_family("guard_fmt_errors")
