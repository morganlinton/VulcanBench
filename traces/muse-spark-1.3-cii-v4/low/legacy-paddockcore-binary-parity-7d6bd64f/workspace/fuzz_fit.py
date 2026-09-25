"""Fit V/U ordering: replay engine-observed moves, search sort-key space."""
import itertools
import sys

def parse(path):
    samples = []
    cmds, outs, mode = [], [], None
    with open(path) as fh:
        for line in fh:
            line = line.rstrip("\n")
            if line.startswith("### CMDS"):
                if cmds or outs:
                    samples.append((cmds, outs))
                cmds, outs, mode = [], [], "c"
            elif line.startswith("### OUT"):
                mode = "o"
            elif mode == "c":
                cmds.append(line)
            else:
                outs.append(line)
    if cmds or outs:
        samples.append((cmds, outs))
    return samples

def replay(cmds, outs):
    """Replay using engine-observed moves. Returns list of V/U observations.
    Each obs: (kind, ordered_names, attr dict)."""
    build = {}
    pseq = {}
    where = {}
    g = {}
    h = {}
    l = {}
    reloc = {}   # PUT count (incl affinity returns)
    barned = {}  # ever BARNed by shift
    out = list(outs)
    oi = 0
    obs = []

    def emit(line):
        nonlocal oi
        assert out[oi] == line, f"desync: want {line!r} got {out[oi]!r} cmds={cmds}"
        oi += 1

    def take_prefix(prefix):
        nonlocal oi
        assert out[oi].startswith(prefix), f"desync want prefix {prefix!r} got {out[oi]!r}"
        v = out[oi][len(prefix):]
        oi += 1
        return v

    for c in cmds:
        t = c.split()
        if t[0] == "N":
            ln = out[oi]; oi += 1
            if ln.startswith("OK"):
                build[t[1]] = int(t[2])
                pseq[t[1]] = len(pseq)
                where[t[1]] = None
                g[t[1]] = 0; h[t[1]] = 0; l[t[1]] = 0
                reloc[t[1]] = 0; barned[t[1]] = 0
        elif t[0] == "F":
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
        elif t[0] == "S":
            # consume WAKE/REST/PUT/BARN/SHIFT/F lines (S has no own line)
            while True:
                ln = out[oi]; oi += 1
                if ln.startswith("WAKE"):
                    pass
                elif ln.startswith("REST"):
                    pass
                elif ln.startswith("PUT"):
                    _, p, f = ln.split()
                    where[p] = f
                    reloc[p] += 1
                elif ln.startswith("BARN"):
                    _, p = ln.split()
                    where[p] = None
                    reloc[p] += 1
                    barned[p] += 1
                elif ln.startswith("SHIFT"):
                    k = int(ln.split()[1])
                    for _ in range(k):
                        assert out[oi].startswith("F "), out[oi]
                        oi += 1
                    break
                else:
                    raise AssertionError(f"unexpected S line {ln!r}")
        elif t[0] == "V":
            k = int(take_prefix("OUT "))
            names = []
            for _ in range(k):
                names.append(take_prefix("P "))
            attrs = {}
            for p in names:
                attrs[p] = dict(build=build[p], pseq=pseq[p], g=g[p],
                               h=h[p], l=l[p], reloc=reloc[p],
                               barned=barned[p], out=1)
            obs.append(("V", names, attrs))
        elif t[0] == "R":
            ln = out[oi]; oi += 1
            assert ln.startswith("FLD "), ln
            k = int(ln.split()[2])
            for _ in range(k):
                assert out[oi].startswith("P "), out[oi]
                oi += 1
        elif t[0] == "U":
            k = int(take_prefix("MUSTER "))
            names = []
            for _ in range(k):
                names.append(take_prefix("P "))
            attrs = {}
            for p in names:
                attrs[p] = dict(build=build[p], pseq=pseq[p], g=g[p],
                               h=h[p], l=l[p], reloc=reloc[p],
                               barned=barned[p], out=1 if where[p] else 0)
            obs.append(("U", names, attrs))
        else:
            raise AssertionError(t)
    return obs

KEYS = ["build", "pseq", "g", "h", "l", "reloc", "barned", "out"]

def check(obs, key):
    """key: tuple of (attr, sign). True if sort matches all obs of that kind."""
    for kind, names, attrs in obs:
        if kind != check.kind:
            continue
        ranked = sorted(names, key=lambda p: tuple(
            s * attrs[p][a] for a, s in key))
        if ranked != names:
            return False
    return True

def main():
    path = sys.argv[1]
    samples = parse(path)
    obs = []
    for cmds, outs in samples:
        try:
            obs.extend(replay(cmds, outs))
        except AssertionError as e:
            print("SKIP sample:", e)
    print(f"samples={len(samples)} obs={len(obs)}")
    for kind in ("V", "U"):
        check.kind = kind
        nob = sum(1 for o in obs if o[0] == kind)
        print(f"== {kind} obs={nob}")
        # try 1..3 key tuples
        cands = [(a, s) for a in KEYS for s in (1, -1)]
        found = []
        for r in (1, 2, 3, 4):
            for key in itertools.product(cands, repeat=r):
                attrs_used = [a for a, s in key]
                if len(set(attrs_used)) != len(attrs_used):
                    continue
                if check(obs, key):
                    found.append(key)
                    print("  MATCH", key)
                    if len(found) > 15:
                        break
            if len(found) > 15:
                break
        if not found:
            print("  no key tuple up to 3")

if __name__ == "__main__":
    main()
