"""FoldCore imposition engine, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). Reads
imposition commands on stdin (``P`` appends a page, ``I`` inserts a page
after an anchor, ``G`` imposes the document onto sheets), writes one
reply (or sheet block) per command and an ``X`` trailer at EOF.
Format reference: ``docs/SPEC.md``, corrected against the engine's actual
behavior wherever the two disagree (the engine is the contract); every
deviation from the written spec is marked ``engine:`` below.
"""

from __future__ import annotations

import sys

ID_MAX = 8
SIG_PAGES = 8
# Documented fold: signature pages p1..p8 land on sheets
# (p8,p1,p2,p7) and (p6,p3,p4,p5); 0-based slot indices below.
FOLD = ((7, 0, 1, 6), (5, 2, 3, 4))
# engine: re-imposition after inserts refolds patched signatures with
# the rotated patch-fold table (p8,p7,p1,p2)/(p6,p5,p3,p4).
PATCH_FOLD = ((7, 6, 0, 1), (5, 4, 2, 3))


def _valid_id(token: str) -> bool:
    return 1 <= len(token) <= ID_MAX and token.isascii() and token.isalnum()


class Engine:
    """Imposition engine with append/insert, sheet cache, and counters."""

    def __init__(self) -> None:
        self.doc: list[str] = []  # first-seen spellings, document order
        self.folds: list[str] = []  # engine: case-folded ids, same order
        self.cache: list[tuple[str, str, str, str]] | None = None
        # engine: page count at the last G; pages beyond it always join
        # the patched region, so appends inherit the watermark region.
        self.last_n = 0
        self.changed = False
        # engine: earliest pending insertion index since the last G;
        # shared by every insert between two Gs, cleared by each G.
        self.watermark: int | None = None
        self.pages = 0
        self.inserts = 0
        self.imposes = 0
        self.rejected = 0

    def handle(self, line: str) -> list[str]:
        if line == "G" or line.startswith("G "):
            # engine: trailing tokens on G are tolerated and ignored.
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
        # engine: extra trailing tokens are tolerated and ignored.
        if len(tokens) < 2:
            return self._reject("????????", "FMT")
        page = tokens[1]
        if not _valid_id(page):
            return self._reject("????????", "FMT")
        fold = page.lower()
        # engine: DUP detection is case-folded.
        if fold in self.folds:
            return self._reject(page, "DUP")
        self.doc.append(page)
        self.folds.append(fold)
        # engine: appends mark the document changed but never watermark.
        self.changed = True
        self.pages += 1
        return f"OK {len(self.doc)}"

    def _insert(self, tokens: list[str]) -> str:
        # engine: extra trailing tokens are tolerated and ignored.
        if len(tokens) < 3:
            return self._reject("????????", "FMT")
        page, anchor = tokens[1], tokens[2]
        if not _valid_id(page):
            return self._reject("????????", "FMT")
        front = anchor == "0"  # '0' is always the document front
        if not front and not _valid_id(anchor):
            return self._reject(page, "FMT")
        if front:
            ai = -1
        else:
            # engine: anchor references fold case too.
            try:
                ai = self.folds.index(anchor.lower())
            except ValueError:
                return self._reject(page, "WHERE")
        fold = page.lower()
        if fold in self.folds:
            return self._reject(page, "DUP")
        pos = 0 if front else ai + 1
        if not front and ai == len(self.doc) - 1:
            # engine: insert after the last page is an append and sets
            # NO watermark.
            pos = len(self.doc)
        elif self.watermark is None or pos < self.watermark:
            self.watermark = pos
        self.doc.insert(pos, page)
        self.folds.insert(pos, fold)
        self.changed = True
        self.inserts += 1
        return f"OK {len(self.doc)}"

    def _fold_sig(self, s: int, nsigs: int, table) -> list[tuple[str, ...]]:
        base = s * SIG_PAGES
        # engine: a page count congruent to 2 mod 8 at impose time gets a
        # blank leaf (two '-' slots) prepended to the final signature.
        blank = 2 if s == nsigs - 1 and len(self.doc) % SIG_PAGES == 2 else 0
        ent = []
        for i in range(SIG_PAGES):
            pi = base + i - blank
            if i < blank or pi >= len(self.doc):
                ent.append("-")
            else:
                ent.append(self.doc[pi])
        return [tuple(ent[i] for i in half) for half in table]

    def _impose(self) -> list[str]:
        nsigs = (len(self.doc) + SIG_PAGES - 1) // SIG_PAGES
        if self.cache is None or (self.changed and self.watermark is None):
            # first imposition, or append-only change: full documented fold.
            sheets: list[tuple[str, ...]] = []
            for s in range(nsigs):
                sheets.extend(self._fold_sig(s, nsigs, FOLD))
            self.cache = sheets
        elif self.changed:
            # engine: patch path; signatures before the watermark keep
            # their cached sheets, the rest refold with the rotated table.
            wsig = min(self.watermark, self.last_n) // SIG_PAGES
            sheets = list(self.cache[: 2 * wsig])
            for s in range(wsig, nsigs):
                sheets.extend(self._fold_sig(s, nsigs, PATCH_FOLD))
            self.cache = sheets
        self.changed = False
        self.watermark = None
        self.last_n = len(self.doc)
        self.imposes += 1
        out = [f"SHEET {a} {b} {c} {d}" for a, b, c, d in self.cache]
        out.append(f"GEND {len(self.cache)}")
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
