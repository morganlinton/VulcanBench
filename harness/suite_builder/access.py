"""The suite builder's password gate (a placeholder for subscriber accounts).

The builder is meant for paying subscribers. Until accounts exist, a shared
password stands in: ``[builder].password_hash`` in ``vulcanbench.toml`` (or
``VULCANBENCH_BUILDER_PASSWORD_HASH``) holds an scrypt hash of it, and
``vulcanbench suite unlock`` checks the password once per machine and writes an
unlock record under ``~/.config/vulcanbench``. Every command that creates or
changes a suite calls :func:`require_unlocked`; reading suites and running a
frozen one stay open.

What this is not: protection against someone who edits the code. VulcanBench
is public and runs on the operator's machine, so the gate only keeps the
builder behind a deliberate step. Real subscriber gating needs a server-side
license check (docs/DECISIONS.md, 2026-11-03).

The password itself is never stored, logged, printed or accepted on a command
line: it comes from a terminal prompt or one line on stdin. The unlock record
holds a fingerprint of the configured hash, not the password, so changing the
password locks every machine again.
"""

from __future__ import annotations

import base64
import hashlib
import hmac
import json
import os
import secrets
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from harness.settings import builder_settings

HASH_ENV = "VULCANBENCH_BUILDER_PASSWORD_HASH"
UNLOCK_FILE_ENV = "VULCANBENCH_BUILDER_UNLOCK_FILE"
UNLOCK_SCHEMA = "vulcanbench.suite.builder-unlock.v1"
MIN_PASSWORD_LEN = 12

# scrypt parameters: about 30 ms and 32 MiB per check on a laptop.
_N = 2**15
_R = 8
_P = 1
_DKLEN = 32
_MAXMEM = 64 * 1024 * 1024


class BuilderLocked(Exception):
    """The builder is locked: no password configured, never unlocked, or the password changed."""

    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code
        self.message = message


def _b64(raw: bytes) -> str:
    return base64.urlsafe_b64encode(raw).decode("ascii").rstrip("=")


def _unb64(text: str) -> bytes:
    return base64.urlsafe_b64decode(text + "=" * (-len(text) % 4))


def _derive(password: str, salt: bytes, n: int, r: int, p: int) -> bytes:
    return hashlib.scrypt(
        password.encode("utf-8"), salt=salt, n=n, r=r, p=p, maxmem=_MAXMEM, dklen=_DKLEN
    )


def hash_password(password: str) -> str:
    """``scrypt$<n>$<r>$<p>$<salt>$<hash>`` for ``password`` (url-safe base64, no padding)."""
    if len(password) < MIN_PASSWORD_LEN:
        raise ValueError(f"the password must be at least {MIN_PASSWORD_LEN} characters")
    salt = secrets.token_bytes(16)
    digest = _derive(password, salt, _N, _R, _P)
    return f"scrypt${_N}${_R}${_P}${_b64(salt)}${_b64(digest)}"


def _parse(stored: str) -> tuple[int, int, int, bytes, bytes]:
    parts = stored.strip().split("$")
    if len(parts) != 6 or parts[0] != "scrypt":
        raise ValueError("the configured password hash is not an scrypt hash from password-hash")
    try:
        n, r, p = int(parts[1]), int(parts[2]), int(parts[3])
        salt, digest = _unb64(parts[4]), _unb64(parts[5])
    except ValueError as exc:
        raise ValueError("the configured password hash is malformed") from exc
    if n < 2**14 or n > 2**20 or r < 1 or r > 32 or p < 1 or p > 4 or len(digest) != _DKLEN:
        raise ValueError("the configured password hash has unsupported parameters")
    return n, r, p, salt, digest


def verify_password(password: str, stored: str) -> bool:
    """Constant-time check of ``password`` against a hash from :func:`hash_password`."""
    n, r, p, salt, digest = _parse(stored)
    return hmac.compare_digest(_derive(password, salt, n, r, p), digest)


def configured_hash() -> str:
    """The password hash in force: the env var, else ``[builder].password_hash``; '' when unset."""
    return (os.environ.get(HASH_ENV) or str(builder_settings()["password_hash"])).strip()


def fingerprint(stored: str) -> str:
    """A short, non-reversible id of a configured hash (what the unlock record keeps)."""
    return hashlib.sha256(stored.strip().encode("utf-8")).hexdigest()[:16]


def unlock_file() -> Path:
    raw = os.environ.get(UNLOCK_FILE_ENV)
    if raw:
        return Path(raw).expanduser()
    return Path.home() / ".config" / "vulcanbench" / "suite-builder-unlock.json"


def _read_unlock() -> dict[str, Any] | None:
    try:
        data = json.loads(unlock_file().read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    return data if isinstance(data, dict) else None


def status() -> dict[str, Any]:
    """``{configured, unlocked, unlocked_at, unlock_file}``; never raises."""
    stored = configured_hash()
    record = _read_unlock()
    unlocked = bool(stored) and record is not None and record.get("hash_id") == fingerprint(stored)
    return {
        "schema": "vulcanbench.suite.access.v1",
        "configured": bool(stored),
        "unlocked": unlocked,
        "unlocked_at": record.get("unlocked_at") if unlocked and record else None,
        "unlock_file": str(unlock_file()),
    }


def require_unlocked() -> None:
    """Raise :class:`BuilderLocked` unless this machine is unlocked for the configured password."""
    stored = configured_hash()
    if not stored:
        raise BuilderLocked(
            "builder_not_configured",
            "the suite builder is for subscribers and no password is set "
            "([builder].password_hash in vulcanbench.toml)",
        )
    record = _read_unlock()
    if record is None:
        raise BuilderLocked(
            "builder_locked",
            "the suite builder is for subscribers; unlock it with `vulcanbench suite unlock`",
        )
    if record.get("hash_id") != fingerprint(stored):
        raise BuilderLocked(
            "builder_locked",
            "the suite builder password changed; unlock it again with `vulcanbench suite unlock`",
        )


def unlock(password: str) -> dict[str, Any]:
    """Check ``password`` and write the unlock record. Raises :class:`BuilderLocked` when wrong."""
    stored = configured_hash()
    if not stored:
        raise BuilderLocked(
            "builder_not_configured",
            "no suite builder password is set ([builder].password_hash in vulcanbench.toml)",
        )
    if not verify_password(password, stored):
        raise BuilderLocked("wrong_password", "that is not the suite builder password")
    path = unlock_file()
    path.parent.mkdir(parents=True, exist_ok=True)
    record = {
        "schema": UNLOCK_SCHEMA,
        "hash_id": fingerprint(stored),
        "unlocked_at": datetime.now(UTC).isoformat(timespec="seconds"),
    }
    tmp = path.with_suffix(".tmp")
    tmp.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    tmp.chmod(0o600)
    tmp.replace(path)
    return status()


def lock() -> dict[str, Any]:
    """Remove this machine's unlock record."""
    unlock_file().unlink(missing_ok=True)
    return status()
