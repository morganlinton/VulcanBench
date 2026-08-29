#!/usr/bin/env python3
"""Build-time fixture generator for legacy-tallycore-binary-parity.

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
BINARY = ROOT / "builder" / "tallycore-darwin-arm64"


def _load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


gold = _load("gold_tallycore", ROOT / "builder" / "gold_tallycore.py")
naive = _load("naive_tallycore", ROOT / "repo" / "tallycore.py")


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


def C(*ids: str) -> list[str]:
    return [f"C {i}" for i in ids]


def V(*ranks: str) -> list[str]:
    return [f"V {r}" for r in ranks]


# ---------------------------------------------------------------- families

rng = random.Random(20260829)
families: dict[str, list[list[str]]] = {}


def add(family: str, *batches: list[str]) -> None:
    families.setdefault(family, []).extend(batches)


# Guard territory: single recount rounds (round 1 has zero carryover),
# every ballot leaves at least one candidate unranked (no completeness
# bonus), distinct first-choice counts at every elimination step (no
# ties, so the tie sort key never engages), no two-candidate ballot whose
# first choice is eliminated, exact tokens, consistent case, known
# candidates only.
add(
    "guard_registration",
    ["C ax", "C bx", "C cx", "C ax", "C dx"],  # re-register is a no-op in both
    ["C a1", "V a1", "C b1", "W", "C b1", "V b1,a1", "C c1"],  # LATE lane both
    ["C k", "C m", "C k", "V k", "C m"],
)
add(
    "guard_single_round",
    # 3 candidates, distinct counts everywhere, no full rankings
    C("ax", "bx", "cx") + V("ax", "ax", "ax,bx", "bx,ax", "bx", "cx") + ["W"],
    # leader hand-off along the way, still no ties at any step
    C("p1", "q1", "r1") + V("q1", "p1,q1", "p1", "p1", "q1,r1", "q1", "q1") + ["W"],
    # transfers that behave identically: 3-entry ballots transfer in both
    C("aa", "bb", "cc", "dd")
    + V("aa,bb,cc", "aa", "aa,cc", "bb,aa,dd", "bb,cc,aa", "cc,bb,aa", "cc", "cc,aa", "dd,cc,aa")
    + ["W"],
    # single candidate: WIN with no ELIM
    ["C solo", "V solo", "W"],
    # W with no ballots rejects in both; W with args rejects in both
    ["C ax", "W", "V ax", "W x", "W"],
)
add(
    "guard_two_rounds_low_tallies",
    # two rounds, but every final tally stays under 4: carryover floors
    # to zero and the naive matches anyway
    C("ax", "bx", "cx") + V("ax", "ax,bx", "bx", "bx,cx", "bx") + ["W"]
    + V("cx", "cx,ax", "cx", "ax,cx") + ["W"],
)
add(
    "guard_rejects",
    ["C toolong99"],  # id length 9
    ["C a-b"],
    ["C"],
    ["V"],
    ["C ax", "V "],  # note: trailing space strips to bare V
    ["C ax", "V ax,"],  # empty entry
    ["C ax", "V ,ax"],
    ["C ax", "V ax,,bx"],
    ["C ax", "V toolong99,ax"],  # 9-char entry: FMT before CAND in both
    ["C ax", "V a-b"],
    ["C ax", "C bx", "V ax,bx,ax"],  # DUP in both
    ["C ax", "V ax,ax"],
    ["W"],  # no ballots
    ["C ax", "V ax", "W junk"],
    ["Q what"],
    ["X"],
    ["c ax"],
    ["v ax"],
    ["w"],
    ["C ax", "V ax", "C bx"],  # LATE in both
)
add("guard_empty", [], ["C ax"], ["", "C ax", "", "V ax"])
add(
    "guard_trailer",
    ["C ax", "C bx", "V ax", "V bx", "W", "V ax", "C toolong99", "V cx",
     "V ax,ax", "W", "Q junk", "W"],
)

# f2p families: engine behavior the written spec gets wrong or omits

add(
    "f2p_momentum",
    # Round 1 gives ax a big final tally; round 2 is arranged so the
    # naive (no carryover) eliminates differently than the engine.
    C("ax", "bx", "cx")
    + V("ax", "ax", "ax", "ax", "ax", "ax,bx", "bx,ax", "bx", "cx,bx", "cx")
    + ["W"]
    + V("bx", "bx", "cx", "cx", "ax,cx", "bx,cx")
    + ["W"],
    # Three rounds: momentum compounds across W resets.
    C("m1", "n1", "p1")
    + V("m1", "m1", "m1", "m1", "n1,m1", "n1", "p1") + ["W"]
    + V("n1", "n1", "n1", "p1,n1", "p1", "m1") + ["W"]
    + V("p1", "p1", "m1,p1", "n1") + ["W"],
    # Carryover reaches a candidate eliminated in round 1 as well.
    C("q", "r", "s")
    + V("q", "q", "q", "q", "r", "r", "r", "r", "r,q", "s") + ["W"]
    + V("s", "s", "q,s", "r") + ["W"],
    # Control: second round with all prior finals under 4, both agree.
    C("u", "v")
    + V("u", "u,v", "v") + ["W"] + V("v", "u") + ["W"],
    # A candidate registered between rounds starts with zero carryover.
    C("aa", "bb")
    + V("aa", "aa", "aa", "aa", "bb,aa", "bb") + ["W"]
    + ["C cc"] + V("cc", "cc", "bb", "aa,cc") + ["W"],
)
add(
    "f2p_longid_tiebreak",
    # tie at the bottom between a short and a long id: engine eliminates
    # the long one first, naive the last registered
    C("longname", "z") + V("longname", "z") + ["W"],
    C("a", "bcdefgh", "m")
    + V("a,m", "a", "bcdefgh,a", "bcdefgh", "m,a", "m", "m,bcdefgh") + ["W"],
    # long id registered EARLY so last-registered and longest disagree
    C("verylong", "aa", "bb")
    + V("verylong,aa", "aa", "bb,aa", "verylong", "aa,bb", "bb") + ["W"],
    # control: equal-length ids, tie falls back to last-registered in both
    C("aa", "bb", "cc") + V("aa,bb", "bb", "cc,bb", "aa") + ["W"],
)
add(
    "f2p_full_rank_bonus",
    # every candidate ranked: 2x on the first choice flips the recount
    C("ax", "bx", "cx")
    + V("ax,bx,cx", "bx", "bx,ax", "cx,bx", "cx") + ["W"],
    # bonus only while on the first choice: after transfer, weight 1
    C("p", "q", "r")
    + V("p,q,r", "q,r", "q", "r,p", "r,q,p", "r") + ["W"],
    # LEAD is unweighted in both (documented), then W diverges
    C("k1", "k2", "k3")
    + V("k1,k2,k3", "k2,k1", "k2", "k1,k3,k2", "k3,k2") + ["W"],
    # control: no ballot ranks everyone, no bonus anywhere
    C("d", "e", "f") + V("d,e", "d", "e", "e,f", "f,d") + ["W"],
)
add(
    "f2p_two_cand_exhaustion",
    # a 2-entry ballot whose first choice is eliminated: engine discards,
    # naive transfers, and the transfer decides the winner
    C("ax", "bx", "cx")
    + V("ax,bx", "ax,cx,bx", "bx", "bx,cx", "cx", "cx,ax", "bx,ax,cx") + ["W"],
    # 3-entry ballots transfer in both; only the 2-entry one differs
    C("m", "n", "p")
    + V("m,n", "m,n,p", "n", "n,p", "p", "p,n", "p,m") + ["W"],
    # 2-entry ballot whose SECOND choice is eliminated first: both keep it
    C("q", "r", "s")
    + V("q,s", "q", "q,r", "r", "r,q", "s,r", "r,s") + ["W"],
)
add(
    "f2p_unknown_entry",
    # unknown entry mid-ranking: engine drops it and accepts, naive CAND
    ["C ax", "C bx", "V ax,zz,bx", "V bx", "W"],
    ["C ax", "V zz,ax"],  # unknown FIRST entry: engine counts ax
    ["C ax", "C bx", "V ax,bx", "V bx,qq", "V bx", "W"],
    ["C ax", "V zz,yy"],  # nothing survives: CAND in both, same echo
    ["C ax", "C bx", "V ax,zz,ax"],  # engine: DUP on 3rd; naive: CAND on 2nd
    ["C ax", "C bx", "V ax,bx", "W"],  # control: all known, both agree
)
add(
    "f2p_case_fold",
    # case-variant registration collapses in the engine (count differs)
    ["C Alpha", "C ALPHA", "C beta", "V Alpha,beta", "W"],
    # ranking in the wrong case: engine folds, naive rejects CAND
    ["C Alpha", "C beta", "V ALPHA,beta", "V beta", "W"],
    # folded duplicate inside one ranking: engine DUP, naive CAND
    ["C Alpha", "V Alpha,ALPHA"],
    # first-seen spelling is reported by LEAD/ELIM/WIN
    ["C Kilo", "C juli", "V KILO", "V KILO,juli", "V juli", "W"],
    # control: consistent case everywhere, both agree
    ["C kilo", "C juli", "V kilo", "V juli,kilo", "V kilo", "W"],
)
add(
    "f2p_lenient_tokens",
    ["C ax extra", "V ax"],
    ["C ax", "V ax tail junk", "W"],
    ["C ax", "C bx trailing", "V bx,ax more", "W"],
    ["C ax", "V ax", "W"],  # control: exact tokens, both agree
    ["C ax", "V ax", "W x"],  # control: W stays strict in both
)

# f2p_parity: held-out mixed corpora; any missed quirk fails them
IDS = ["ax", "bx", "cx", "Alpha", "ALPHA", "beta", "longname", "z", "m",
       "verylong", "kilo", "KILO", "juli"]
BAD = ["toolong99", "a-b", "", "zz", "yy"]


def random_ranking(pool: list[str]) -> str:
    k = rng.randrange(1, min(len(pool), 5) + 1)
    picks = rng.sample(pool, k)
    if rng.random() < 0.18:
        picks.insert(rng.randrange(len(picks) + 1), rng.choice(IDS + BAD))
    return ",".join(picks)


def parity_batch(n: int) -> list[str]:
    batch: list[str] = []
    pool = rng.sample(IDS, rng.randrange(3, 7))
    for cid in pool:
        batch.append(f"C {cid}")
    ballots_in_round = 0
    for _ in range(n):
        roll = rng.random()
        if roll < 0.10:
            cid = rng.choice(IDS + BAD[:2])
            batch.append(f"C {cid}" + ("" if rng.random() < 0.8 else " tail"))
            if ballots_in_round == 0 and cid not in BAD and cid not in pool:
                pool.append(cid)
        elif roll < 0.72:
            batch.append(f"V {random_ranking(pool)}"
                         + ("" if rng.random() < 0.85 else " junk"))
            ballots_in_round += 1  # close enough: some are rejected
        elif roll < 0.88:
            batch.append("W")
            ballots_in_round = 0
        else:
            batch.append(rng.choice(["Q hey", "X", "c ax", "V", "C", "W x"]))
    batch.append("W")
    return batch


add("f2p_parity", parity_batch(150), parity_batch(145))

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
    batch = parity_batch(rng.randrange(5, 60))
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
