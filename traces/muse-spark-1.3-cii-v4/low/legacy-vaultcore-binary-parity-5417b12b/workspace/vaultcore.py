"""VaultCore versioned document vault, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). Reads vault
commands on stdin (``W`` writes a document at a format version, ``G``
reads a document's digest, ``M`` bulk-migrates remaining v1 documents,
``C`` prints the vault checksum), writes one reply per command and an
``X`` trailer at EOF. Format reference: ``docs/SPEC.md`` (note the drift
warning at the top of that file; the legacy engine's behavior is the
contract).
"""

from __future__ import annotations

import sys

ID_MAX = 8
BODY_MAX = 20
DIGEST_MOD = 97
CHECKSUM_MOD = 1000000007
_WEIGHTS = (4, 9)


def _weight(index: int) -> int:
    """Per-byte digest weight: the first byte is ignored, then weights
    cycle 4, 9 starting with 4 at the second byte."""
    if index == 0:
        return 0
    return _WEIGHTS[(index - 1) % 2]


def digest(body: str) -> int:
    """Weighted byte sum of the stored body (first byte unweighted,
    then weights cycling 4, 9)."""
    total = sum(ord(c) * _weight(i) for i, c in enumerate(body))
    return total % DIGEST_MOD


_DIGITS = "0123456789"


def _migrate_rewritten(body: str) -> str:
    """Migrate a v1 body belonging to a document that was rewritten in
    place (v1 write over an already-v1 document), and the bulk sweep's
    migration of any v1 body: trailing digits are stripped, the rest is
    uppercased, and a body still 20 characters long is cut to 18."""
    body = body.rstrip(_DIGITS).upper()
    if len(body) == BODY_MAX:
        body = body[: BODY_MAX - 2]
    return body


def migrate(body: str) -> str:
    """Migrate a v1 body to v2 on the lazy read path (full uppercase)."""
    return body.upper()


def _valid_id(token: str) -> bool:
    return 1 <= len(token) <= ID_MAX and token.isascii() and token.isalnum()


def _valid_body(token: str) -> bool:
    return 1 <= len(token) <= BODY_MAX and token.isascii() and token.isalnum()


def _split(line: str) -> list[str]:
    """Split a command line on ASCII spaces only; empty fields from
    repeated or trailing spaces carry no token. Tabs and other
    whitespace are data, not separators."""
    return [tok for tok in line.split(" ") if tok]


def _key(doc: str) -> str:
    """Storage key for a document id (ids are case-insensitive)."""
    return doc.lower()


class Engine:
    """Document vault with lazy/bulk migration and batch counters."""

    def __init__(self) -> None:
        # normalized doc id -> (version, body, rewritten: last write was
        # a v1 write over an already-v1 document)
        self.docs: dict[str, tuple[int, str, bool]] = {}
        self.writes = 0
        self.reads = 0
        self.migrated = 0
        self.rejected = 0

    def handle(self, line: str) -> str:
        if line == "W" or line.startswith("W "):
            reply = self._write(_split(line))
            kind = "W"
        elif line == "G" or line.startswith("G "):
            reply = self._read(_split(line))
            kind = "G"
        elif line == "M" or line.startswith("M "):
            reply = self._sweep(_split(line))
            kind = "M"
        elif line == "C" or line.startswith("C "):
            reply = self._checksum(_split(line))
            kind = "C"
        else:
            reply = "R ???????? FMT"
            kind = "?"
        if reply.startswith("R "):
            self.rejected += 1
        elif kind == "W":
            self.writes += 1
        elif kind == "G":
            self.reads += 1
        return reply

    def trailer(self) -> str:
        return f"X {self.writes} {self.reads} {self.migrated} {self.rejected}"

    def _counts(self) -> tuple[int, int]:
        c1 = sum(1 for ver, _, _ in self.docs.values() if ver == 1)
        return c1, len(self.docs) - c1

    def _write(self, tokens: list[str]) -> str:
        if len(tokens) < 4:
            return "R ???????? FMT"
        doc, vtok, body = tokens[1], tokens[2], tokens[3]
        if not _valid_id(doc):
            return "R ???????? FMT"
        if vtok not in ("1", "2"):
            return f"R {doc} VER"
        if not _valid_body(body):
            return f"R {doc} BODY"
        ver = int(vtok)
        key = _key(doc)
        # The engine accepts version downgrades; a v1 write over an
        # already-v1 document only rewrites in place.
        rewritten = ver == 1 and key in self.docs and self.docs[key][0] == 1
        self.docs[key] = (ver, body, rewritten)
        c1, c2 = self._counts()
        return f"OK {c1} {c2}"

    def _read(self, tokens: list[str]) -> str:
        if len(tokens) < 2:
            return "R ???????? FMT"
        doc = tokens[1]
        if not _valid_id(doc):
            return "R ???????? FMT"
        key = _key(doc)
        if key not in self.docs:
            return "NIL"
        ver, body, rewritten = self.docs[key]
        if ver == 1:
            if rewritten:
                body = _migrate_rewritten(body)
            else:
                body = migrate(body)
            self.docs[key] = (2, body, False)
            self.migrated += 1
        return f"D {digest(body)}"

    def _sweep(self, tokens: list[str]) -> str:
        del tokens  # the engine ignores extra arguments to M
        count = 0
        for key, (ver, body, _) in self.docs.items():
            if ver != 1:
                continue
            self.docs[key] = (2, _migrate_rewritten(body), False)
            count += 1
        self.migrated += count
        return f"M {count}"

    def _checksum(self, tokens: list[str]) -> str:
        del tokens  # the engine ignores extra arguments to C
        total = sum(digest(body) for _, body, _ in self.docs.values())
        return f"C {total % CHECKSUM_MOD}"


def process(lines: list[str], out=None) -> list[str]:
    engine = Engine()
    replies = []
    for raw in lines:
        line = raw.rstrip("\r\n")
        if not line:
            continue
        replies.append(engine.handle(line))
    replies.append(engine.trailer())
    if out is not None:
        for reply in replies:
            print(reply, file=out)
    return replies


def main() -> None:
    process(sys.stdin.readlines(), out=sys.stdout)


if __name__ == "__main__":
    main()
