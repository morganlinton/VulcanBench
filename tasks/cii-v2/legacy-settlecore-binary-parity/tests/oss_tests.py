"""Fail-to-pass: parity with the legacy engine on everything the written
spec gets wrong or omits."""

from conftest import assert_family


def test_rounding_regimes():
    assert_family("f2p_rounding")


def test_tier_rate_table():
    assert_family("f2p_tier_rates")


def test_weekend_mcc_surcharge():
    assert_family("f2p_surcharge")


def test_eur_levy_threshold():
    assert_family("f2p_eur_levy")


def test_fee_cap_exception():
    assert_family("f2p_fee_cap")


def test_lenient_amount_padding():
    assert_family("f2p_lenient_amount")


def test_negative_refunds():
    assert_family("f2p_negative_refund")


def test_leap_year_rule():
    assert_family("f2p_leap_window")


def test_batch_ordering():
    assert_family("f2p_batch_ordering")


def test_full_corpus_parity():
    assert_family("f2p_parity")
