"""Test helpers for the password-gated suite builder."""

from __future__ import annotations

from pathlib import Path

import pytest

from harness.suite_builder import access

#: A test-only password (never a real one); its hash is made once per session.
TEST_PASSWORD = "test-only-builder-password"
_HASH: list[str] = []


def test_hash() -> str:
    if not _HASH:
        _HASH.append(access.hash_password(TEST_PASSWORD))
    return _HASH[0]


def configure_builder(monkeypatch: pytest.MonkeyPatch, tmp_path: Path, *, unlocked: bool) -> Path:
    """Point the gate at a test hash and a tmp unlock file; unlock it when asked."""
    unlock_path = tmp_path / "builder-unlock.json"
    monkeypatch.setenv(access.HASH_ENV, test_hash())
    monkeypatch.setenv(access.UNLOCK_FILE_ENV, str(unlock_path))
    if unlocked:
        access.unlock(TEST_PASSWORD)
    return unlock_path
