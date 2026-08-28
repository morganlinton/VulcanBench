"""Fail-to-pass: parity with the legacy engine on everything the written
spec gets wrong or omits."""

from conftest import assert_family


def test_winter_night_tier1():
    assert_family("f2p_winter_night_tier1")


def test_meter_uplift():
    assert_family("f2p_meter_uplift")


def test_summer_tier2():
    assert_family("f2p_summer_tier2")


def test_rounding_regime():
    assert_family("f2p_rounding")


def test_account_casefold():
    assert_family("f2p_casefold")


def test_month_wrap():
    assert_family("f2p_month_wrap")


def test_zero_kwh_skip():
    assert_family("f2p_zero_kwh")


def test_lenient_tokens():
    assert_family("f2p_lenient_tokens")


def test_full_corpus_parity():
    assert_family("f2p_parity")
