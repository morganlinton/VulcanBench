import random, subprocess, sys
ENG="./legacy/lodgecore-darwin-arm64"
def run(argv, sess):
    p = subprocess.run(argv, input=sess, capture_output=True, text=True)
    return p.stdout
def gen(rng, mode):
    pids = ["a","b","c","d","e","f","g","h","A1","Z9","q","w","x","y","z"]
    if mode == "tiny":
        rids = ["r1","r2"]; pids = ["a","b","c"]
        sizes = [1,2,3]; bunks = [1,2,3,4,5]
    elif mode == "hist":
        rids = ["r1","r2","r3"]; sizes = [1,2,3,4]; bunks = [6,10,24]
    elif mode == "rota":
        rids = ["r1","r2","r3","r4","r5"]; sizes = [1,2,4,8]; bunks = [4,8,12]
    else:
        rids = ["r1","r2","r3","r4"]; sizes = list(range(1,9)); bunks = list(range(1,25))
    cmds = []
    n = rng.randint(20, 150)
    for _ in range(n):
        t = rng.random()
        if mode == "rota":
            w = [0.05,0.10,0.25,0.40,0.50,0.75,0.85,0.92,0.95]
        elif mode == "hist":
            w = [0.08,0.14,0.30,0.48,0.66,0.70,0.76,0.84,0.88]
        elif mode == "tiny":
            w = [0.08,0.16,0.34,0.52,0.64,0.76,0.84,0.90,0.93]
        else:
            w = [0.10,0.20,0.35,0.50,0.60,0.70,0.78,0.86,0.90]
        if t < w[0]:
            cmds.append(f"P {rng.choice(pids)} {rng.choice(sizes)}")
        elif t < w[1]:
            cmds.append(f"O {rng.choice(rids)} {rng.choice(bunks)}")
        elif t < w[2]:
            cmds.append(f"B {rng.choice(pids)} {rng.choice(rids)}")
        elif t < w[3]:
            cmds.append(f"A {rng.choice(pids)}")
        elif t < w[4]:
            cmds.append(f"{rng.choice(['G','E','L'])} {rng.choice(pids)}")
        elif t < w[5]:
            cmds.append("W")
        elif t < w[6]:
            cmds.append("S")
        elif t < w[7]:
            cmds.append(f"Q {rng.choice(rids)}")
        elif t < w[8]:
            cmds.append("V")
        else:
            cmds.append(rng.choice(["", "P", "P a", "P a 2 x", "P a 0", "P a 9",
                "O r 0", "O r 25", "B", "A", "Q", "V x", "W x", "S x", "X",
                "P toolongid9 2", "O r 007", "B ghost r", "Q ghost", "G ghost",
                "P a 02", "E ghost", "L ghost", "A ghost"]))
    return "\n".join(cmds) + "\n"
def main():
    seed0 = int(sys.argv[1]); count = int(sys.argv[2]); modes = sys.argv[3:]
    bad = 0; total = 0
    for i in range(count):
        mode = modes[i % len(modes)]
        rng = random.Random(seed0 + i*7919)
        sess = gen(rng, mode)
        l = run([ENG], sess); p = run(["python3","lodgecore.py"], sess)
        total += 1
        if l != p:
            bad += 1
            print(f"=== MISMATCH seed={seed0+i*7919} mode={mode} ===")
            print("--- session ---"); print(sess)
            print("--- legacy ---"); print(l)
            print("--- python ---"); print(p)
            if bad >= 3:
                break
    print(f"done: {total} sessions, {bad} mismatches")
main()
