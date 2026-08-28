"""Fail-to-pass: parity with the legacy engine on everything the written
spec gets wrong or omits."""

from conftest import assert_family


def test_retro_promotion():
    assert_family("f2p_retro_promotion")


def test_gold_threshold():
    assert_family("f2p_gold_threshold")


def test_electronics_promo():
    assert_family("f2p_electronics_promo")


def test_travel_truncation():
    assert_family("f2p_travel_truncation")


def test_sub100_skip():
    assert_family("f2p_sub100_skip")


def test_order_of_operations():
    assert_family("f2p_order_of_operations")


def test_id_case_fold():
    assert_family("f2p_id_case_fold")


def test_wildcard_promo():
    assert_family("f2p_wildcard_promo")


def test_full_corpus_parity():
    assert_family("f2p_parity")
