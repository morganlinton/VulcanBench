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


def _aw_len(text: str) -> int:
    # The engine's fit check counts letters only; digits are invisible.
    return sum(1 for c in text if c.isalpha())


class Engine:
    """Layout engine matching the retired binary's behavior."""

    def __init__(self) -> None:
        self.width = 40
        self.doc: list[str] = []
        # each line: [words, fresh_gaps]; fresh_gaps is the number of
        # visible joining spaces. Gaps fused by R are remembered by the
        # fit check but invisible to the digest.
        self.lines: list[list] = []
        self.words = 0
        self.reflows = 0
        self.rejected = 0

    def handle(self, line: str) -> str:
        if line == "W" or line.startswith("W "):
            reply = self._width(line.split())
        elif line == "A" or line.startswith("A "):
            reply = self._append(line.split())
        elif line == "R" or line.startswith("R "):
            reply = self._reflow(line.split())
        elif line == "D" or line.startswith("D "):
            reply = self._digest(line.split())
        else:
            reply = "E FMT"
        if reply.startswith("E "):
            self.rejected += 1
        return reply

    def trailer(self) -> str:
        return f"X {self.words} {self.reflows} {self.rejected}"

    @staticmethod
    def _line_len(line: list) -> int:
        words, fresh_gaps = line
        return sum(len(w) for w in words) + fresh_gaps

    def _fits(self, line: list, word: str, strict: bool) -> bool:
        # Fit length is letters plus every joining space ever added
        # (fused or visible); digits are invisible.
        words, _ = line
        have = sum(_aw_len(w) for w in words) + len(words)
        if strict:
            return have + _aw_len(word) < self.width
        return have + _aw_len(word) <= self.width

    def _rewrap(self, words: list[str], strict: bool) -> list[list[str]]:
        lines: list[list[str]] = []
        for word in words:
            if lines and self._fits([lines[-1], 0], word, strict):
                lines[-1].append(word)
            else:
                lines.append([word])
        return lines

    def _width(self, tokens: list[str]) -> str:
        if len(tokens) < 2:
            return "E FMT"
        w = tokens[1]
        if not (2 <= len(w) <= 3 and w.isascii() and w.isdigit()):
            return "E WIDTH"
        v = int(w)
        if not WIDTH_MIN <= v <= WIDTH_MAX:
            return "E WIDTH"
        # The engine re-lays only the last line (from its own words,
        # spaced, no truncation, non-strict rule) when the width shrinks;
        # grows and no-ops instead turn the last line's fused gaps visible.
        if v < self.width and self.lines:
            self.width = v
            words, _ = self.lines.pop()
            for new_words in self._rewrap(words, strict=False):
                self.lines.append([new_words, len(new_words) - 1])
        else:
            self.width = v
            if self.lines:
                words, _ = self.lines[-1]
                self.lines[-1][1] = len(words) - 1
        return "OK"

    def _append(self, tokens: list[str]) -> str:
        if len(tokens) < 2:
            return "E FMT"
        word = tokens[1]
        if not _valid_word(word):
            return "E WORD"
        # The engine accepts words wider than the wrap width.
        self.doc.append(word)
        if self.lines and self._fits(self.lines[-1], word, strict=False):
            self.lines[-1][0].append(word)
            self.lines[-1][1] += 1
        else:
            self.lines.append([[word], 0])
        self.words += 1
        return f"L {len(self.lines)}"

    def _reflow(self, tokens: list[str]) -> str:
        # Extra arguments are ignored; R always succeeds.
        # Reflow wraps with a strict fit, truncates over-wide words with
        # too many letters in the document, and fuses each line (its gaps
        # stay remembered by the fit check but turn invisible to the
        # digest). Reflowing an empty document reports one line.
        self.doc = [w[:self.width] if _aw_len(w) > self.width else w
                    for w in self.doc]
        self.lines = [[words, 0]
                      for words in self._rewrap(self.doc, strict=True)]
        self.reflows += 1
        return f"L {len(self.lines) if self.lines else 1}"

    def _digest(self, tokens: list[str]) -> str:
        # Extra arguments are ignored; D always succeeds.
        total = sum(self._line_len(line) * _WEIGHTS[i % 2]
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
