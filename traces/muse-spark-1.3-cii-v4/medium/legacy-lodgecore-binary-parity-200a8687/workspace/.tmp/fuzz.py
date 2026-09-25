import random, subprocess, sys
ENG="./legacy/lodgecore-darwin-arm64"
def run(argv, sess):
    p = subprocess.run(argv, input=sess, capture_output=True, text=True)
    return p.stdout
def gen(rng):
    pids = ["a","b","c","d","e","f","g","A1","Z9","q"]
    rids = ["r1","r2","r3","r4"]
    cmds = []
    n = rng.randint(5, 60)
    for _ in range(n):
        t = rng.random()
        if t < 0.10:
            cmds.append(f"P {rng.choice(pids)} {rng.randint(1,8)}")
        elif t < 0.20:
            cmds.append(f"O {rng.choice(rids)} {rng.randint(1,24)}")
        elif t < 0.35:
            cmds.append(f"B {rng.choice(pids)} {rng.choice(rids)}")
        elif t < 0.50:
            cmds.append(f"A {rng.choice(pids)}")
        elif t < 0.60:
            cmds.append(f"{rng.choice(['G','E','L'])} {rng.choice(pids)}")
        elif t < 0.70:
            cmds.append("W")
        elif t < 0.78:
            cmds.append("S")
        elif t < 0.86:
            cmds.append(f"Q {rng.choice(rids)}")
        elif t < 0.90:
            cmds.append("V")
        else:
            cmds.append(rng.choice([
                "", "P", "P a", "P a 2 x", "P a 0", "P a 9", "O r 0", "O r 25",
                "B", "A", "Q", "V x", "W x", "S x", "X", "p a 2",
                "P toolongid9 2", "O r 007", "B ghost r", "Q ghost", "G ghost",
                "P a 02", "O r1 5",
            ]))
    return "\n".join(cmds) + "\n"
def main():
    seed0 = int(sys.argv[1]) if len(sys.argv)>1 else 0
    count = int(sys.argv[2]) if len(sys.argv)>2 else 300
    bad = 0
    for i in range(count):
        rng = random.Random(seed0 + i)
        sess = gen(rng)
        l = run([ENG], sess)
        p = run(["python3","lodgecore.py"], sess)
        if l != p:
            bad += 1
            print(f"=== MISMATCH seed={seed0+i} ===")
            print("--- session ---"); print(sess)
            print("--- legacy ---"); print(l)
            print("--- python ---"); print(p)
            if bad >= 3:
                break
    print(f"done: {count} sessions, {bad} mismatches")
main()
