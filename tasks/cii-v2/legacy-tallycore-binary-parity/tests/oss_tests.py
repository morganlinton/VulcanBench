"""Fail-to-pass: parity with the legacy engine on everything the written
spec gets wrong or omits, across the incremental leader path and the
recount path."""

from conftest import assert_family


def test_momentum_carryover():
    assert_family("f2p_momentum")


def test_longid_tiebreak():
    assert_family("f2p_longid_tiebreak")


def test_full_rank_bonus():
    assert_family("f2p_full_rank_bonus")


def test_two_cand_exhaustion():
    assert_family("f2p_two_cand_exhaustion")


def test_unknown_entry():
    assert_family("f2p_unknown_entry")


def test_case_fold():
    assert_family("f2p_case_fold")


def test_lenient_tokens():
    assert_family("f2p_lenient_tokens")


def test_full_corpus_parity():
    assert_family("f2p_parity")
