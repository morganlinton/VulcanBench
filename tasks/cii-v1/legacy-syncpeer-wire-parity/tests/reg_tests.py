"""Regression guards: everywhere the written spec IS accurate, behavior
must not change."""

from conftest import assert_family


def test_basic_kv():
    assert_family("guard_basic_kv")


def test_handshake_errors():
    assert_family("guard_handshake_errors")


def test_fmt_errors():
    assert_family("guard_fmt_errors")


def test_persistence_across_connections():
    assert_family("guard_persistence_across_connections")
