"""Collect all V/U observations and fit (led, g*, pseq) models."""
import os
import pickle
import subprocess
import sys

sys.path.insert(0, ".")
from fuzz_fit import parse, replay

TMP = os.environ["TMPDIR"]
obs = []
for path in [f"{TMP}/fz11.txt", f"{TMP}/ft2.txt"]:
    for cmds, outs in parse(path):
        try:
            obs.extend(replay(cmds, outs))
        except AssertionError:
            pass
hard = ["wc2", "ex2", "pp", "gs", "ws", "ses6", "gl", "la", "ph",
        "l2", "av", "av2", "rf", "rf2", "ma2", "ub2"]
for h in hard:
    try:
        with open(f"{TMP}/{h}.txt") as fh:
            cmds = fh.read().splitlines()
    except OSError:
        continue
    p = subprocess.run(["legacy/run"], input="\n".join(cmds) + "\n",
                       capture_output=True, text=True)
    try:
        obs.extend(replay(cmds, p.stdout.splitlines()))
    except AssertionError as e:
        print("hardskip", h, e)
print("total obs:", len(obs))
pickle.dump(obs, open(f"{TMP}/allobs.pkl", "wb"))

# per-pony: build,g,h,l,pseq. candidate g*: G-H, G-H+reloc, G, G+reloc.
# led: boolean l>0, or count l.
VOBS = [o for o in obs if o[0] == "V"]
print("V obs:", len(VOBS))


def gstar(a, mode):
    g, h, r = a["g"], a["h"], a["reloc"]
    if mode == "G-H":
        return g - h
    if mode == "G-H+R":
        return g - h + r
    if mode == "G":
        return g
    if mode == "G+R":
        return g + r
    raise AssertionError


def fit(mode, ledfmt):
    bad = 0
    for kind, names, attrs in VOBS:
        def key(p):
            a = attrs[p]
            led = (1 if a["l"] > 0 else 0) if ledfmt == "bool" else a["l"]
            return (-a["build"], -led, gstar(a, mode), a["pseq"])
        if sorted(names, key=key) != names:
            bad += 1
    return bad


for mode in ("G-H", "G-H+R", "G", "G+R"):
    for ledfmt in ("bool", "count"):
        print(mode, ledfmt, "bad=", fit(mode, ledfmt))
