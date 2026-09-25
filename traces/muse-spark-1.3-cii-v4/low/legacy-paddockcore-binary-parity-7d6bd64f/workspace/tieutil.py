def replay_fields(cmds, outs):
    """Replay tracking where/reloc/led/affret. Returns (V obs with context)."""
    where = {}
    g = {}
    h = {}
    l = {}
    reloc = {}
    affret = {}
    pseq = {}
    ever = {}
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
            is_aff = False
            while True:
                ln = out[oi]; oi += 1
                if ln.startswith("WAKE"):
                    is_aff = True  # subsequent PUTs before REST are returns
                elif ln.startswith("REST"):
                    is_aff = False
                    ever[ln.split()[1]] = True
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
                           aff=affret[p], pseq=pseq[p], fld=where[p],
                           ev=ever.get(where[p], False) if where[p] else None)
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


