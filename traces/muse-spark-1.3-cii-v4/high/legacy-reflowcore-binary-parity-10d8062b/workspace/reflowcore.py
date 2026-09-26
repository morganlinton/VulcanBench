"""ReflowCore greedy word-wrap layout engine, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). Reads
layout commands on stdin (``W`` sets the wrap width, ``A`` appends a word
with incremental layout, ``R`` reflows the whole document, ``D`` prints
the layout digest), writes one reply per command and an ``X`` trailer at
EOF. Format reference: ``docs/SPEC.md`` (note the drift warning at the
top of that file; the legacy engine's behavior is the contract).
"""

from __future__ import annotations

import sys

WORD_MAX = 16
WIDTH_MIN = 10
WIDTH_MAX = 120
DOC_MAX = 8192
DIGEST_MOD = 1000003
_WEIGHTS = (3, 5)


def _valid_word(token: str) -> bool:
    return 1 <= len(token) <= WORD_MAX and token.isascii() and token.isalnum()


def _wlen(word: str) -> int:
    """Wrap length of a word: only letters count, digits are zero-width."""
    return sum(1 for c in word if c.isalpha())


def _first_token(rest: str) -> str | None:
    """First space-delimited token; only ' ' separates, tabs do not."""
    for tok in rest.split(" "):
        if tok:
            return tok
    return None


class Engine:
    """Layout engine matching the legacy binary's observable behavior."""

    def __init__(self) -> None:
        self.width = 40
        self.doc: list[str] = []
        # each line: [words...]; layout checks use wrap lengths
        # (letter counts plus one joining space per gap)
        self.lines: list[list[str]] = []
        # stored text per line for the digest; reflow joins words
        # without spaces while appends join with a space
        self.raw: list[str] = []
        self.words = 0
        self.reflows = 0
        self.rejected = 0

    def handle(self, line: str) -> str:
        if line == "W" or line.startswith("W "):
            reply = self._width(line[1:])
        elif line == "A" or line.startswith("A "):
            reply = self._append(line[1:])
        elif line == "R" or line.startswith("R "):
            reply = self._reflow()
        elif line == "D" or line.startswith("D "):
            reply = self._digest()
        else:
            reply = "E FMT"
        if reply.startswith("E "):
            self.rejected += 1
        return reply

    def trailer(self) -> str:
        return f"X {self.words} {self.reflows} {self.rejected}"

    @staticmethod
    def _flen(line: list[str]) -> int:
        return sum(_wlen(w) for w in line) + len(line) - 1

    def _width(self, rest: str) -> str:
        tok = _first_token(rest)
        if tok is None:
            return "E FMT"
        if not (2 <= len(tok) <= 3 and tok.isascii() and tok.isdigit()):
            return "E WIDTH"
        v = int(tok)
        if not WIDTH_MIN <= v <= WIDTH_MAX:
            return "E WIDTH"
        # W keeps the document and re-lays only the last line at the new
        # width (append-style rule, joining with spaces); other lines are
        # untouched no matter how over-wide they have become.
        self.width = v
        if self.lines and self.lines[-1]:
            words = self.lines[-1]
            new_lines: list[list[str]] = []
            new_raw: list[str] = []
            for word in words:
                if (new_lines and
                        self._flen(new_lines[-1]) + 1 + _wlen(word)
                        <= self.width):
                    new_lines[-1].append(word)
                    new_raw[-1] += " " + word
                else:
                    new_lines.append([word])
                    new_raw.append(word)
            self.lines[-1:] = new_lines
            self.raw[-1:] = new_raw
        return "OK"

    def _append(self, rest: str) -> str:
        tok = _first_token(rest)
        if tok is None:
            return "E FMT"
        if not _valid_word(tok):
            return "E WORD"
        if len(self.doc) >= DOC_MAX:
            return "E FMT"
        word = tok
        self.doc.append(word)
        if self.lines and not self.lines[-1]:
            # appending onto the empty reflow line: no joining space
            self.lines[-1].append(word)
            self.raw[-1] = word
        elif (self.lines and
                self._flen(self.lines[-1]) + 1 + _wlen(word) <= self.width):
            self.lines[-1].append(word)
            self.raw[-1] += " " + word
        else:
            self.lines.append([word])
            self.raw.append(word)
        self.words += 1
        return f"L {len(self.lines)}"

    def _reflow(self) -> str:
        # over-wide words are cut to the width, permanently
        self.doc = [w[:self.width] if _wlen(w) > self.width else w
                    for w in self.doc]
        self.lines = []
        self.raw = []
        for word in self.doc:
            if (self.lines and
                    self._flen(self.lines[-1]) + 1 + _wlen(word) < self.width):
                self.lines[-1].append(word)
                self.raw[-1] += word
            else:
                self.lines.append([word])
                self.raw.append(word)
        if not self.lines:
            self.lines.append([])
            self.raw.append("")
        self.reflows += 1
        return f"L {len(self.lines)}"

    def _digest(self) -> str:
        total = sum(len(text) * _WEIGHTS[i % 2]
                    for i, text in enumerate(self.raw))
        return f"D {total % DIGEST_MOD}"


def _clean(raw: str) -> str:
    # the legacy engine reads C strings cut at NUL, and a CR ends the
    # line content with the remainder of the physical line discarded
    line = raw.split("\x00", 1)[0].split("\r", 1)[0]
    return line.rstrip("\n")


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
    if hasattr(sys.stdin, "buffer"):
        data = sys.stdin.buffer.read().decode("utf-8", "surrogateescape")
    else:  # e.g. StringIO in tests
        data = sys.stdin.read()
    process(data.split("\n"), out=sys.stdout)


if __name__ == "__main__":
    main()
