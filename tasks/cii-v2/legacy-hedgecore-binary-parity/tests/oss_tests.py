"""Fail-to-pass: parity with the legacy engine on everything the written
spec gets wrong or omits, across trades, revaluations, and their
composite net values."""

from conftest import assert_family


def test_reval_cent():
    assert_family("f2p_reval_cent")


def test_washout():
    assert_family("f2p_washout")


def test_jpy_fixing():
    assert_family("f2p_jpy_fixing")


def test_flat_drop():
    assert_family("f2p_flat_drop")


def test_amount_zero():
    assert_family("f2p_amount_zero")


def test_book_case():
    assert_family("f2p_book_case")


def test_lenient_tokens():
    assert_family("f2p_lenient_tokens")


def test_full_corpus_parity():
    assert_family("f2p_parity")
