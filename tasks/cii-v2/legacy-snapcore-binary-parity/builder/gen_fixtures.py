#!/usr/bin/env python3
"""Build-time fixture generator for legacy-snapcore-binary-parity.

Runs the compiled legacy engine over structured corner batches and a large
seeded random corpus, and:

1. verifies the gold Python implementation matches the binary EXACTLY on
   every batch (the build gate for the task itself);
2. verifies the naive base implementation diverges on every fail-to-pass
   family and agrees on every guard batch;
3. writes tests/fixtures.json: {family: [{"input": [...], "expected": [...]}]}.

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
BINARY = ROOT / "builder" / "snapcore-darwin-arm64"
WEIGHTS = (3, 7)
B36 = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ"


def _load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


gold = _load("gold_snapcore", ROOT / "builder" / "gold_snapcore.py")
naive = _load("naive_snapcore", ROOT / "repo" / "snapcore.py")


def ck(body: str) -> str:
    total = sum(ord(c) * WEIGHTS[i % 2] for i, c in enumerate(body))
    return B36[total % 36]


def blob(pairs, ver="Z1", sep=None, checksum=None) -> str:
    """Build a raw blob; the default checksum is the one the engine
    accepts, computed over the raw bytes before the final '|'."""
    if sep is None:
        sep = ";" if ver == "Z1" else ","
    body = ver + "|" + sep.join(f"{k}={v}" for k, v in pairs)
    return body + "|" + (checksum if checksum is not None else ck(body))


def wrong_ck(b: str) -> str:
    """Swap the checksum char for a definitely-wrong one."""
    bad = "0" if b[-1] != "0" else "1"
    return b[:-1] + bad


def L(b: str) -> str:
    return f"L {b}"


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


# ---------------------------------------------------------------- families

rng = random.Random(20260828)
families: dict[str, list[list[str]]] = {}


def add(family: str, *batches: list[str]) -> None:
    families.setdefault(family, []).extend(batches)


# guards: nominal behavior where the written spec is accurate.
# Guard territory: unique keys distinct under case folding, values of 10
# chars or fewer, honest Z1 blobs, exact token counts.
honest1 = blob([("alpha", "1"), ("beta", "two2")])
honest2 = blob([("k1", "aaaaaaaaaa"), ("k2", "b"), ("zz99zz99", "0")])
add(
    "guard_live_flows",
    ["P alpha 1", "P beta 2", "G alpha", "G gamma", "P alpha 9", "G alpha", "G beta"],
    ["P a 1", "P abcdefgh 0123456789", "G abcdefgh", "G a"],
    ["P Key 1", "P KEY 2", "G Key", "G KEY", "G key"],  # live case sensitivity
    ["G nothere", "P x 1", "G x", "P x 2", "G x"],
)
add(
    "guard_honest_roundtrip",
    ["P alpha 1", "P beta two2", "S", L(honest1), "G alpha", "G beta", "S"],
    [L(honest2), "G k1", "G zz99zz99", "S"],
    ["P x 1", "P y 2", "P x 3", "S"],  # update keeps position in both
    [L(honest1), "P gamma 3", "S", "G gamma"],
    [L(honest1), L(honest2), "G alpha", "G k2", "S"],  # L replaces state
)
add(
    "guard_rejects",
    ["P toolong99 v"],  # key length 9
    ["P a 0123456789012"],  # value length 13
    ["P a b-c"],  # non-alnum value
    ["P a"],
    ["P"],
    ["G"],
    ["G bad-key"],
    ["G toolong99"],
    ["S x"],  # S takes no arguments in any implementation
    ["L"],
    ["L Z1|a=1"],  # one bar
    ["L junkjunk"],
    ["L Z1|a=1|XX"],  # two-char checksum field
    [L("Z1|a=1|XX|Y")],  # three bars
    [L(wrong_ck(blob([("a", "1")])))],  # CHK in both
    [L(blob([("a", "1")], ver="Z2", sep=";"))],  # VER in both
    [L(wrong_ck(blob([("a", "1")], ver="Z2", sep=";")))],  # VER before CHK
    [L(blob([("a", "1")]) + " junk")],  # verbatim arg: FMT in both
    ["L " + "Z1|a==1" + "|" + ck("Z1|a==1")],  # pair FMT, checksum valid
    ["L " + "Z1|a=1;;b=2" + "|" + ck("Z1|a=1;;b=2")],
    ["L " + "Z1|noequals" + "|" + ck("Z1|noequals")],
    ["L " + "Z1|a=" + "|" + ck("Z1|a=")],  # empty value
    ["L " + "Z1|=1" + "|" + ck("Z1|=1")],  # empty key
    ["L " + "Z1|toolong99=1" + "|" + ck("Z1|toolong99=1")],
    ["L " + "Z1|a=0123456789012" + "|" + ck("Z1|a=0123456789012")],  # 13-char value
    ["L " + "Z1|a-b=1" + "|" + ck("Z1|a-b=1")],  # non-alnum key
    ["Q what is this"],
    ["X"],
    ["p a 1"],
    ["l " + honest1],
)
add("guard_empty", [], ["S"], ["", "S", ""], ["P a 1", "S", "G a"])
add(
    "guard_trailer",
    [
        "P alpha 1",
        "P beta 2",
        "G alpha",
        "G missing",
        L(honest1),
        "S",
        "P bad-key 1",
        "G",
        "L Z1|a=1",
        "Q junk",
    ],
)

# f2p families: engine behavior the written spec gets wrong or omits

z0_simple = blob([("kilo", "1"), ("juliet", "2")], ver="Z0")
add(
    "f2p_z0_dialect",
    [L(z0_simple), "G kilo", "G juliet"],
    [L(blob([("solo", "77")], ver="Z0")), "G solo"],
    [L(blob([("a", "1"), ("b", "2")], ver="Z0", sep=";"))],  # ';' inside Z0: FMT vs VER
    [L(wrong_ck(blob([("a", "1")], ver="Z0")))],  # CHK vs VER (order boundary)
    [L(blob([("a", "1"), ("b", "2")])), "G a"],  # control: honest Z1, both OK
)
add(
    "f2p_dup_keys",
    [L(blob([("bb", "1"), ("aa", "2"), ("bb", "3")])), "G bb", "G aa"],
    [L(blob([("k", "1"), ("z", "2"), ("m", "3"), ("k", "9")])), "G k", "G z"],
    [L(blob([("a", "1"), ("a", "2"), ("a", "3")])), "G a"],
    [L(blob([("a", "1"), ("b", "2"), ("c", "3")])), "G b"],  # control: unique, both OK
)
z0_cab = blob([("c", "1"), ("a", "2"), ("b", "3")], ver="Z0")
add(
    "f2p_alpha_rebuild",
    [L(z0_cab), "S", "P d 4", "S"],  # alpha rebuild, then insertion baseline
    [L(z0_cab), "S", "S"],  # second serialize keeps the new baseline
    [L(blob([("q", "1"), ("b", "2"), ("q", "3")])), "S"],  # dup-triggered rebuild
    [L(z0_cab), "P a 9", "S"],  # mutation before the first serialize
    [L(z0_cab), "G a", "G b", "S"],  # gets do not clear the rebuild
    [L(blob([("c", "1"), ("a", "2")])), "S"],  # control: clean Z1 keeps blob order
    [L(z0_cab), "S", L(blob([("z", "1"), ("a", "2")])), "S"],  # clean load resets
)
add(
    "f2p_val_truncation",
    ["P k abcdefghij12", "G k", "S"],  # len 12: G full, S truncated
    ["P k abcdefghij1", "S"],  # len 11 control: serialized fully
    ["P k abcdefghij", "S"],  # len 10 control
    ["P k abcdefghij12", "S", L(blob([("k", "abcdefghij")])), "G k"],  # lossy trip
    ["P a 111111111111", "P b 22", "P c 333333333333", "S", "G a", "G c"],
)
add(
    "f2p_key_case_fold",
    [L(blob([("Key", "1"), ("KEY", "2")])), "G Key", "G KEY", "G key", "S"],
    [L(blob([("aB", "x"), ("Ab", "y"), ("AB", "z")])), "G aB", "G Ab", "S"],
    [L(blob([("dog", "1"), ("DOG", "2"), ("cat", "3")])), "G dog", "G cat"],
    [L(blob([("ka", "1"), ("kb", "2")])), "G ka", "G kb"],  # control: distinct folds
    ["P Key 1", "P KEY 2", "S"],  # control: live puts stay case-sensitive
)
empty_z1 = "Z1||" + ck("Z1|")
empty_z0 = "Z0||" + ck("Z0|")
add(
    "f2p_empty_blob",
    ["P a 1", L(empty_z1), "G a", "S"],
    [L(empty_z1)],
    [L(empty_z0), "S"],  # empty legacy blob also sets the rebuild flag
    [L(blob([("a", "1")])), "G a"],  # control: one-pair blob, both OK 1
)
add(
    "f2p_load_count",
    [L(blob([("a", "1"), ("a", "2")]))],  # OK 2 vs R FMT
    [L(blob([("x", "1"), ("X", "2"), ("x", "3")])), "P c 9"],  # OK 3 then live count 2
    [L(blob([("m", "1"), ("M", "2"), ("n", "3"), ("m", "4")])), "G m", "G n"],
    [L(blob([("a", "1"), ("b", "2")]))],  # control: OK 2 in both
)
add(
    "f2p_lenient_tokens",
    ["P a 1 extra"],
    ["P a 1 2 3 4", "G a"],
    ["P a 1", "G a trailing junk"],
    ["P a 1", "G a"],  # control: exact tokens, both OK
    ["P a 1", L(blob([("a", "1")]) + " x")],  # control: L stays verbatim, FMT in both
)
trunc_blob = blob([("bb", "1"), ("aa", "abcdefghij")])  # engine's own lossy S output
alpha_blob = blob([("a", "2"), ("b", "3"), ("c", "1")])  # engine's post-Z0 S output
add(
    "f2p_roundtrip",
    # serialize with a max-length value, reload the engine's own blob
    ["P bb 1", "P aa abcdefghij12", "S", L(trunc_blob), "G aa", "G bb", "S"],
    # Z0 in, normalized Z1 out, reload it: identity from then on
    [L(z0_cab), "S", L(alpha_blob), "S", "P d 9", "S"],
    # dup blob in, serialize, reload the engine's own output
    [L(blob([("q", "5"), ("b", "2"), ("q", "7")])), "S", L(blob([("b", "2"), ("q", "7")])), "S"],
    # honest control round trip
    ["P n 1", "S", L(blob([("n", "1")])), "S", "G n"],
)

# f2p_parity: held-out mixed corpora; any missed quirk fails them
KEYS = ["a", "bb", "Key", "KEY", "key", "abcdefgh", "zz99", "m", "n", "x", "X",
        "toolong99", "a-b"]
VALS = ["1", "0", "two2", "abcdefghij", "abcdefghij1", "abcdefghij12",
        "0123456789012", "b-c", "9"]
GOOD_KEYS = ["a", "bb", "Key", "KEY", "abcdefgh", "zz99", "m", "x", "X", "n"]
GOOD_VALS = ["1", "two2", "abcdefghij", "abcdefghij1", "abcdefghij12", "9"]


def random_blob() -> str:
    ver = rng.choice(["Z1", "Z1", "Z1", "Z0", "Z0", "Z2", "z1"])
    npair = rng.randrange(0, 5)
    pairs = [(rng.choice(GOOD_KEYS + KEYS[-2:]), rng.choice(GOOD_VALS + VALS[-2:]))
             for _ in range(npair)]
    sep = None
    if rng.random() < 0.12:
        sep = rng.choice([";", ","])  # sometimes the wrong dialect
    b = blob(pairs, ver=ver, sep=sep)
    roll = rng.random()
    if roll < 0.12:
        b = wrong_ck(b)
    elif roll < 0.18:
        b = b[:-1] + rng.choice(B36)
    elif roll < 0.24:
        b = b[: rng.randrange(0, len(b))]
    elif roll < 0.30:
        b = b + rng.choice([" tail", "x", "|Y"])
    return b


def parity_batch(n: int) -> list[str]:
    batch = []
    for _ in range(n):
        roll = rng.random()
        if roll < 0.32:
            cmd = f"P {rng.choice(KEYS)} {rng.choice(VALS)}"
            if rng.random() < 0.2:
                cmd += " extra tail"
        elif roll < 0.55:
            cmd = f"G {rng.choice(KEYS)}"
            if rng.random() < 0.2:
                cmd += " junk"
        elif roll < 0.70:
            cmd = "S" if rng.random() < 0.9 else "S x"
        elif roll < 0.95:
            cmd = L(random_blob())
        else:
            cmd = rng.choice(["Q hey", "X", "p a 1", "L", "G", "P"])
        batch.append(cmd)
    return batch


add("f2p_parity", parity_batch(160), parity_batch(155))

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
                print(f"GUARD BROKEN AT BASE [{family}]: {batch[:2]}")
                gold_mismatch += 1

# big random parity sweep (not shipped): gold must match binary everywhere
sweep_fail = 0
for i in range(40):
    batch = []
    for _ in range(rng.randrange(1, 45)):
        roll = rng.random()
        if roll < 0.30:
            batch.append(f"P {rng.choice(KEYS)} {rng.choice(VALS)}"
                         + rng.choice(["", "", " t"]))
        elif roll < 0.50:
            batch.append(f"G {rng.choice(KEYS)}" + rng.choice(["", "", " j k"]))
        elif roll < 0.64:
            batch.append("S")
        elif roll < 0.94:
            b = random_blob()
            mut = rng.random()
            if mut < 0.25 and b:  # single-char corruption anywhere
                pos = rng.randrange(0, len(b))
                repl = rng.choice("abzZ019|;,=. ")
                b = b[:pos] + repl + b[pos + 1:]
            elif mut < 0.35:  # separator swap in place
                b = b.replace(";", ",") if rng.random() < 0.5 else b.replace(",", ";")
            elif mut < 0.45:  # version flip keeping the rest
                b = rng.choice(["Z0", "Z1", "Z2", "z0"]) + b[2:] if len(b) >= 2 else b
            batch.append(L(b))
        else:
            batch.append(rng.choice(["Q x", "", "L", "S s", "g a"]))
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
