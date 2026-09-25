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


def digest(body: str) -> int:
    """Weighted byte sum of the stored body, skipping the first byte,
    weights cycling 4, 9 from the second byte on."""
    total = sum(ord(c) * _WEIGHTS[i % 2] for i, c in enumerate(body[1:]))
    return total % DIGEST_MOD


def migrate(body: str) -> str:
    """Migrate a v1 body to v2 on the lazy read path (plain uppercase)."""
    return body.upper()


def migrate_bulk(body: str) -> str:
    """Migrate a v1 body to v2 on the bulk sweep path: uppercase, strip
    trailing digits, then drop to the first 18 characters when the
    stripped body is still 20 characters long."""
    body = body.upper()
    while body and body[-1] in "0123456789":
        body = body[:-1]
    if len(body) == 20:
        body = body[:18]
    return body


def _valid_id(token: str) -> bool:
    return 1 <= len(token) <= ID_MAX and token.isascii() and token.isalnum()


def _valid_body(token: str) -> bool:
    return 1 <= len(token) <= BODY_MAX and token.isascii() and token.isalnum()


def _split(line: str) -> list[str]:
    """Split a command line on ASCII spaces (other whitespace stays
    inside tokens), collapsing repeats like the engine does."""
    return [token for token in line.split(" ") if token != ""]


class Engine:
    """Document vault with lazy/bulk migration and batch counters."""

    def __init__(self) -> None:
        # normed doc id -> [version, body, rewritten-at-v1 flag]
        self.docs: dict[str, list] = {}
        self.writes = 0
        self.reads = 0
        self.migrated = 0
        self.rejected = 0

    def handle(self, line: str) -> str:
        # The engine tokenizes on ASCII spaces only (tabs and other
        # whitespace stay inside tokens); extra tokens are ignored.
        if line == "W" or line.startswith("W "):
            reply = self._write(_split(line))
            kind = "W"
        elif line == "G" or line.startswith("G "):
            reply = self._read(_split(line))
            kind = "G"
        elif line == "M" or line.startswith("M "):
            reply = self._sweep()
            kind = "M"
        elif line == "C" or line.startswith("C "):
            reply = self._checksum()
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
        # The engine accepts version downgrades; a v1 write over a v1
        # document additionally marks it so the lazy read path applies
        # the bulk migration transform to it later.
        key = doc.lower()
        rewritten = key in self.docs and self.docs[key][0] == 1 and ver == 1
        self.docs[key] = [ver, body, rewritten]
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
            body = migrate_bulk(body) if rewritten else migrate(body)
            self.docs[key] = [2, body, False]
            self.migrated += 1
        return f"D {digest(body)}"

    def _sweep(self) -> str:
        count = 0
        for key, (ver, body, _) in self.docs.items():
            if ver != 1:
                continue
            self.docs[key] = [2, migrate_bulk(body), False]
            count += 1
        self.migrated += count
        return f"M {count}"

    def _checksum(self) -> str:
        total = sum(digest(body) for _, body, _ in self.docs.values())
        return f"C {total % CHECKSUM_MOD}"


def _clean(raw: str) -> str:
    """Reduce one raw input line to the command text: drop the trailing
    newline, then drop anything from the first carriage return on."""
    return raw.split("\n", 1)[0].split("\r", 1)[0]


def process(lines: list[str], out=None) -> list[str]:
    engine = Engine()
    replies = []
    for raw in lines:
        line = _clean(raw)
        if not line:
            continue
        replies.append(engine.handle(line))
    replies.append(engine.trailer())
    if out is not None:
        for reply in replies:
            print(reply, file=out)
    return replies


def main() -> None:
    # Read stdin as bytes so carriage returns reach us intact (text-mode
    # universal newlines would otherwise turn a mid-line CR into a line
    # break the engine never sees).
    data = sys.stdin.buffer.read().split(b"\n")
    lines = [chunk.split(b"\r", 1)[0].decode("latin-1") for chunk in data]
    process(lines, out=sys.stdout)


if __name__ == "__main__":
    main()
