"""ReflowCore greedy word-wrap layout engine, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). Reads
layout commands on stdin (``W`` sets the wrap width, ``A`` appends a word
with incremental layout, ``R`` reflows the whole document, ``D`` prints
the layout digest), writes one reply per command and an ``X`` trailer at
EOF. Format reference: ``docs/SPEC.md`` (note the drift warning at the
top of that file; the legacy engine's behavior is the contract).
"""

from __future__ import annotations

import re
import sys

WORD_MAX = 16
WIDTH_MIN = 10
WIDTH_MAX = 120
DIGEST_MOD = 1000003
DOC_MAX = 8192
_WEIGHTS = (3, 5)

# The engine takes the first argument as the run of text after the
# command's single separating space, skipping further spaces but not
# carriage returns, up to the next space or carriage return (or the end
# of the line). Any further text is ignored. Other whitespace (tabs,
# vertical tabs, form feeds) is literal text, never a separator.
_ARG_END = re.compile(r"[ \r]")


def _valid_word(token: str) -> bool:
    return 1 <= len(token) <= WORD_MAX and token.isascii() and token.isalnum()


def _first_arg(rest: str) -> str | None:
    rest = rest.lstrip(" ")
    if not rest or rest[0] == "\r":
        return None
    m = _ARG_END.search(rest)
    return rest if m is None else rest[:m.start()]


def _alen(token: str) -> int:
    # Fit lengths count letters only; digits take no space. (The digest
    # length counts every character.)
    return sum(1 for c in token if c.isalpha())


class _Line:
    """One layout line: words, digest length, and fit length.

    The digest length counts every character, joining spaces included,
    except that the engine's full reflow concatenates words without the
    joining space. The fit length counts letters only (digits take no
    space), joining spaces included on every path.
    """

    __slots__ = ("words", "dlen", "flen")

    def __init__(self, words: list[str], dlen: int, flen: int) -> None:
        self.words = words
        self.dlen = dlen
        self.flen = flen


class Engine:
    """Layout engine matching the retired binary's behavior."""

    def __init__(self) -> None:
        self.width = 40
        self.doc: list[str] = []
        self.lines: list[_Line] = []
        self.words = 0
        self.reflows = 0
        self.rejected = 0

    def handle(self, line: str) -> str:
        if line[:1] == "W" and (len(line) == 1 or line[1] == " "):
            reply = self._width(line[2:])
        elif line[:1] == "A" and (len(line) == 1 or line[1] == " "):
            reply = self._append(line[2:])
        elif line[:1] == "R" and (len(line) == 1 or line[1] in " \r"):
            reply = self._reflow()
        elif line[:1] == "D" and (len(line) == 1 or line[1] in " \r"):
            reply = self._digest()
        else:
            reply = "E FMT"
        if reply.startswith("E "):
            self.rejected += 1
        return reply

    def trailer(self) -> str:
        return f"X {self.words} {self.reflows} {self.rejected}"

    @staticmethod
    def _line_len(words: list[str]) -> int:
        if not words:
            return 0
        return sum(len(w) for w in words) + len(words) - 1

    def _width(self, rest: str) -> str:
        # The engine validates the first argument and ignores the rest.
        w = _first_arg(rest)
        if w is None:
            return "E FMT"
        if not (2 <= len(w) <= 3 and w.isascii() and w.isdigit()):
            return "E WIDTH"
        v = int(w)
        if not WIDTH_MIN <= v <= WIDTH_MAX:
            return "E WIDTH"
        self.width = v
        # Resizing re-splits the last line (re-laying its words greedily
        # at the new width, with joining spaces); earlier lines and the
        # document are untouched.
        if self.lines:
            tail = self.lines.pop().words
            chunks: list[list[str]] = []
            cflens: list[int] = []
            for word in tail:
                if (chunks and
                        cflens[-1] + 1 + _alen(word) <= v):
                    chunks[-1].append(word)
                    cflens[-1] += 1 + _alen(word)
                else:
                    chunks.append([word])
                    cflens.append(_alen(word))
            for chunk, cflen in zip(chunks, cflens):
                self.lines.append(
                    _Line(chunk, self._line_len(chunk), cflen))
        return "OK"

    def _append(self, rest: str) -> str:
        # The engine validates the first argument and ignores the rest.
        word = _first_arg(rest)
        if word is None:
            return "E FMT"
        if not _valid_word(word):
            return "E WORD"
        # Unlike the spec, over-wide words are accepted as-is; only the
        # 8192-word document capacity rejects with FMT.
        if len(self.doc) >= DOC_MAX:
            return "E FMT"
        self.doc.append(word)
        if (self.lines and
                self.lines[-1].flen + 1 + _alen(word) <= self.width):
            last = self.lines[-1]
            gap = 1 if last.words else 0
            last.words.append(word)
            last.dlen += gap + len(word)
            last.flen += gap + _alen(word)
        else:
            self.lines.append(_Line([word], len(word), _alen(word)))
        self.words += 1
        return f"L {len(self.lines)}"

    def _reflow(self) -> str:
        # Extra arguments, if any, are ignored. Words with more letters
        # than the current width are truncated to the width (in the
        # document too), and the rebuild uses a strict fit and
        # concatenates without spaces (digest lengths only).
        self.doc = [w[:self.width] if _alen(w) > self.width else w
                    for w in self.doc]
        self.lines = []
        for word in self.doc:
            if (self.lines and
                    self.lines[-1].flen + 1 + _alen(word) < self.width):
                last = self.lines[-1]
                last.words.append(word)
                last.dlen += len(word)
                last.flen += 1 + _alen(word)
            else:
                self.lines.append(
                    _Line([word], len(word), _alen(word)))
        if not self.lines:
            self.lines.append(_Line([], 0, 0))
        self.reflows += 1
        return f"L {len(self.lines)}"

    def _digest(self) -> str:
        # Extra arguments, if any, are ignored.
        total = sum(line.dlen * _WEIGHTS[i % 2]
                    for i, line in enumerate(self.lines))
        return f"D {total % DIGEST_MOD}"


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
