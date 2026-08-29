"""Fail-to-pass: parity with the legacy engine on everything the written
spec gets wrong or omits, across width changes, appends, reflows, and
digests."""

from conftest import assert_family


def test_incremental_vs_reflow():
    assert_family("f2p_incr_vs_reflow")


def test_width_rewrap():
    assert_family("f2p_width_rewrap")


def test_overlong_words():
    assert_family("f2p_overlong")


def test_digest_spaces():
    assert_family("f2p_digest_spaces")


def test_empty_reflow():
    assert_family("f2p_empty_reflow")


def test_digit_widths():
    assert_family("f2p_digit_words")


def test_lenient_tokens():
    assert_family("f2p_lenient_tokens")


def test_full_corpus_parity():
    assert_family("f2p_parity")
