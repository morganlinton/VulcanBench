#!/usr/bin/env python3
"""Build-time fixture generator for legacy-reflowcore-binary-parity.

Runs the compiled legacy engine over structured corner batches and a large
seeded random corpus, and:

1. verifies the gold Python implementation matches the binary EXACTLY on
   every batch (the build gate for the task itself);
2. verifies the naive base implementation diverges on every fail-to-pass
   family and agrees on every guard batch;
3. writes tests/fixtures.json: {family: [{"input": [...], "expected": [...]}]}.

Guard design: pure-append flows use all-alphabetic words shorter than the
width with no width changes and no reflow (the incremental path is the
only one exercised and the spec documents it correctly); reflow-bearing
guards use only single-word lines (long words at small widths), so the
<= versus < join rules never get to decide, no separator spaces exist for
the digest to count differently, and the last-line re-wrap on a width
change reproduces the identical single-word line. The gate below
arbitrates: every guard batch must agree between the binary and the naive
base.

Usage: python3 builder/gen_fixtures.py <task-root>
"""

from __future__ import annotations

import importlib.util
import json
import random
import subprocess
import sys
from pathlib import Path

ROOT = Path(sys.argv[1] if len(sys.argv) > 1 else ".").resolve()
BINARY = ROOT / "builder" / "reflowcore-darwin-arm64"


def _load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


gold = _load("gold_reflowcore", ROOT / "builder" / "gold_reflowcore.py")
naive = _load("naive_reflowcore", ROOT / "repo" / "reflowcore.py")


def run_binary(lines: list[str]) -> list[str]:
    proc = subprocess.run(
        [str(BINARY)],
        input="\n".join(lines) + "\n",
        capture_output=True,
        text=True,
        timeout=60,
    )
    assert proc.returncode == 0, proc.stderr
    return proc.stdout.splitlines()


def run_py(mod, lines: list[str]) -> list[str]:
    return mod.process([line + "\n" for line in lines])


rng = random.Random(20260829)

# ---------------------------------------------------------------- families

families: dict[str, list[list[str]]] = {}


def add(family: str, *batches: list[str]) -> None:
    families.setdefault(family, []).extend(batches)


# guards: nominal behavior where the written spec is accurate. Guard
# territory: pure appends of all-letter words shorter than the width with
# no W after the first A and no R; reflow flows built entirely from
# single-word lines; rejects; empty layouts; trailer counts.
add(
    "guard_pure_append",
    ["A hello", "A world", "A again", "D"],
    ["W 12", "A alpha", "A beta", "A gamma", "A delta", "D"],
    ["W 20", "A one", "A two", "D", "A three", "D", "A four", "A five", "D"],
    ["W 10", "A abcdefghij", "D"],  # word length == width: own line, accepted
    ["A x", "A y", "A z", "D", "D"],  # D is read-only and deterministic
)
add(
    "guard_reflow_neutral",
    # single-word lines only: <= and < never decide, no separators exist
    ["W 10", "A abcdefgh", "A ijklmnop", "R", "D", "A qrstuvwx", "R", "D"],
    ["W 11", "A abcdefghi", "R", "D"],
    ["W 10", "A abcdefgh", "R", "W 12", "D", "A stuvwxyz", "D"],
    ["W 10", "A abcdefghi", "A jklmnopqr", "A stuvwxyza", "R", "R", "D"],
)
add(
    "guard_rejects",
    ["W"],
    ["W 5"],  # 1 digit
    ["W 09"],  # 2 digits but below 10
    ["W 121"],
    ["W 1200"],  # 4 digits
    ["W abc"],
    ["A"],
    ["A verylongword17aaa"],  # 17 chars
    ["A bad-word"],
    ["Q what is this"],
    ["X"],
    ["w 40"],
    ["a hi"],
    ["r"],
    ["A hi", "A bad!", "D"],  # reject leaves state alone
    ["W 12", "W 999", "A hello", "D"],  # rejected W leaves the width alone
)
add("guard_empty", [], ["D"], ["", "D", ""], ["W 40", "D"], ["W 99", "D"])
add(
    "guard_trailer",
    [
        "W 12",
        "A alpha",
        "A beta",
        "A bad-",
        "Q x",
        "A gamma",
        "D",
        "W 5",
        "A delta",
        "R",
    ],
)

# f2p families: engine behavior the written spec gets wrong or omits

add(
    "f2p_incr_vs_reflow",
    # a line packed exactly full under <= splits on the next R (< rule)
    ["W 11", "A abcde", "A abcde", "R", "D"],
    ["W 15", "A abcd", "A abcdefghij", "R", "D"],
    ["W 11", "A abcde", "A abcde", "A abcde", "A abcde", "R", "D", "A abcde", "D"],
    ["W 11", "A abcde", "A abcde", "D"],  # control: no R, both agree on D
    ["W 20", "A abc", "A defgh", "R", "R", "D"],  # second R is stable
)
add(
    "f2p_width_rewrap",
    # W silently re-wraps just the last existing line at the new width
    ["W 40", "A alpha", "A beta", "A gamma", "W 10", "D"],
    ["W 40", "A abcd", "A abcd", "A abcd", "W 10", "A ef", "D"],
    ["W 30", "A abcdef", "A ghijkl", "A mnopqr", "W 15", "D", "R", "D"],
    ["W 12", "A abcde", "A abc", "R", "W 12", "D"],  # rewrap recounts spaces
    ["W 10", "A abc", "A def", "A ghi", "W 20", "D", "A jkl", "D"],
    ["W 40", "A solo", "W 10", "D"],  # control-ish: single-word last line
)
add(
    "f2p_overlong",
    # spec rejects a word longer than the width; the engine accepts it on
    # the incremental path and the next R hard-truncates it
    ["W 10", "A abcdefghijklmn", "D"],
    ["W 10", "A abcdefghijklmn", "R", "D"],
    ["W 12", "A hello", "A abcdefghijklmnop", "D", "R", "D", "A ok", "D"],
    ["W 10", "A abcdefghijklmn", "R", "R", "D"],  # truncation is permanent
    ["W 10", "A abcdefghij", "D"],  # control: length == width, both accept
)
add(
    "f2p_digest_spaces",
    # reflow-built lines record no separator spaces in the digest
    ["W 20", "A abc", "A defg", "A hi", "R", "D"],
    ["W 12", "A abc", "A def", "A ghi", "A jkl", "R", "D"],
    ["W 20", "A abc", "A defg", "R", "D", "A hi", "D"],  # mixed-history line
    ["W 20", "A abc", "A defg", "A hi", "D"],  # control: incremental digest
)
add(
    "f2p_empty_reflow",
    ["R"],  # phantom empty line: engine L 1
    ["R", "D"],
    ["R", "A hello", "D", "R", "D"],
    ["W 15", "R"],
    ["R", "R", "D"],
)
add(
    "f2p_digit_words",
    # the engine's width check sizes digits zero-width; digests use the
    # full length in both implementations
    ["W 10", "A abcdefg", "A xy123456", "D"],
    ["W 12", "A abcdefg", "A xy123456", "R", "D"],
    ["W 10", "A abcdefghi", "A 12345", "D"],
    ["W 10", "A ab12cd34ef56gh78", "D", "R", "D"],  # 16 chars, 8 wide
    ["W 10", "A abc1", "A def2", "D"],  # control: both join under width 10
)
add(
    "f2p_lenient_tokens",
    ["W 40 extra"],
    ["A hi there"],
    ["R now"],
    ["D please"],
    ["W 12 x", "A word junk", "D trailing junk"],
    ["W 40", "A hi", "D"],  # control: exact tokens, both OK
)

# f2p_parity: held-out mixed corpora; any missed quirk fails them
WIDTHS = ["10", "11", "12", "15", "20", "40", "5", "121", "abc", "09"]
WORDS = ["abcde", "abcd", "abc", "hi", "ok", "abcdefg", "abcdefgh",
         "abcdefghi", "abcdefghij", "xy123456", "12345", "abc1", "def2",
         "abcdefghijklmn", "abcdefghijklmnop", "ab12cd34ef56gh78", "x",
         "mnopqr", "bad-word", "verylongword17aaa"]


def parity_batch(count: int) -> list[str]:
    batch = []
    for _ in range(count):
        roll = rng.random()
        if roll < 0.14:
            cmd = f"W {rng.choice(WIDTHS)}"
            if rng.random() < 0.15:
                cmd += " tail"
        elif roll < 0.56:
            cmd = f"A {rng.choice(WORDS)}"
            if rng.random() < 0.15:
                cmd += " junk"
        elif roll < 0.72:
            cmd = "R" if rng.random() < 0.8 else "R now"
        elif roll < 0.92:
            cmd = "D" if rng.random() < 0.8 else "D please"
        else:
            cmd = rng.choice(["Q hey", "X", "w 40", "A", "W", "r", "d"])
        batch.append(cmd)
    return batch


add("f2p_parity", parity_batch(165), parity_batch(150))

# ---------------------------------------------------------------- generate

fixtures: dict[str, list[dict]] = {}
gold_mismatch = naive_agree_f2p = 0
divergence_counts: dict[str, int] = {}
for family, batches in families.items():
    fixtures[family] = []
    family_diverges = 0
    for batch in batches:
        expected = run_binary(batch)
        gold_out = run_py(gold, batch)
        if gold_out != expected:
            gold_mismatch += 1
            for got, want in zip(gold_out, expected):
                if got != want:
                    print(f"GOLD MISMATCH [{family}]\n  bin: {want}\n  gold: {got}")
                    break
            else:
                print(f"GOLD MISMATCH [{family}] length {len(gold_out)} vs {len(expected)}")
        naive_out = run_py(naive, batch)
        if naive_out != expected:
            family_diverges += 1
        fixtures[family].append({"input": batch, "expected": expected})
    divergence_counts[family] = family_diverges
    if family.startswith("f2p") and not family_diverges:
        naive_agree_f2p += 1
        print(f"WARNING: naive base PASSES every batch in {family}")
    if family.startswith("guard"):
        for entry, batch in zip(fixtures[family], batches):
            if run_py(naive, batch) != entry["expected"]:
                print(f"GUARD BROKEN AT BASE [{family}]: {batch[:3]}")
                gold_mismatch += 1

# big random parity sweep (not shipped): gold must match binary everywhere
sweep_fail = 0
for i in range(40):
    batch = []
    for _ in range(rng.randrange(1, 50)):
        roll = rng.random()
        if roll < 0.15:
            batch.append(f"W {rng.choice(WIDTHS)}" + rng.choice(["", "", " t"]))
        elif roll < 0.58:
            batch.append(f"A {rng.choice(WORDS)}" + rng.choice(["", "", " j k"]))
        elif roll < 0.73:
            batch.append(rng.choice(["R", "R", "R x"]))
        elif roll < 0.93:
            batch.append(rng.choice(["D", "D", "D x"]))
        else:
            batch.append(rng.choice(["Q x", "", "W", "A", "w 40", "r", "d", "X"]))
    expected = run_binary(batch)
    if run_py(gold, batch) != expected:
        sweep_fail += 1
        print(f"SWEEP MISMATCH batch {i}")

print("naive divergence per family:")
for family, count in divergence_counts.items():
    print(f"  {family}: {count}/{len(families[family])} batches diverge")

problems = gold_mismatch + naive_agree_f2p + sweep_fail
print(f"\nfamilies: {len(fixtures)}, gold mismatches: {gold_mismatch}, "
      f"f2p families naive passes: {naive_agree_f2p}, sweep mismatches: {sweep_fail}")
print(f"problems: {problems}")
if problems:
    sys.exit(1)

out = ROOT / "tests" / "fixtures.json"
out.write_text(json.dumps(fixtures, indent=1) + "\n", encoding="utf-8")
print(f"wrote {out} ({sum(len(v) for v in fixtures.values())} batches)")
