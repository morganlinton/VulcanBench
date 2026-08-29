"""Fail-to-pass: parity with the legacy engine on everything the written
spec gets wrong or omits, across live pacing, true-ups, and cross-day
state."""

from conftest import assert_family


def test_audit_clamp():
    assert_family("f2p_audit_clamp")


def test_rollover_debt():
    assert_family("f2p_rollover_debt")


def test_overnight_damper():
    assert_family("f2p_overnight_damper")


def test_exact_exhaustion():
    assert_family("f2p_exact_exhaustion")


def test_one_digit_hours():
    assert_family("f2p_one_digit_hours")


def test_case_fold():
    assert_family("f2p_case_fold")


def test_lenient_tokens():
    assert_family("f2p_lenient_tokens")


def test_multiday_carry():
    assert_family("f2p_multiday_carry")


def test_full_corpus_parity():
    assert_family("f2p_parity")
