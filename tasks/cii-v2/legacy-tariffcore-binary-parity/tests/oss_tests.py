"""Fail-to-pass: parity with the legacy engine on everything the written
spec gets wrong or omits."""

from conftest import assert_family


def test_duty_rounding():
    assert_family("f2p_duty_rounding")


def test_weight_ceiling():
    assert_family("f2p_weight_ceiling")


def test_dense_air_band():
    assert_family("f2p_dense_air_band")


def test_origin_levy():
    assert_family("f2p_origin_levy")


def test_audit_mode():
    assert_family("f2p_audit_mode")


def test_jpy_sea_waiver():
    assert_family("f2p_jpy_sea_waiver")


def test_cap_scope():
    assert_family("f2p_cap_scope")


def test_chapter98():
    assert_family("f2p_chapter98")


def test_full_corpus_parity():
    assert_family("f2p_parity")
