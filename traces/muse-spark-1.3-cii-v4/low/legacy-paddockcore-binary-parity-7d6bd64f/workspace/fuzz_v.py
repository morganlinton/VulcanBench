"""Fuzz the legacy engine for V/U ordering samples."""
import random
import subprocess
import sys

run = ["legacy/run"]

def gen_session(rng):
    npony = rng.randint(2, 5)
    nfield = rng.randint(2, 3)
    names = ["a", "b", "c", "d", "e"][:npony]
    flds = ["f1", "f2", "f3"][:nfield]
    cmds = []
    for i, p in enumerate(names):
        cmds.append(f"N {p} {rng.choice([10, 10, 10, 20, 5])}")
    for f in flds:
        cmds.append(f"F {f} {rng.choice([1, 2, 3, 9])}")
    # random turnouts
    for p in names:
        if rng.random() < 0.9:
            cmds.append(f"G {p} {rng.choice(flds)}")
            if rng.random() < 0.5:
                cmds.append(f"H {p}")
                if rng.random() < 0.7:
                    cmds.append(f"G {p} {rng.choice(flds)}")
    # extra G/H/L cycles to build counts
    for _ in range(rng.randint(0, 6)):
        p = rng.choice(names)
        op = rng.random()
        if op < 0.4:
            cmds.append(f"H {p}")
        elif op < 0.7:
            cmds.append(f"G {p} {rng.choice(flds)}")
        else:
            cmds.append(f"L {p} {rng.choice(flds)}")
    for _ in range(rng.randint(1, 3)):
        cmds.append("S")
        if rng.random() < 0.3:
            p = rng.choice(names)
            cmds.append(f"{rng.choice(['G','H'])} {p}" + (f" {rng.choice(flds)}" if rng.random() < 0.7 else ""))
    cmds.append("V")
    if rng.random() < 0.5:
        cmds.append("U")
    return cmds

def main():
    seed = int(sys.argv[1]) if len(sys.argv) > 1 else 1
    n = int(sys.argv[2]) if len(sys.argv) > 2 else 40
    rng = random.Random(seed)
    out = []
    for i in range(n):
        cmds = gen_session(rng)
        p = subprocess.run(run, input="\n".join(cmds) + "\n",
                           capture_output=True, text=True)
        out.append("### CMDS %d" % i)
        out.extend(cmds)
        out.append("### OUT %d" % i)
        out.extend(p.stdout.splitlines())
    sys.stdout.write("\n".join(out) + "\n")

if __name__ == "__main__":
    main()
