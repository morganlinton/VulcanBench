"""Fail-to-pass: parity with the legacy engine on everything the written
spec gets wrong or omits."""

from conftest import assert_family


def test_slot_reuse_ordering():
    assert_family("f2p_slot_reuse")


def test_find_case_rule():
    assert_family("f2p_find_case")


def test_wildcard_semantics():
    assert_family("f2p_wildcard")


def test_range_bounds():
    assert_family("f2p_range_bounds")


def test_duplicate_insert_update():
    assert_family("f2p_dup_update")


def test_name_truncation():
    assert_family("f2p_name_truncation")


def test_avg_truncation():
    assert_family("f2p_avg_truncation")


def test_sum_wrap():
    assert_family("f2p_sum_wrap")


def test_full_corpus_parity():
    assert_family("f2p_parity")
