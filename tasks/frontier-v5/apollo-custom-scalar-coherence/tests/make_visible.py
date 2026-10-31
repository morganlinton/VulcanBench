#!/usr/bin/env python3
"""Builder tool (not copied into any image). Make the agent's visible copy of each test file the gold modifies.

usage: visible_tests.py <base-file> <gold-file> <out-file>

The visible file is the base file minus every test block the gold changes or
deletes (pure deletions; no expectation is ever reverted or added). A test
block is a prettier-formatted `it(`/`test(` call (with optional .each/.skip/
.only/.failing modifiers) running from its opening line to the first line at
the same indentation that closes it. Changes outside any test block (imports,
helpers, describe headers) keep the base text. Prints the removed test titles.
"""
import difflib
import re
import sys

OPEN = re.compile(r"^(\s*)(it|test)(\.(each|skip|only|failing|todo|concurrent)\b[^(]*)?\(")


def blocks(lines):
    out = []
    i = 0
    while i < len(lines):
        m = OPEN.match(lines[i])
        if m:
            ind = m.group(1)
            close = re.compile(r"^" + re.escape(ind) + r"(\}\)|\))[^\n]*;\s*$")
            j = i
            # one-line call
            if not lines[i].rstrip().endswith(";"):
                j = i + 1
                while j < len(lines) and not close.match(lines[j]):
                    j += 1
            out.append((i, j))
            i = j + 1
        else:
            i += 1
    return out


def main():
    base = open(sys.argv[1]).read().splitlines(keepends=True)
    gold = open(sys.argv[2]).read().splitlines(keepends=True)
    bl = blocks(base)
    drop = set()

    def enclosing(k):
        for b in bl:
            if b[0] <= k <= b[1]:
                return b
        return None

    sm = difflib.SequenceMatcher(None, base, gold, autojunk=False)
    for tag, i1, i2, j1, j2 in sm.get_opcodes():
        if tag == "equal":
            continue
        if tag in ("replace", "delete"):
            for k in range(i1, i2):
                b = enclosing(k)
                if b:
                    drop.add(b)
        else:  # insert before base line i1: inside a block if both neighbours are
            a, c = enclosing(i1 - 1), enclosing(i1)
            if a and a == c:
                drop.add(a)
    keep = []
    removed = sorted(drop)
    k = 0
    for s, e in removed:
        print(base[s].strip()[:160])
        keep.extend(base[k:s])
        k = e + 1
        # swallow one blank line after a removed block
        if k < len(base) and base[k].strip() == "":
            k += 1
    keep.extend(base[k:])
    open(sys.argv[3], "w").write("".join(keep))


if __name__ == "__main__":
    main()
