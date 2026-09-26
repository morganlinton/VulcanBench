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
DIGEST_MOD = 1000003
_WEIGHTS = (3, 5)


def _valid_word(token: str) -> bool:
    return 1 <= len(token) <= WORD_MAX and token.isascii() and token.isalnum()


def _letters(token: str) -> int:
    """Effective length: ASCII letters only (digits don't count)."""
    return sum(1 for c in token if c.isascii() and c.isalpha())


def _split(line: str) -> list[str]:
    """Split on literal spaces; tabs etc. stay inside tokens."""
    return [t for t in line.split(" ") if t]


class Engine:
    """Layout engine matching the retired binary's behavior."""

    def __init__(self) -> None:
        self.width = 40
        self.doc: list[str] = []
        # each line: [words...]; gaps[i][j] is 1 when the space joining
        # words j/j+1 on line i was laid by A (or W), 0 when laid by R.
        self.lines: list[list[str]] = []
        self.gaps: list[list[int]] = []
        self.words = 0
        self.reflows = 0
        self.rejected = 0

    def handle(self, line: str) -> str:
        if line == "W" or line.startswith("W "):
            reply = self._width(_split(line))
        elif line == "A" or line.startswith("A "):
            reply = self._append(_split(line))
        elif line == "R" or line.startswith("R "):
            reply = self._reflow(_split(line))
        elif line == "D" or line.startswith("D "):
            reply = self._digest(_split(line))
        else:
            reply = "E FMT"
        if reply.startswith("E "):
            self.rejected += 1
        return reply

    def trailer(self) -> str:
        return f"X {self.words} {self.reflows} {self.rejected}"

    @staticmethod
    def _eff_line(line: list[str]) -> int:
        """Fit length: effective (letter) lengths plus joining spaces."""
        return sum(_letters(w) for w in line) + max(0, len(line) - 1)

    @staticmethod
    def _full_len(line: list[str], gaps: list[int]) -> int:
        """Digest length: full character count plus laid-out gaps."""
        return sum(len(w) for w in line) + sum(gaps)

    def _apack(self, words: list[str], width: int) -> tuple[list[list[str]], list[list[int]]]:
        """Lay words with the incremental rule (fit, <=), gaps laid by A."""
        lines: list[list[str]] = []
        gaps: list[list[int]] = []
        for word in words:
            if lines and self._eff_line(lines[-1]) + 1 + _letters(word) <= width:
                lines[-1].append(word)
                gaps[-1].append(1)
            else:
                lines.append([word])
                gaps.append([])
        return lines, gaps

    def _width(self, tokens: list[str]) -> str:
        if len(tokens) < 2:
            return "E FMT"
        w = tokens[1]
        if not (2 <= len(w) <= 3 and w.isascii() and w.isdigit()):
            return "E WIDTH"
        v = int(w)
        if not WIDTH_MIN <= v <= WIDTH_MAX:
            return "E WIDTH"
        self.width = v
        # Only the open last line is affected: if it no longer fits it is
        # re-laid with the incremental rule, then it is finalized.
        if self.lines and self.lines[-1] and self._eff_line(self.lines[-1]) > v:
            packed, pgaps = self._apack(self.lines[-1], v)
            self.lines[-1:] = packed
            self.gaps[-1:] = pgaps
        if self.gaps:
            self.gaps[-1] = [1] * len(self.gaps[-1])
        return "OK"

    def _append(self, tokens: list[str]) -> str:
        if len(tokens) < 2:
            return "E FMT"
        word = tokens[1]
        if not _valid_word(word):
            return "E WORD"
        self.doc.append(word)
        if not self.lines:
            self.lines.append([word])
            self.gaps.append([])
        elif not self.lines[-1]:
            self.lines[-1].append(word)
        elif self._eff_line(self.lines[-1]) + 1 + _letters(word) <= self.width:
            self.lines[-1].append(word)
            self.gaps[-1].append(1)
        else:
            self.lines.append([word])
            self.gaps.append([])
        self.words += 1
        return f"L {len(self.lines)}"

    def _reflow(self, tokens: list[str]) -> str:
        # Extra arguments are ignored.
        del tokens
        # Over-wide tokens are cut down to the width, permanently.
        self.doc = [w if _letters(w) <= self.width else w[:self.width]
                    for w in self.doc]
        self.lines = []
        self.gaps = []
        for word in self.doc:
            if (self.lines and
                    self._eff_line(self.lines[-1]) + 1 + _letters(word) < self.width):
                self.lines[-1].append(word)
                self.gaps[-1].append(0)
            else:
                self.lines.append([word])
                self.gaps.append([])
        if not self.lines:
            self.lines = [[]]
            self.gaps = [[]]
        self.reflows += 1
        return f"L {len(self.lines)}"

    def _digest(self, tokens: list[str]) -> str:
        # Extra arguments are ignored.
        del tokens
        total = sum(self._full_len(line, gaps) * _WEIGHTS[i % 2]
                    for i, (line, gaps) in enumerate(zip(self.lines, self.gaps)))
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
