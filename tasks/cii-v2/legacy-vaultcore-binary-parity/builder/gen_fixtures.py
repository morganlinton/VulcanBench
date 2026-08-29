#!/usr/bin/env python3
"""Build-time fixture generator for legacy-vaultcore-binary-parity.

Runs the compiled legacy engine over structured corner batches and a large
seeded random corpus, and:

1. verifies the gold Python implementation matches the binary EXACTLY on
   every batch (the build gate for the task itself);
2. verifies the naive base implementation diverges on every fail-to-pass
   family and agrees on every guard batch;
3. writes tests/fixtures.json: {family: [{"input": [...], "expected": [...]}]}.

Guard batches only ever digest FIRST-BYTE-NEUTRAL bodies: uppercase-letter
strings whose spec digest equals the engine digest mod 97 (the difference
is 4*b0 + 5*(b1 - b2 + b3 - ...) mod 97, searched below), kept free of
trailing digits and under 20 chars so the two migration paths and the
naive transform all coincide.

Usage: python3 builder/gen_fixtures.py <task-root>
"""

from __future__ import annotations

import importlib.util
import json
import random
import string
import subprocess
import sys
from pathlib import Path

ROOT = Path(sys.argv[1] if len(sys.argv) > 1 else ".").resolve()
BINARY = ROOT / "builder" / "vaultcore-darwin-arm64"
WEIGHTS = (4, 9)


def _load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


gold = _load("gold_vaultcore", ROOT / "builder" / "gold_vaultcore.py")
naive = _load("naive_vaultcore", ROOT / "repo" / "vaultcore.py")


def spec_digest(body: str) -> int:
    return sum(ord(c) * WEIGHTS[i % 2] for i, c in enumerate(body)) % 97


def engine_digest(body: str) -> int:
    return sum(ord(c) * WEIGHTS[i % 2] for i, c in enumerate(body[1:])) % 97


def is_neutral(body: str) -> bool:
    return spec_digest(body) == engine_digest(body)


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


# ------------------------------------------------- neutral guard bodies

rng = random.Random(20260828)


def find_neutral(n: int, min_len: int = 2, max_len: int = 19) -> list[str]:
    found: list[str] = []
    seen = set()
    while len(found) < n:
        length = rng.randrange(min_len, max_len + 1)
        body = "".join(rng.choice(string.ascii_uppercase) for _ in range(length))
        if body not in seen and is_neutral(body):
            seen.add(body)
            found.append(body)
    return found


NEUTRAL = find_neutral(14)
assert all(is_neutral(b) and b.isalpha() and b.isupper() and len(b) < 20
           for b in NEUTRAL)

# ---------------------------------------------------------------- families

families: dict[str, list[list[str]]] = {}


def add(family: str, *batches: list[str]) -> None:
    families.setdefault(family, []).extend(batches)


# guards: nominal behavior where the written spec is accurate. Guard
# territory: v2-only flows, benign migrations (neutral all-letter bodies,
# under 20 chars, never rewritten), rejects, counts, and checksums over
# neutral bodies.
n = NEUTRAL
add(
    "guard_v2_flows",
    [f"W alpha 2 {n[0]}", f"W beta 2 {n[1]}", "G alpha", "G beta", "G gamma"],
    [f"W a 2 {n[2]}", f"W a 2 {n[3]}", "G a", "C"],  # same-version update
    [f"W k1 2 {n[4]}", f"W k2 2 {n[5]}", "C", "M", "C", "G k1"],  # M is a no-op
    ["G nothere", f"W x 2 {n[6]}", "G x", "C"],
)
add(
    "guard_benign_migrations",
    # lazy == bulk == naive: all-letter neutral bodies, never rewritten
    [f"W a 1 {n[0]}", f"W b 1 {n[1]}", "G a", "M", "G a", "G b", "C"],
    [f"W p 1 {n[2]}", f"W q 2 {n[3]}", "M", "C", "G p", "G q"],
    [f"W s 1 {n[4]}", "G s", "G s", "C", "M"],  # second read migrates nothing
    [f"W m1 1 {n[5]}", f"W m2 1 {n[6]}", f"W m3 1 {n[7]}", "M", "M", "C"],
    [f"W up 1 {n[8]}", f"W up 2 {n[9]}", "G up", "C"],  # v2 overwrite of a v1 doc
)
add(
    "guard_rejects",
    ["W toolong99 1 abc"],  # doc id length 9
    ["W a-b 1 abc"],  # non-alnum doc id
    ["W a 3 abc"],  # bad version
    ["W a 0 abc"],
    ["W a 1 abcdefghijklmnopqrstu"],  # body length 21
    ["W a 1 bad-body"],  # non-alnum body
    ["W a 9 abcdefghijklmnopqrstu"],  # VER before BODY
    ["W toolong99 9 bad-body"],  # FMT before VER before BODY
    ["W a 1"],
    ["W a"],
    ["W"],
    ["G"],
    ["G bad-key"],
    ["G toolong99"],
    ["Q what is this"],
    ["X"],
    ["w a 1 abc"],
    ["g a"],
    [f"W ok 2 {n[10]}", "W ok 3 zzz", "G ok"],  # reject leaves state alone
)
add("guard_empty", [], ["C"], ["M"], ["", "C", "", "M"], ["G ghost", "C"])
add(
    "guard_trailer",
    [
        f"W alpha 2 {n[11]}",
        f"W beta 1 {n[12]}",
        "G alpha",
        "G missing",
        "M",
        "C",
        "W bad-id 1 x",
        "G",
        "W beta 4 x",
        "Q junk",
    ],
)

# f2p families: engine behavior the written spec gets wrong or omits

add(
    "f2p_sweep_vs_read",
    # same body, one doc drained by a read, the twin swept: digests split
    ["W a 1 abc7", "W b 1 abc7", "G a", "M", "G a", "G b", "C"],
    ["W a 1 report2024", "W b 1 report2024", "G a", "M", "C"],
    ["W solo 1 log42", "M", "G solo"],  # sweep-only vault
    ["W solo 1 log42", "G solo", "C"],  # read-only twin of the batch above
    ["W d 1 42424242", "M", "G d", "C"],  # all-digit body swept empty
    ["W d 1 42424242", "G d", "C"],  # lazy keeps the digits
    ["W e 1 abcdef", "G e", "M", "G e", "C"],  # control: no trailing digits
)
add(
    "f2p_rewrite_dirty",
    # a v1 rewrite flips the doc onto the bulk rule for its lazy read
    ["W d 1 x9", "W d 1 y7", "G d", "C"],
    ["W c 1 y7", "G c", "C"],  # clean twin of the rewritten doc
    ["W d 1 note1", "W d 1 note1", "G d"],  # same body rewritten: still dirty
    ["W d 1 plan88", "W d 2 plan88", "G d", "C"],  # v2 write clears the mark
    ["W d 1 a5", "W d 1 b5", "M", "G d", "C"],  # dirty doc swept: same rule
    ["W d 1 abc", "W d 1 abc", "G d", "C"],  # control: no digits, paths agree
)
add(
    "f2p_downgrade",
    # spec rejects v1-over-v2; the engine downgrades and remigrates
    ["W a 2 hello", "W a 1 world9", "G a", "C"],
    ["W a 2 hello", "W a 1 world9", "M", "G a", "C"],
    ["W a 1 first1", "G a", "W a 1 back2", "G a", "C"],  # downgrade a migrated doc
    ["W a 2 keep", "W a 1 temp3", "W a 1 temp4", "G a"],  # downgrade then dirty
    ["W a 2 x", "W a 2 y", "G a"],  # control: v2 over v2 is fine in both
)
LONG18 = "abcdefghijklmnopqr"
add(
    "f2p_maxlen",
    [f"W a 1 {LONG18}xy", "M", "G a", "C"],  # 20 chars swept: truncated to 18
    [f"W b 1 {LONG18}xy", "G b", "C"],  # 20 chars read lazily: kept
    [f"W c 1 {LONG18}x", "M", "G c"],  # 19-char control: kept on both paths
    [f"W d 1 {LONG18}x9", "M", "G d", "C"],  # strip first: 19 left, no truncation
    [f"W e 1 {LONG18}99", "M", "G e", "C"],  # strip to 18: no truncation either
    [f"W f 1 {LONG18}xyz"],  # 21 chars: BODY reject in both
    [f"W g 1 {LONG18}xy", f"W g 1 {LONG18}xy", "G g", "C"],  # dirty read truncates
    [f"W h 2 {LONG18}xy", "M", "G h", "C"],  # v2 doc: sweep never touches it
)
add(
    "f2p_digest_skew",
    ["W a 2 xbc", "W b 2 ybc", "G a", "G b"],  # first-byte twins: engine equal
    ["W a 2 Abc", "G a", "C"],
    ["W a 2 q", "G a"],  # single-char body: engine digests 0
    ["W a 2 z", "W b 2 a", "G a", "G b", "C"],
    ["W a 1 mix3", "G a", "C"],  # skew composes with a lazy migration
)
add(
    "f2p_doc_case_fold",
    ["W Doc 1 abc", "G DOC", "G doc", "C"],
    ["W KEY 2 first", "W key 2 second", "G KeY", "C"],  # one doc, not two
    ["W a 1 x7", "W A 1 y7", "G a", "C"],  # case-folded rewrite dirties
    ["W low 2 x", "W other 2 y", "G low", "G other"],  # control: distinct folds
)
add(
    "f2p_lenient_tokens",
    ["W a 2 body extra"],
    ["W a 2 body x y z", "G a junk", "C"],
    ["M now"],
    ["W a 1 x1", "M please", "G a trailing junk"],
    ["C now"],
    ["W a 2 body", "G a"],  # control: exact tokens, both OK
)
add(
    "f2p_checksum_drift",
    # the issue's symptom: checksums drift after sweeps, not after reads
    ["W r1 1 inv2020", "W r2 1 inv2021", "W r3 1 inv2022", "G r1", "G r2",
     "G r3", "C"],
    ["W r1 1 inv2020", "W r2 1 inv2021", "W r3 1 inv2022", "M", "C"],
    ["W a 1 tag5", "W b 2 tag5", "C", "M", "C", "G a", "C"],
    ["W a 1 99", "W b 1 x99", "M", "C", "G a", "G b"],  # emptied and shrunk
    ["W a 2 tagged", "W b 2 tagged", "C"],  # control: v2-only vault
)

# f2p_parity: held-out mixed corpora; any missed quirk fails them
DOCS = ["a", "bb", "Doc", "DOC", "doc", "abcdefgh", "zz99", "m", "n", "x",
        "X", "toolong99", "a-b"]
VERS = ["1", "1", "1", "2", "2", "3", "0", "x"]
BODIES = ["abc", "abc7", "x9", "y7", "report2024", "42424242", "q", "z",
          "xbc", "ybc", "mix3", "tag5", LONG18 + "xy", LONG18 + "x9",
          LONG18 + "x", "inv2021", "99", "bad-body", LONG18 + "xyz"]
GOOD_DOCS = ["a", "bb", "Doc", "DOC", "abcdefgh", "zz99", "m", "x", "X", "n"]


def parity_batch(count: int) -> list[str]:
    batch = []
    for _ in range(count):
        roll = rng.random()
        if roll < 0.40:
            cmd = f"W {rng.choice(DOCS)} {rng.choice(VERS)} {rng.choice(BODIES)}"
            if rng.random() < 0.15:
                cmd += " extra tail"
        elif roll < 0.70:
            cmd = f"G {rng.choice(DOCS)}"
            if rng.random() < 0.15:
                cmd += " junk"
        elif roll < 0.82:
            cmd = "M" if rng.random() < 0.8 else "M now"
        elif roll < 0.94:
            cmd = "C" if rng.random() < 0.8 else "C now"
        else:
            cmd = rng.choice(["Q hey", "X", "w a 1 x", "G", "W", "W a", "W a 1"])
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
        if roll < 0.42:
            batch.append(f"W {rng.choice(DOCS)} {rng.choice(VERS)} "
                         f"{rng.choice(BODIES)}" + rng.choice(["", "", " t"]))
        elif roll < 0.70:
            batch.append(f"G {rng.choice(DOCS)}" + rng.choice(["", "", " j k"]))
        elif roll < 0.82:
            batch.append(rng.choice(["M", "M", "M x"]))
        elif roll < 0.94:
            batch.append(rng.choice(["C", "C", "C x"]))
        else:
            batch.append(rng.choice(["Q x", "", "W", "G", "W a", "g a", "m", "c"]))
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
