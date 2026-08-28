"""Fail-to-pass: parity with the legacy engine on everything the written
spec gets wrong or omits, in both codec directions."""

from conftest import assert_family


def test_memo_squeeze():
    assert_family("f2p_memo_squeeze")


def test_january_audit_flag():
    assert_family("f2p_january_flag")


def test_prefix_case_leniency():
    assert_family("f2p_prefix_case")


def test_acct_padding_leniency():
    assert_family("f2p_acct_padding")


def test_null_date_decode():
    assert_family("f2p_null_date")


def test_comma_memo_padding():
    assert_family("f2p_comma_padding")


def test_check_override():
    assert_family("f2p_check_override")


def test_lenient_tokens():
    assert_family("f2p_lenient_tokens")


def test_round_trips():
    assert_family("f2p_roundtrip")


def test_full_corpus_parity():
    assert_family("f2p_parity")
