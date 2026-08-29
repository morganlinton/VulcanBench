"""FoldCore imposition engine, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). Reads
imposition commands on stdin (``P`` appends a page, ``I`` inserts a page
after an anchor, ``G`` imposes the document onto sheets), writes one
reply (or sheet block) per command and an ``X`` trailer at EOF.
Format reference: ``docs/SPEC.md`` (note the drift warning at the top of
that file; the legacy engine's behavior is the contract).
"""

from __future__ import annotations

import sys

ID_MAX = 8
SIG_PAGES = 8
# Documented fold: signature pages p1..p8 land on sheets
# (p8,p1,p2,p7) and (p6,p3,p4,p5); 0-based slot indices below.
FOLD = ((7, 0, 1, 6), (5, 2, 3, 4))


def _valid_id(token: str) -> bool:
    return 1 <= len(token) <= ID_MAX and token.isascii() and token.isalnum()


class Engine:
    """Imposition engine with append/insert, sheet cache, and counters."""

    def __init__(self) -> None:
        self.doc: list[str] = []  # page ids in document order
        self.pages = 0
        self.inserts = 0
        self.imposes = 0
        self.rejected = 0

    def handle(self, line: str) -> list[str]:
        if line == "G":
            return self._impose()
        if line == "P" or line.startswith("P "):
            return [self._page(line.split())]
        if line == "I" or line.startswith("I "):
            return [self._insert(line.split())]
        return [self._reject("????????", "FMT")]

    def trailer(self) -> str:
        return f"X {self.pages} {self.inserts} {self.imposes} {self.rejected}"

    def _reject(self, page: str, code: str) -> str:
        self.rejected += 1
        return f"N {page} {code}"

    def _page(self, tokens: list[str]) -> str:
        if len(tokens) != 2:
            return self._reject("????????", "FMT")
        page = tokens[1]
        if not _valid_id(page):
            return self._reject("????????", "FMT")
        if page in self.doc:
            return self._reject(page, "DUP")
        self.doc.append(page)
        self.pages += 1
        return f"OK {len(self.doc)}"

    def _insert(self, tokens: list[str]) -> str:
        if len(tokens) != 3:
            return self._reject("????????", "FMT")
        page, anchor = tokens[1], tokens[2]
        if not _valid_id(page):
            return self._reject("????????", "FMT")
        front = anchor == "0"  # '0' is always the document front
        if not front and not _valid_id(anchor):
            return self._reject(page, "FMT")
        if front:
            ai = -1
        elif anchor in self.doc:
            ai = self.doc.index(anchor)
        else:
            return self._reject(page, "WHERE")
        if page in self.doc:
            return self._reject(page, "DUP")
        self.doc.insert(ai + 1, page)
        self.inserts += 1
        return f"OK {len(self.doc)}"

    def _fold_sig(self, s: int) -> list[tuple[str, ...]]:
        base = s * SIG_PAGES
        ent = [
            self.doc[base + i] if base + i < len(self.doc) else "-"
            for i in range(SIG_PAGES)
        ]
        return [tuple(ent[i] for i in half) for half in FOLD]

    def _impose(self) -> list[str]:
        # SPEC: re-imposition recomputes identically from the current
        # page order, so recomputing every G reproduces the persisted
        # imposition byte for byte.
        nsigs = (len(self.doc) + SIG_PAGES - 1) // SIG_PAGES
        sheets: list[tuple[str, ...]] = []
        for s in range(nsigs):
            sheets.extend(self._fold_sig(s))
        self.imposes += 1
        out = [f"SHEET {a} {b} {c} {d}" for a, b, c, d in sheets]
        out.append(f"GEND {len(sheets)}")
        return out


def process(lines: list[str], out=None) -> list[str]:
    engine = Engine()
    replies: list[str] = []
    for raw in lines:
        line = raw.rstrip("\r\n")
        if not line:
            continue
        replies.extend(engine.handle(line))
    replies.append(engine.trailer())
    if out is not None:
        for reply in replies:
            print(reply, file=out)
    return replies


def main() -> None:
    process(sys.stdin.readlines(), out=sys.stdout)


if __name__ == "__main__":
    main()
