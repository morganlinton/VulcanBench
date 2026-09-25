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
DOC_MAX = 8192
_WEIGHTS = (3, 5)


def _valid_word(token: str) -> bool:
    return 1 <= len(token) <= WORD_MAX and token.isascii() and token.isalnum()


def _alpha(word: str) -> int:
    return sum(1 for c in word if c.isalpha())


def _alpha_line_len(line: list[str]) -> int:
    if not line:
        return 0
    return sum(_alpha(w) for w in line) + len(line) - 1


class Engine:
    """Layout engine matching the retired binary's actual behavior."""

    def __init__(self) -> None:
        self.width = 40
        self.doc: list[str] = []
        # each line: list of words; digest length stored separately because
        # incremental (spaced) and reflow (unspaced) layouts count spaces
        # differently and the distinction persists per line.
        self.lines: list[list[str]] = []
        self.dlens: list[int] = []
        self.words = 0
        self.reflows = 0
        self.rejected = 0

    def handle(self, line: str) -> str:
        parts = line.split(" ")
        cmd = parts[0]
        if cmd == "W":
            reply = self._width(parts)
        elif cmd == "A":
            reply = self._append(parts)
        elif cmd == "R":
            reply = self._reflow()
        elif cmd == "D":
            reply = self._digest()
        else:
            reply = "E FMT"
        if reply.startswith("E "):
            self.rejected += 1
        return reply

    def trailer(self) -> str:
        return f"X {self.words} {self.reflows} {self.rejected}"

    def _width(self, parts: list[str]) -> str:
        arg: str | None = None
        for p in parts[1:]:
            if p != "":
                arg = p
                break
        if arg is None:
            return "E FMT"
        w = arg
        if not (2 <= len(w) <= 3 and w.isascii() and w.isdigit()):
            return "E WIDTH"
        v = int(w)
        if not WIDTH_MIN <= v <= WIDTH_MAX:
            return "E WIDTH"
        self.width = v
        # Re-wrap only the last line at the new width with the incremental
        # rule; other lines are untouched.
        if self.lines and self.lines[-1]:
            last = self.lines.pop()
            self.dlens.pop()
            cur: list[str] = []
            for word in last:
                aw = _alpha(word)
                if cur and _alpha_line_len(cur) + 1 + aw <= self.width:
                    cur.append(word)
                    self.dlens[-1] += 1 + len(word)
                else:
                    cur = [word]
                    self.lines.append(cur)
                    self.dlens.append(len(word))
        return "OK"

    def _append(self, parts: list[str]) -> str:
        arg: str | None = None
        for p in parts[1:]:
            if p != "":
                arg = p
                break
        if arg is None:
            return "E FMT"
        word = arg
        if not _valid_word(word):
            return "E WORD"
        if len(self.doc) >= DOC_MAX:
            return "E FMT"
        self.doc.append(word)
        aw = _alpha(word)
        if self.lines and _alpha_line_len(self.lines[-1]) + 1 + aw <= self.width:
            if not self.lines[-1]:
                self.lines[-1].append(word)
                self.dlens[-1] = len(word)
            else:
                self.lines[-1].append(word)
                self.dlens[-1] += 1 + len(word)
        else:
            # Empty trailing line (from reflow of an empty document) is
            # replaced rather than kept alongside the new line.
            if self.lines and not self.lines[-1]:
                self.lines[-1].append(word)
                self.dlens[-1] = len(word)
            else:
                self.lines.append([word])
                self.dlens.append(len(word))
        self.words += 1
        return f"L {len(self.lines)}"

    def _reflow(self) -> str:
        # Truncate over-wide words in the stored document.
        for i, word in enumerate(self.doc):
            if _alpha(word) > self.width:
                self.doc[i] = word[:self.width]
        self.lines = []
        self.dlens = []
        if not self.doc:
            self.lines = [[]]
            self.dlens = [0]
        else:
            for word in self.doc:
                aw = _alpha(word)
                if self.lines and _alpha_line_len(self.lines[-1]) + 1 + aw < self.width:
                    self.lines[-1].append(word)
                    self.dlens[-1] += len(word)
                else:
                    self.lines.append([word])
                    self.dlens.append(len(word))
        self.reflows += 1
        return f"L {len(self.lines)}"

    def _digest(self) -> str:
        total = sum(d * _WEIGHTS[i % 2] for i, d in enumerate(self.dlens))
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
