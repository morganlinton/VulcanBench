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


def _weight(i: int) -> int:
    if i == 0:
        return 0
    return 4 if i % 2 == 1 else 9


def digest(body: str) -> int:
    """Weighted byte sum of the stored body; position 0 weighs 0, then
    weights cycle 4, 9."""
    total = sum(ord(c) * _weight(i) for i, c in enumerate(body))
    return total % DIGEST_MOD


def _strip_migrated(body: str) -> str:
    body = body.rstrip("0123456789")
    if len(body) == 20:
        body = body[:18]
    return body


def migrate(body: str, strip: bool = False) -> str:
    """Migrate a v1 body to v2 by uppercasing it. The bulk sweep (and a
    lazy read of a document rewritten in place at v1) additionally
    strips trailing digits and truncates a 20-character result to 18."""
    body = body.upper()
    if strip:
        body = _strip_migrated(body)
    return body


def _valid_id(token: str) -> bool:
    return 1 <= len(token) <= ID_MAX and token.isascii() and token.isalnum()


def _valid_body(token: str) -> bool:
    return 1 <= len(token) <= BODY_MAX and token.isascii() and token.isalnum()


class Engine:
    """Document vault with lazy/bulk migration and batch counters."""

    def __init__(self) -> None:
        # norm id -> (version, body, rewritten-in-place-at-v1)
        self.docs: dict[str, tuple[int, str, bool]] = {}
        self.writes = 0
        self.reads = 0
        self.migrated = 0
        self.rejected = 0

    def handle(self, line: str) -> str:
        if line == "W" or line.startswith("W "):
            reply = self._write(line.split())
            kind = "W"
        elif line == "G" or line.startswith("G "):
            reply = self._read(line.split())
            kind = "G"
        elif line == "M" or line.startswith("M "):
            reply = self._sweep(line.split())
            kind = "M"
        elif line == "C" or line.startswith("C "):
            reply = self._checksum(line.split())
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
        key = doc.lower()
        rewritten = (
            key in self.docs
            and self.docs[key][0] == 1
            and ver == 1
        )
        self.docs[key] = (ver, body, rewritten)
        c1, c2 = self._counts()
        return f"OK {c1} {c2}"

    def _read(self, tokens: list[str]) -> str:
        if len(tokens) < 2:
            return "R ???????? FMT"
        doc = tokens[1]
        if not _valid_id(doc):
            return "R ???????? FMT"
        key = doc.lower()
        if key not in self.docs:
            return "NIL"
        ver, body, rewritten = self.docs[key]
        if ver == 1:
            body = migrate(body, strip=rewritten)
            self.docs[key] = (2, body, False)
            self.migrated += 1
        return f"D {digest(body)}"

    def _sweep(self, tokens: list[str]) -> str:
        count = 0
        for doc, (ver, body, _) in self.docs.items():
            if ver != 1:
                continue
            self.docs[doc] = (2, migrate(body, strip=True), False)
            count += 1
        self.migrated += count
        return f"M {count}"

    def _checksum(self, tokens: list[str]) -> str:
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
