"""Fail-to-pass: parity with the legacy engine on everything the written
spec gets wrong or omits."""

from conftest import assert_family


def test_retry_tie():
    assert_family("f2p_retry_tie")


def test_double_penalty():
    assert_family("f2p_double_penalty")


def test_fanin_boost():
    assert_family("f2p_fanin_boost")


def test_stale_dep():
    assert_family("f2p_stale_dep")


def test_dup_deps():
    assert_family("f2p_dup_deps")


def test_job_case():
    assert_family("f2p_job_case")


def test_trailing():
    assert_family("f2p_trailing")


def test_full_corpus_parity():
    assert_family("f2p_parity")
