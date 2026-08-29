"""Fail-to-pass: parity with the legacy engine on everything the written
spec gets wrong or omits."""

from conftest import assert_family


def test_requeue_penalty():
    assert_family("f2p_requeue_penalty")


def test_tie_aging():
    assert_family("f2p_tie_aging")


def test_sentinel():
    assert_family("f2p_sentinel")


def test_cap999():
    assert_family("f2p_cap999")


def test_undo_two_deep():
    assert_family("f2p_undo_two_deep")


def test_item_case():
    assert_family("f2p_item_case")


def test_trailing():
    assert_family("f2p_trailing")


def test_full_corpus_parity():
    assert_family("f2p_parity")
