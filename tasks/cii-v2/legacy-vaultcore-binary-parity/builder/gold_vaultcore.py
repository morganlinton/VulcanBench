"""VaultCore versioned document vault, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). Reads vault
commands on stdin (``W`` writes a document at a format version, ``G``
reads a document's digest, ``M`` bulk-migrates remaining v1 documents,
``C`` prints the vault checksum), writes one reply per command and an
``X`` trailer at EOF. Format reference: ``docs/SPEC.md``, corrected
against the engine's actual behavior wherever the two disagree (the
engine is the contract); every deviation from the written spec is marked
``engine:`` below.
"""

from __future__ import annotations

import sys

ID_MAX = 8
BODY_MAX = 20
DIGEST_MOD = 97
CHECKSUM_MOD = 1000000007
_WEIGHTS = (4, 9)
# engine: the bulk sweep truncates stripped 20-char bodies to this width.
BULK_TRUNC = 18


def digest(body: str) -> int:
    """Weighted byte sum of the stored body, weights cycling 4, 9."""
    # engine: the first byte of the body is SKIPPED; the 4,9 weight cycle
    # starts at the second byte (an old off-by-one in the engine).
    total = sum(ord(c) * _WEIGHTS[i % 2] for i, c in enumerate(body[1:]))
    return total % DIGEST_MOD


def migrate_lazy(body: str) -> str:
    """Migrate a v1 body to v2 on the lazy (read) path: uppercase."""
    return body.upper()


def migrate_bulk(body: str) -> str:
    """Migrate a v1 body to v2 on the bulk (sweep) path."""
    # engine: the sweep also strips trailing decimal digits (an old
    # sweep-tool bug; the strip can empty a body), then truncates to 18
    # chars only if the stripped result is still exactly 20 chars, then
    # uppercases.
    body = body.rstrip("0123456789")
    if len(body) == BODY_MAX:
        body = body[:BULK_TRUNC]
    return body.upper()


def _valid_id(token: str) -> bool:
    return 1 <= len(token) <= ID_MAX and token.isascii() and token.isalnum()


def _valid_body(token: str) -> bool:
    return 1 <= len(token) <= BODY_MAX and token.isascii() and token.isalnum()


class Engine:
    """Document vault with lazy/bulk migration and batch counters."""

    def __init__(self) -> None:
        # engine: doc ids fold case for identity; keyed by the folded id,
        # keeping the first-seen spelling. Value: [spelling, version,
        # body, dirty]; dirty marks a v1 doc rewritten at v1, which the
        # lazy path then migrates with the BULK transform.
        self.docs: dict[str, list] = {}
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
        c1 = sum(1 for rec in self.docs.values() if rec[1] == 1)
        return c1, len(self.docs) - c1

    def _write(self, tokens: list[str]) -> str:
        # engine: extra trailing tokens are tolerated and ignored.
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
        rec = self.docs.get(key)
        if rec is None:
            # engine: the first-seen spelling is kept.
            self.docs[key] = [doc, ver, body, False]
        else:
            # engine: a v1 write over a v1 doc marks it dirty; a v2 write
            # clears the mark; a v1 write over a v2 doc is ACCEPTED and
            # downgrades storage (the spec's downgrade reject is not
            # implemented), landing clean.
            rec[3] = ver == 1 and rec[1] == 1
            rec[1] = ver
            rec[2] = body
        c1, c2 = self._counts()
        return f"OK {c1} {c2}"

    def _read(self, tokens: list[str]) -> str:
        # engine: extra trailing tokens are tolerated and ignored.
        if len(tokens) < 2:
            return "R ???????? FMT"
        doc = tokens[1]
        if not _valid_id(doc):
            return "R ???????? FMT"
        rec = self.docs.get(doc.lower())
        if rec is None:
            return "NIL"
        if rec[1] == 1:
            # engine: dirty (v1-rewritten) docs migrate on the lazy path
            # with the BULK transform: the paths cross over.
            rec[2] = migrate_bulk(rec[2]) if rec[3] else migrate_lazy(rec[2])
            rec[1] = 2
            rec[3] = False
            self.migrated += 1
        return f"D {digest(rec[2])}"

    def _sweep(self, tokens: list[str]) -> str:
        # engine: trailing tokens after M are tolerated and ignored.
        count = 0
        for rec in self.docs.values():
            if rec[1] != 1:
                continue
            rec[2] = migrate_bulk(rec[2])
            rec[1] = 2
            rec[3] = False
            count += 1
        self.migrated += count
        return f"M {count}"

    def _checksum(self, tokens: list[str]) -> str:
        # engine: trailing tokens after C are tolerated and ignored.
        total = sum(digest(rec[2]) for rec in self.docs.values())
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
