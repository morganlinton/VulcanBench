"""Fail-to-pass: parity with the legacy engine on everything the written
spec gets wrong or omits."""

from conftest import assert_family


def test_display_stock():
    assert_family("f2p_display_stock")


def test_cost_merge():
    assert_family("f2p_cost_merge")


def test_shrinkage():
    assert_family("f2p_shrinkage")


def test_cost_cap():
    assert_family("f2p_cost_cap")


def test_partial_fill():
    assert_family("f2p_partial_fill")


def test_case_fold():
    assert_family("f2p_case_fold")


def test_zero_qty():
    assert_family("f2p_zero_qty")


def test_lenient_tokens():
    assert_family("f2p_lenient_tokens")


def test_full_corpus_parity():
    assert_family("f2p_parity")
