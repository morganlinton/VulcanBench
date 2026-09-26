"""Scan for full-ties (build,ledflag,fresh,g,ever tied) with H differing."""
import os
import sys

sys.path.insert(0, ".")
from fuzz_fit import parse
from tieutil import replay_fields
import subprocess

TMP = os.environ["TMPDIR"]


def replay2(cmds, outs):
    """Extended ctx: shift idx, ledflag (L at shift 0), lastG idx, reloc."""
    where = {}
    g = {}
    h = {}
    l = {}
    reloc = {}
    pseq = {}
    ever = {}
    ledflag = {}
    lastG = {}
    shift = 0
    out = list(outs)
    oi = 0
    obs = []

    def take_prefix(prefix):
        nonlocal oi
        assert out[oi].startswith(prefix), f"desync {prefix!r} vs {out[oi]!r}"
        v = out[oi][len(prefix):]
        oi += 1
        return v

    for c in cmds:
        t = c.split()
        if t[0] == "N":
            ln = out[oi]; oi += 1
            if ln.startswith("OK"):
                where[t[1]] = None
                g[t[1]] = 0; h[t[1]] = 0; l[t[1]] = 0
                reloc[t[1]] = 0
                pseq[t[1]] = len(pseq)
                ledflag[t[1]] = False
                lastG[t[1]] = -1
        elif t[0] == "F":
            oi += 1
        elif t[0] in ("G", "H", "L"):
            ln = out[oi]; oi += 1
            if ln == "OK":
                if t[0] == "G":
                    where[t[1]] = t[2]; g[t[1]] += 1
                    lastG[t[1]] = shift
                elif t[0] == "H":
                    where[t[1]] = None; h[t[1]] += 1
                else:
                    where[t[1]] = t[2]; l[t[1]] += 1
                    if shift == 0:
                        ledflag[t[1]] = True
        elif t[0] == "R":
            ln = out[oi]; oi += 1
            oi += int(ln.split()[2])
        elif t[0] == "S":
            shift += 1
            while True:
                ln = out[oi]; oi += 1
                if ln.startswith("REST"):
                    ever[ln.split()[1]] = True
                elif ln.startswith("PUT"):
                    _, p, f = ln.split()
                    where[p] = f
                    reloc[p] += 1
                elif ln.startswith("BARN"):
                    _, p = ln.split()
                    where[p] = None
                    reloc[p] += 1
                elif ln.startswith("SHIFT"):
                    oi += int(ln.split()[1])
                    break
        elif t[0] == "V":
            k = int(take_prefix("OUT "))
            names = [take_prefix("P ") for _ in range(k)]
            ctx = {p: dict(g=g[p], h=h[p], l=l[p], reloc=reloc[p],
                           pseq=pseq[p], fld=where[p],
                           ev=ever.get(where[p], False),
                           led=ledflag[p],
                           fresh=(lastG[p] == shift and shift >= 1
                                  and reloc[p] > 0))
                   for p in names}
            obs.append((names, ctx))
        elif t[0] == "U":
            k = int(take_prefix("MUSTER "))
            oi += k
    return obs


VOBS = []
for path in (f"{TMP}/fz11.txt", f"{TMP}/ft2.txt"):
    for cmds, outs in parse(path):
        try:
            for n, c in replay2(cmds, outs):
                VOBS.append((cmds, n, c))
        except AssertionError:
            pass
print("V obs:", len(VOBS))
# find full-ties with H differing
for cmds, names, ctx in VOBS:
    builds = {}
    for c in cmds:
        t = c.split()
        if t[0] == "N":
            builds[t[1]] = int(t[2])
    j = 0
    while j < len(names):
        k = j
        while (k + 1 < len(names)
               and builds[names[k + 1]] == builds[names[j]]
               and all(ctx[names[k + 1]][a] == ctx[names[j]][a]
                       for a in ("led", "fresh", "g", "ev"))):
            k += 1
        if k > j:
            grp = names[j:k + 1]
            hs = [ctx[p]["h"] for p in grp]
            ps = [ctx[p]["pseq"] for p in grp]
            asc = all(a < b for a, b in zip(ps, ps[1:]))
            if len(set(hs)) > 1:
                print(("ASC " if asc else "DESC"), grp,
                      [(p, ctx[p]) for p in grp])
                print("   ", cmds)
        j = k + 1
print("done")
