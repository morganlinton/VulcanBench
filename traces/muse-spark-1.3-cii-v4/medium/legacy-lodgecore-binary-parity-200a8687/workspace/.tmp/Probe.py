import subprocess
ENG="./legacy/lodgecore-darwin-arm64"
PY="python3 lodgecore.py"
def run_legacy(sess):
    p = subprocess.run([ENG], input=sess, capture_output=True, text=True)
    return p.stdout
def run_py(sess):
    p = subprocess.run(["python3","lodgecore.py"], input=sess, capture_output=True, text=True)
    return p.stdout
def diff(sess):
    l=run_legacy(sess); p=run_py(sess)
    if l!=p:
        print("SESSION:"); print(sess)
        print("--- legacy ---"); print(l)
        print("--- py ---"); print(p)
        return False
    return True
