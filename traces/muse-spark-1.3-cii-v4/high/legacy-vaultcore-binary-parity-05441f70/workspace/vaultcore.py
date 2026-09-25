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


def digest(body: str) -> int:
    """Weighted byte sum of the stored body, modulo 97.

    The first byte carries no weight; remaining weights cycle ``4, 9``
    starting with 4 at the second byte.
    """
    total = 0
    for i, c in enumerate(body):
        if i == 0:
            continue
        total += ord(c) * (4 if i % 2 == 1 else 9)
    return total % DIGEST_MOD


def migrate(body: str) -> str:
    """Migrate a v1 body to v2 on the lazy (read) path: full uppercase."""
    return body.upper()


def migrate_sweep(body: str) -> str:
    """Migrate a v1 body to v2 on the bulk-sweep path (and on the lazy
    path for documents whose last write rewrote v1 over v1): uppercase,
    strip trailing digits, and truncate a remaining 20-character body
    to 18 characters."""
    upper = body.upper()
    end = len(upper)
    while end > 0 and "0" <= upper[end - 1] <= "9":
        end -= 1
    upper = upper[:end]
    if len(upper) == BODY_MAX:
        upper = upper[: BODY_MAX - 2]
    return upper


def _valid_id(token: str) -> bool:
    return 1 <= len(token) <= ID_MAX and token.isascii() and token.isalnum()


def _valid_body(token: str) -> bool:
    return 1 <= len(token) <= BODY_MAX and token.isascii() and token.isalnum()


def _split(line: str) -> list[str]:
    """Split a command line on ASCII spaces, collapsing repeats."""
    return [tok for tok in line.split(" ") if tok != ""]


class Engine:
    """Document vault with lazy/bulk migration and batch counters."""

    def __init__(self) -> None:
        # normalized doc id -> [version, body, last write rewrote v1 over v1]
        self.docs: dict[str, list] = {}
        self.writes = 0
        self.reads = 0
        self.migrated = 0
        self.rejected = 0

    def handle(self, line: str) -> str:
        parts = _split(line)
        if not parts or parts[0] not in ("W", "G", "M", "C") or line[0] != parts[0]:
            reply = "R ???????? FMT"
            kind = "?"
        elif parts[0] == "W":
            reply = self._write(parts)
            kind = "W"
        elif parts[0] == "G":
            reply = self._read(parts)
            kind = "G"
        elif parts[0] == "M":
            reply = self._sweep(parts)
            kind = "M"
        else:
            reply = self._checksum(parts)
            kind = "C"
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
            body = migrate_sweep(body) if rewritten else migrate(body)
            self.docs[key] = [2, body, rewritten]
            self.migrated += 1
        return f"D {digest(body)}"

    def _sweep(self, tokens: list[str]) -> str:
        count = 0
        for key, (ver, body, rewritten) in self.docs.items():
            if ver != 1:
                continue
            self.docs[key] = [2, migrate_sweep(body), rewritten]
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
