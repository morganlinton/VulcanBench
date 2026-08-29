"""Fail-to-pass: parity with the legacy engine on everything the written
spec gets wrong or omits, across writes, reads, sweeps, and checksums."""

from conftest import assert_family


def test_sweep_vs_read():
    assert_family("f2p_sweep_vs_read")


def test_rewrite_dirty():
    assert_family("f2p_rewrite_dirty")


def test_downgrade():
    assert_family("f2p_downgrade")


def test_maxlen_bodies():
    assert_family("f2p_maxlen")


def test_digest_skew():
    assert_family("f2p_digest_skew")


def test_doc_case_fold():
    assert_family("f2p_doc_case_fold")


def test_lenient_tokens():
    assert_family("f2p_lenient_tokens")


def test_checksum_drift():
    assert_family("f2p_checksum_drift")


def test_full_corpus_parity():
    assert_family("f2p_parity")
