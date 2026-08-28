"""Fail-to-pass: parity with the legacy engine on everything the written
spec gets wrong or omits."""

from conftest import assert_family


def test_household_surtax():
    assert_family("f2p_household_surtax")


def test_levy_cap_crossing():
    assert_family("f2p_levy_cap_crossing")


def test_married_relief_order():
    assert_family("f2p_married_relief_order")


def test_state_sd():
    assert_family("f2p_state_sd")


def test_flag_recapture():
    assert_family("f2p_flag_recapture")


def test_bonus_period():
    assert_family("f2p_bonus_period")


def test_zero_gross():
    assert_family("f2p_zero_gross")


def test_id_case_fold():
    assert_family("f2p_id_case_fold")


def test_rounding_regime():
    assert_family("f2p_rounding_regime")


def test_full_corpus_parity():
    assert_family("f2p_parity")
