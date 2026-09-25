"""Compare wc2's d/c tie against fuzz ties with field/reloc context."""
import os
import sys

sys.path.insert(0, ".")
from fuzz_fit import parse, replay

TMP = os.environ["TMPDIR"]


def replay_fields(cmds, outs):
    """Replay tracking where/reloc/led/affret. Returns (V obs with context)."""
    where = {}
    g = {}
    h = {}
    l = {}
    reloc = {}
    affret = {}
    pseq = {}
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
                reloc[t[1]] = 0; affret[t[1]] = 0
                pseq[t[1]] = len(pseq)
        elif t[0] in ("F",):
            oi += 1
        elif t[0] in ("G", "H", "L"):
            ln = out[oi]; oi += 1
            if ln == "OK":
                if t[0] == "G":
                    where[t[1]] = t[2]; g[t[1]] += 1
                elif t[0] == "H":
                    where[t[1]] = None; h[t[1]] += 1
                else:
                    where[t[1]] = t[2]; l[t[1]] += 1
        elif t[0] == "R":
            ln = out[oi]; oi += 1
            k = int(ln.split()[2])
            oi += k
        elif t[0] == "S":
            oi += 1
            is_aff = False
            while True:
                ln = out[oi]; oi += 1
                if ln.startswith("WAKE"):
                    is_aff = True  # subsequent PUTs before REST are returns
                elif ln.startswith("REST"):
                    is_aff = False
                elif ln.startswith("PUT"):
                    _, p, f = ln.split()
                    where[p] = f
                    reloc[p] += 1
                    if is_aff:
                        affret[p] += 1
                elif ln.startswith("BARN"):
                    _, p = ln.split()
                    where[p] = None
                    reloc[p] += 1
                elif ln.startswith("SHIFT"):
                    k = int(ln.split()[1])
                    oi += k
                    break
        elif t[0] == "V":
            k = int(take_prefix("OUT "))
            names = [take_prefix("P ") for _ in range(k)]
            ctx = {p: dict(g=g[p], h=h[p], l=l[p], reloc=reloc[p],
                           aff=affret[p], pseq=pseq[p], fld=where[p])
                   for p in names}
            obs.append((names, ctx))
        elif t[0] == "U":
            k = int(take_prefix("MUSTER "))
            oi += k
    return obs


def show(title, names, ctx):
    print(f"--- {title}: V={names}")
    for p in names:
        print("   ", p, ctx[p])


# wc2
with open(f"{TMP}/wc2.txt") as fh:
    cmds = fh.read().splitlines()
import subprocess
p = subprocess.run(["legacy/run"], input="\n".join(cmds) + "\n",
                   capture_output=True, text=True)
for names, ctx in replay_fields(cmds, p.stdout.splitlines()):
    show("wc2", names, ctx)

# fuzz: find V with a cross-field g-tie ordered DESC-pseq
n = 0
for path in (f"{TMP}/fz11.txt", f"{TMP}/ft2.txt"):
    for i, (cmds, outs) in enumerate(parse(path)):
        try:
            ob = replay_fields(cmds, outs)
        except AssertionError:
            continue
        for names, ctx in ob:
            # find adjacent pair in V with same g, different field, DESC pseq
            for a, b in zip(names, names[1:]):
                A, B = ctx[a], ctx[b]
                if A["g"] == B["g"] and A["fld"] != B["fld"] \
                        and A["pseq"] > B["pseq"] and A["l"] == 0 and B["l"] == 0:
                    print(f"=== DESC-tie {path}#{i}: {a}{A} before {b}{B}")
                    print("   cmds:", cmds)
                    n += 1
                    if n > 6:
                        break
            if n > 6:
                break
        if n > 6:
            break
print("done")
