"""Short program templates for ``code-output``.

Each template draws its names and numbers from the item's random stream and
returns the program plus "trap" variants: single edits that encode the usual
misreading (a list comprehension instead of an aliased ``*``, a default
argument bound per call, ``let`` instead of ``var``). Their outputs are the
most plausible distractors; automatic single-edit mutants fill the rest.
"""

from __future__ import annotations

import random
from collections.abc import Callable
from dataclasses import dataclass
from string import Template

WORDS = [
    "amber",
    "birch",
    "cedar",
    "delta",
    "ember",
    "fjord",
    "grove",
    "harbor",
    "iris",
    "jasper",
    "kelp",
    "lumen",
    "maple",
    "nectar",
    "onyx",
    "prism",
    "quartz",
    "raven",
    "sable",
    "tundra",
    "umber",
    "velvet",
    "willow",
    "yarrow",
    "zephyr",
]
FRUITS = ["apple", "banana", "cherry", "date", "fig", "grape", "kiwi", "lemon", "mango", "pear"]


@dataclass(frozen=True)
class Program:
    template: str
    lang: str  # "python" or "javascript"
    source: str
    traps: tuple[str, ...]


def _fill(text: str, **values: object) -> str:
    return Template(text).substitute({k: str(v) for k, v in values.items()})


def _pick(rng: random.Random, pool: list[str], k: int) -> list[str]:
    return rng.sample(pool, k)


def _traps(source: str, *pairs: tuple[str, str]) -> tuple[str, ...]:
    out = []
    for old, new in pairs:
        if old in source:
            out.append(source.replace(old, new, 1))
    return tuple(t for t in out if t != source)


# ---------------------------------------------------------------- Python


def p_grid_alias(rng: random.Random) -> Program:
    rows, cols = rng.randint(2, 4), rng.randint(2, 4)
    aliased = "[[fill] * cols] * rows"
    fresh = "[[fill] * cols for _ in range(rows)]"
    use, other = (aliased, fresh) if rng.random() < 0.7 else (fresh, aliased)
    src = _fill(
        """\
def build(rows, cols, fill):
    grid = $use
    return grid


def total(grid):
    return [sum(row) for row in grid]


grid = build($rows, $cols, $fill)
grid[$i][$j] = $v
grid[$k][0] += $w
sums = total(grid)
print(sums)
print(sum(sums), len(grid[0]))
""",
        use=use,
        rows=rows,
        cols=cols,
        fill=rng.randint(0, 2),
        i=rng.randrange(rows),
        j=rng.randrange(cols),
        v=rng.randint(3, 9),
        k=rng.randrange(rows),
        w=rng.randint(1, 5),
    )
    return Program("py-grid-alias", "python", src, _traps(src, (use, other)))


def p_closure_loop(rng: random.Random) -> Program:
    n = rng.randint(3, 5)
    k = rng.randint(2, 5)
    x = rng.randint(1, 4)
    off = rng.randint(0, 3)
    late = f"lambda x: x * i + {off}"
    bound = f"lambda x, i=i: x * i + {off}"
    use, other = (late, bound) if rng.random() < 0.7 else (bound, late)
    if rng.random() < 0.5:
        body = f"handlers = []\nfor i in range({n}):\n    handlers.append({use})"
    else:
        body = f"handlers = [{use} for i in range({n})]"
    src = _fill(
        """\
$body


def apply_all(value):
    out = []
    for handler in handlers:
        out.append(handler(value))
    return out


results = apply_all($x)
print(results)
print(sum(results), "i" in globals())
print(handlers[0]($k))
""",
        body=body,
        x=x,
        k=k,
    )
    return Program("py-closure-loop", "python", src, _traps(src, (use, other)))


def p_default_mutable(rng: random.Random) -> Program:
    names = _pick(rng, WORDS, 4)
    shared = "log=[]):\n    log.append(event)"
    fresh = "log=None):\n    if log is None:\n        log = []\n    log.append(event)"
    use, other = (shared, fresh) if rng.random() < 0.7 else (fresh, shared)
    calls = [
        f'record("{names[0]}")',
        f'record("{names[1]}", [])',
        f'record("{names[2]}")',
        f'record("{names[3]}", ["x"])',
    ]
    rng.shuffle(calls)
    src = _fill(
        """\
def record(event, $use
    return len(log), log[-1]


results = []
results.append($c0)
results.append($c1)
results.append($c2)
results.append($c3)
for count, last in results:
    print(count, last)
print(len(record.__defaults__[0]), sum(c for c, _ in results) - $off)
""",
        use=use,
        c0=calls[0],
        c1=calls[1],
        c2=calls[2],
        c3=calls[3],
        off=rng.randint(0, 3),
    )
    return Program("py-default-mutable", "python", src, _traps(src, (use, other)))


def p_int_div_round(rng: random.Random) -> Program:
    a = rng.choice([-1, 1]) * rng.randint(7, 40)
    b = rng.choice([-1, 1]) * rng.randint(2, 6)
    half = rng.choice([0.5, 1.5, 2.5, 3.5, 4.5, -0.5, -1.5, -2.5])
    src = _fill(
        """\
a, b = $a, $b
quotient = a // b
remainder = a % b
truncated = int(a / b)
print(quotient, remainder, truncated)


def check(x, y):
    q, r = divmod(x, y)
    return q * y + r == x, round(x / y, 1)


print(check(a, b))
print(round($half), round($half + 1), -a // b)
""",
        a=a,
        b=b,
        half=half,
    )
    return Program(
        "py-int-div-round",
        "python",
        src,
        _traps(
            src,
            ("int(a / b)", "a // b"),
            (f"round({half})", f"int({half} + 0.5)"),
            ("-a // b", "-(a // b)"),
        ),
    )


def p_slicing(rng: random.Random) -> Program:
    word = "".join(rng.sample("abcdefghijklmnopqrstuvwxyz", rng.randint(9, 12)))
    n = len(word)
    a, b = sorted(rng.sample(range(1, n - 1), 2))
    step = rng.randint(2, 3)
    src = _fill(
        """\
text = "$word"
head = text[:$a]
middle = text[$a:$b]
tail = text[-$c:]
print(head, middle, tail)
print(text[::$step], text[::-$step])
print(text[$b:$a:-1], text[-1:-$c:-1])
nums = list(range($n))
nums[$a:$b] = []
print(len(nums), nums[$a], nums[-$step:])
""",
        word=word,
        a=a,
        b=b,
        c=rng.randint(2, 4),
        step=step,
        n=n,
    )
    return Program(
        "py-slicing",
        "python",
        src,
        _traps(
            src,
            (f"text[{b}:{a}:-1]", f"text[{a}:{b}][::-1]"),
            (f"nums[{a}:{b}] = []", f"del nums[{a}]"),
        ),
    )


def p_dict_order(rng: random.Random) -> Program:
    f = _pick(rng, FRUITS, 5)
    entries = [(f[0], 3), (f[1], 5), (f[0], 2), (f[2], 1), (f[3], 4)]
    rng.shuffle(entries)
    src = _fill(
        """\
stock = {}
arrivals = $entries
for name, qty in arrivals:
    stock[name] = stock.get(name, 0) + qty

removed = stock.pop("$pop")
stock["$pop"] = removed * 2
stock.update({"$f2": 9, "$f4": 7})
print(list(stock))
print(list(stock.values()))
first_key = next(iter(stock))
print(first_key, len(stock))
""",
        entries=repr(entries),
        pop=f[1],
        f2=f[2],
        f4=f[4],
    )
    return Program(
        "py-dict-order",
        "python",
        src,
        _traps(
            src,
            ("stock.get(name, 0) + qty", "qty"),
            (
                f'stock["{f[1]}"] = removed * 2',
                f'stock["{f[1]}"] = stock.get("{f[1]}", 0) or removed * 2',
            ),
        ),
    )


def p_string_methods(rng: random.Random) -> Program:
    w = _pick(rng, WORDS, 4)
    pad = rng.choice(["-", "*", "="])
    raw = f"  {pad}{pad}{w[0].title()},{w[1]},,{w[2].upper()},{w[3]}{pad}  "
    src = _fill(
        """\
raw = "$raw"
cleaned = raw.strip().strip("$pad")
parts = cleaned.split(",", $maxsplit)
print(parts)
labels = [p.title() for p in parts if p]
print("|".join(labels))
print(cleaned.count("$ch"), cleaned.find("$ch"), cleaned.rfind("$ch"))
print(raw.strip(" $pad$first").lower())
""",
        raw=raw,
        pad=pad,
        maxsplit=rng.randint(1, 3),
        ch=rng.choice("aeior"),
        first=w[0][0].upper(),
    )
    return Program(
        "py-string-methods",
        "python",
        src,
        _traps(src, (".title()", ".capitalize()")),
    )


def p_short_circuit(rng: random.Random) -> Program:
    vals = [rng.choice(["0", '""', "[]", "None", "5", '"ok"', "[0]", "3"]) for _ in range(7)]
    src = _fill(
        """\
calls = []


def check(name, value):
    calls.append(name)
    return value


r1 = check("a", $v0) or check("b", $v1) or check("c", $v2)
r2 = check("d", $v3) and check("e", $v4) and check("f", $v5)
r3 = not check("g", $v6) or check("h", 1)
print(repr(r1), repr(r2), r3)
print("".join(calls))
""",
        **{f"v{i}": v for i, v in enumerate(vals)},
    )
    return Program(
        "py-short-circuit",
        "python",
        src,
        (),
    )


def p_exceptions_loop(rng: random.Random) -> Program:
    pool = ["4", "x", "10", "", "7", "2.5", "-3", "12", "0", "9"]
    data = rng.sample(pool, rng.randint(5, 7))
    limit = rng.randint(5, 11)
    src = _fill(
        """\
data = $data
total = 0
errors = 0
seen = 0
for item in data:
    try:
        n = int(item)
        if n > $limit:
            continue
        total += n
    except ValueError:
        errors += 1
        continue
    else:
        total += 1
    finally:
        seen += 1
print(total, errors, seen)
""",
        data=repr(data),
        limit=limit,
    )
    return Program(
        "py-exceptions-loop",
        "python",
        src,
        _traps(src, ("    finally:\n        seen += 1", "    seen += 1")),
    )


def p_generators(rng: random.Random) -> Program:
    start, stop = rng.randint(1, 4), rng.randint(7, 11)
    n = rng.randint(4, 6)
    src = _fill(
        """\
squares = (k * k for k in range($start, $stop))
first = next(squares)
evens = [s for s in squares if s % 2 == 0]
print(first, evens)
print(sum(squares))

it = iter(range($n))
pairs = list(zip(it, "abc"))
print(pairs[-1], next(it, None))


def countdown(k):
    while k > 0:
        yield k
        k -= $dec


print(list(countdown($n)), max(countdown($n), default=-1))
""",
        start=start,
        stop=stop,
        n=n,
        dec=rng.randint(2, 3),
    )
    return Program(
        "py-generators",
        "python",
        src,
        _traps(
            src,
            (
                f"squares = (k * k for k in range({start}, {stop}))",
                f"squares = [k * k for k in range({start}, {stop})]\nsquares = iter(squares * 2)",
            ),
            (
                'pairs = list(zip(it, "abc"))',
                'pairs = list(zip("abc", it))\npairs = [(b, a) for a, b in pairs]',
            ),
        ),
    )


def p_augmented_alias(rng: random.Random) -> Program:
    x, y, z = rng.sample(range(1, 10), 3)
    src = _fill(
        """\
a = b = [$x, $y]
a += [$z]
c = d = ($x, $y)
c += ($z,)
e = f = [$y]
e = e + [$x]
g = [a, e]
g[0].append(0)
print(a, b)
print(c, d)
print(e, f, len(g[0]))
""",
        x=x,
        y=y,
        z=z,
    )
    return Program(
        "py-augmented-alias",
        "python",
        src,
        _traps(src, ("a += [", "a = a + ["), ("e = e + [", "e += [")),
    )


def p_sort_stability(rng: random.Random) -> Program:
    names = _pick(rng, ["ann", "bob", "cy", "dee", "eve", "flo", "gus", "hal", "ida"], 6)
    ages = [rng.choice([25, 31, 40]) for _ in names]
    people = list(zip(names, ages, strict=True))
    src = _fill(
        """\
people = $people
by_age = sorted(people, key=lambda p: p[1], reverse=True)
print([p[0] for p in by_age])
by_len = sorted(people, key=lambda p: (len(p[0]), -p[1]))
print([p[0] for p in by_len][:4])
people.sort(key=lambda p: p[0][-1])
oldest = max(people, key=lambda p: p[1])
print(oldest[0], people[0][0])
""",
        people=repr(people),
    )
    return Program(
        "py-sort-stability",
        "python",
        src,
        _traps(
            src,
            ("key=lambda p: p[1], reverse=True", "key=lambda p: -p[1]"),
            ("reverse=True)", "reverse=True)[::-1][::-1]"),
        ),
    )


def p_fromkeys_alias(rng: random.Random) -> Program:
    keys = _pick(rng, ["x", "y", "z", "w", "v"], 3)
    src = _fill(
        """\
keys = $keys
groups = dict.fromkeys(keys, [])
groups["$k0"].append($v0)
groups["$k1"] = groups["$k1"] + [$v1]
groups["$k2"].append($v2)
print(groups)
counts = {k: len(v) for k, v in groups.items()}
print(counts)
print(groups["$k0"] is groups["$k2"])
""",
        keys=repr(keys),
        k0=keys[0],
        k1=keys[1],
        k2=keys[2],
        v0=rng.randint(1, 9),
        v1=rng.randint(1, 9),
        v2=rng.randint(1, 9),
    )
    return Program(
        "py-fromkeys-alias",
        "python",
        src,
        _traps(src, ("dict.fromkeys(keys, [])", "{k: [] for k in keys}")),
    )


def p_class_attr(rng: random.Random) -> Program:
    tags = _pick(rng, WORDS, 3)
    src = _fill(
        """\
class Tracker:
    total = 0
    tags = []

    def __init__(self, tag):
        self.total += 1
        self.tags.append(tag)
        self.name = tag


a = Tracker("$t0")
b = Tracker("$t1")
b.total += $inc
c = Tracker("$t2")
print(Tracker.total, a.total, b.total, c.total)
print(len(a.tags), c.tags[-1], b.name)
""",
        t0=tags[0],
        t1=tags[1],
        t2=tags[2],
        inc=rng.randint(2, 5),
    )
    return Program(
        "py-class-attr",
        "python",
        src,
        _traps(
            src,
            ("self.total += 1", "Tracker.total += 1"),
            ("self.tags.append(tag)", "self.tags = [tag]"),
        ),
    )


def p_scoping(rng: random.Random) -> Program:
    x0, inc = rng.randint(5, 20), rng.randint(2, 7)
    n = rng.randint(3, 6)
    src = _fill(
        """\
x = $x0


def outer():
    x = 1

    def inner():
        nonlocal x
        x += $inc
        return x

    inner()
    inner()
    return x


squares = [x * x for x in range($n)]
print(outer(), x, squares[-1])
for k in range($n):
    pass
print(k, sum(squares))
""",
        x0=x0,
        inc=inc,
        n=n,
    )
    return Program(
        "py-scoping",
        "python",
        src,
        _traps(src, ("        nonlocal x\n", "        global x\n")),
    )


def p_for_else(rng: random.Random) -> Program:
    items = rng.sample(range(1, 20), 6)
    present = rng.choice(items)
    absent = next(v for v in range(20, 40) if v not in items)
    src = _fill(
        """\
def find(items, target):
    for i, v in enumerate(items):
        if v == target:
            break
    else:
        return -1
    return i


def first_even(items):
    for v in items:
        if v % 2 == 0:
            return v
    return None


data = $items
print(find(data, $present), find(data, $absent), find([], 1))
print(first_even(data), first_even([1, 3]))
""",
        items=repr(items),
        present=present,
        absent=absent,
    )
    return Program(
        "py-for-else",
        "python",
        src,
        _traps(
            src, ("    else:\n        return -1\n    return i", "        return -1\n    return i")
        ),
    )


def p_formatting(rng: random.Random) -> Program:
    price = rng.choice([2.675, 1.005, 0.125, 2.5, 3.14159, 7.345, 12.355])
    qty = rng.randint(3, 99)
    big = rng.randint(10_000, 9_999_999)
    src = _fill(
        """\
price = $price
qty = $qty
print(f"[{price:.2f}]")
print(f"[{qty:>4}|{qty:<4}|{qty:^5}]")
print(f"[{$big:,}]")
print("[%06.1f]" % price)
print(f"[{qty:03d}] [{qty / 200:.0%}]")
print(f"[{price * qty:.1f}]")
print(f"[{str(qty):*<6}]")
print(f"[{price!r}]")
print(f"[{-qty:+d}] [{qty:+d}]")
""",
        price=price,
        qty=qty,
        big=big,
    )
    return Program(
        "py-formatting",
        "python",
        src,
        _traps(src, ("price:.2f", "round(price, 2)")),
    )


def p_truthiness(rng: random.Random) -> Program:
    pool = ["0", '""', '"0"', "[]", "[0]", "None", "0.0", '"False"', "{}", "(0,)", "-1"]
    values = rng.sample(pool, 6)
    src = _fill(
        """\
values = [$values]
flags = [bool(v) for v in values]
print(flags)
print(sum(flags), flags.index(True) if True in flags else -1)
print(1 < 2 > 1, 3 == 3.0, True + True, (1, 2) < (1, 3))
print([] == [], [] is [], "a" < "B", 0.1 + 0.2 == 0.3)
print(any(values), all(values[1:]))
""",
        values=", ".join(values),
    )
    return Program("py-truthiness", "python", src, ())


def p_copy(rng: random.Random) -> Program:
    tags = _pick(rng, WORDS, 3)
    src = _fill(
        """\
import copy

base = {"tags": ["$t0"], "n": $n, "meta": {"v": 1}}
shallow = copy.copy(base)
deep = copy.deepcopy(base)
alias = base
shallow["tags"].append("$t1")
shallow["n"] = $n2
deep["tags"].append("$t2")
alias["meta"]["v"] += 1
print(base["tags"], base["n"])
print(shallow["tags"] is base["tags"], deep["tags"])
print(shallow["meta"]["v"], deep["meta"]["v"])
""",
        t0=tags[0],
        t1=tags[1],
        t2=tags[2],
        n=rng.randint(1, 5),
        n2=rng.randint(6, 9),
    )
    return Program(
        "py-copy",
        "python",
        src,
        _traps(src, ("shallow = copy.copy(base)", "shallow = copy.deepcopy(base)")),
    )


def p_ranges(rng: random.Random) -> Program:
    hi, lo, step = rng.randint(10, 16), rng.randint(0, 3), rng.randint(2, 4)
    src = _fill(
        """\
down = list(range($hi, $lo, -$step))
print(down)
labels = list(enumerate("$word", start=$start))
print(labels[-1])
pairs = list(zip([1, 2, 3, 4], "$short"))
print(pairs)
odds = list(reversed(range(1, $hi, $step)))
print(odds, len(range($lo, $hi, $step)))
""",
        hi=hi,
        lo=lo,
        step=step,
        word=rng.choice(WORDS),
        start=rng.randint(1, 5),
        short=rng.choice(["xy", "xyz", "q"]),
    )
    return Program("py-ranges", "python", src, ())


def p_memo_default(rng: random.Random) -> Program:
    n1, n2 = rng.randint(4, 10), rng.randint(2, 8)
    src = _fill(
        """\
calls = 0


def ways(n, memo={}):
    global calls
    calls += 1
    if n in memo:
        return memo[n]
    if n <= 1:
        return 1
    memo[n] = ways(n - 1) + ways(n - $back)
    return memo[n]


print(ways($n1), calls)
before = calls
print(ways($n2), calls - before)
""",
        n1=n1,
        n2=n2,
        back=rng.choice([2, 3]),
    )
    return Program(
        "py-memo-default",
        "python",
        src,
        _traps(
            src,
            (
                "def ways(n, memo={}):",
                "def ways(n, memo=None):\n    memo = {} if memo is None else memo",
            ),
        ),
    )


def p_finally_return(rng: random.Random) -> Program:
    vals = rng.sample([-3, -1, 0, 2, 5, 7], 3)
    src = _fill(
        """\
def risky(n):
    try:
        if n < 0:
            raise ValueError("negative")
        return n * $mul
    except ValueError:
        return -1
    finally:
        if n == 0:
            return 99


def lookup(table, key):
    try:
        return table[key]
    except LookupError as error:
        return type(error).__name__


print(risky($a), risky($b), risky($c))
print(lookup({"k": 1}, "z"), lookup([1, 2], $idx), lookup("ab", 1))
""",
        mul=rng.randint(2, 4),
        a=vals[0],
        b=vals[1],
        c=vals[2],
        idx=rng.choice([2, 5, -3]),
    )
    return Program(
        "py-finally-return",
        "python",
        src,
        _traps(
            src,
            (
                "        if n == 0:\n            return 99",
                "        if n == 0:\n            print(end='')",
            ),
        ),
    )


def p_remove_iter(rng: random.Random) -> Program:
    nums = sorted(rng.sample(range(1, 30), 7))
    src = _fill(
        """\
nums = $nums
for n in nums:
    if n % 2 == 0:
        nums.remove(n)
print(nums)

values = $nums
kept = []
for i, v in enumerate(values):
    if v % $m == 0:
        values.pop(i)
    else:
        kept.append(v)
print(len(values), kept)
""",
        nums=repr(nums),
        m=rng.choice([3, 4]),
    )
    return Program(
        "py-remove-iter",
        "python",
        src,
        _traps(src, ("for n in nums:", "for n in nums[:]:")),
    )


def p_string_sorting(rng: random.Random) -> Program:
    f = _pick(rng, FRUITS, 4)
    words = [f[0], f[1].title(), f[2], f[1], f[3].upper()]
    rng.shuffle(words)
    src = _fill(
        """\
words = $words
print(sorted(words))
print(max(words), min(words, key=str.lower))
print(sorted(words, key=len)[:2])
letters = sorted(set("$word"))
print("-".join(letters), "ab" * $rep, [0] * 0)
print(sorted(words, key=str.lower, reverse=True)[0])
""",
        words=repr(words),
        word=rng.choice(FRUITS),
        rep=rng.randint(2, 3),
    )
    return Program("py-string-sorting", "python", src, ())


def p_comprehension(rng: random.Random) -> Program:
    n, m = rng.randint(3, 5), rng.randint(2, 4)
    q, lo = rng.randint(2, 3), rng.randint(0, 1)
    src = _fill(
        """\
pairs = [(i, j) for i in range($lo, $n) for j in range(i) if (i + j) % $q]
print(pairs)
grid = [[i * j for j in range($m + 1)] for i in range($m)]
flat = [x for row in grid for x in row if x % $q]
print(grid[-1])
print(flat)
lookup = {x % $m: x for x in range($lo, $n * 2)}
print(lookup)
nested = [[j for j in range(i)] for i in range($n)]
print(sum(len(r) for r in nested), nested[-1][-$q:])
""",
        n=n,
        m=m,
        q=q,
        lo=lo,
    )
    return Program(
        "py-comprehension",
        "python",
        src,
        _traps(
            src,
            (
                f"for i in range({lo}, {n}) for j in range(i)",
                f"for j in range({lo}, {n}) for i in range(j)",
            ),
            (f"lookup = {{x % {m}: x", f"lookup = {{x // {m}: x"),
        ),
    )


def p_bit_ops(rng: random.Random) -> Program:
    a, b = rng.randint(5, 30), rng.randint(2, 9)
    src = _fill(
        """\
a, b = $a, $b
print(a & b, a | b, a ^ b)
print(~a, a >> 2, b << 3)
print(-a % b, -(a % b), a % -b)
print(sum([True, False, True]), True * $b)
print(2 ** -1, 7 // 2.0, abs(-a) // b)
flags = 0
for bit in ($b0, $b1, $b0):
    flags ^= 1 << bit
print(bin(flags), flags)
""",
        a=a,
        b=b,
        b0=rng.randint(0, 3),
        b1=rng.randint(4, 6),
    )
    return Program(
        "py-bit-ops",
        "python",
        src,
        _traps(src, ("flags ^= 1 << bit", "flags |= 1 << bit")),
    )


def p_while_walrus(rng: random.Random) -> Program:
    items = rng.sample(range(1, 15), 7)
    limit = rng.randint(10, 25)
    src = _fill(
        """\
queue = $items
taken = []
budget = $limit
while queue and (item := queue.pop(0)) <= budget:
    taken.append(item)
    budget -= item
print(taken, budget, queue)

count = 0
n = $n
while n != 1:
    n = n // 2 if n % 2 == 0 else 3 * n + 1
    count += 1
print(count)
""",
        items=repr(items),
        limit=limit,
        n=rng.randint(6, 12),
    )
    return Program(
        "py-while-walrus",
        "python",
        src,
        _traps(
            src,
            ("(item := queue.pop(0)) <= budget", "(item := queue[0]) <= budget and queue.pop(0)"),
        ),
    )


# ------------------------------------------------------------ JavaScript


def j_var_let(rng: random.Random) -> Program:
    n, k = rng.randint(3, 6), rng.randint(2, 9)
    use, other = ("var", "let") if rng.random() < 0.7 else ("let", "var")
    src = _fill(
        """\
const fns = [];
for ($use i = 0; i < $n; i++) {
  fns.push(() => i * $k);
}
console.log(fns.map((f) => f()).join(","));

const later = [];
for ($use j = 1; j <= 3; j++) {
  later.push(function () {
    return j + $k;
  });
}
console.log(later[0](), later[2]());
console.log(typeof i, typeof j);
""",
        use=use,
        n=n,
        k=k,
    )
    return Program("js-var-let", "javascript", src, (src.replace(f"({use} ", f"({other} "),))


def j_coercion(rng: random.Random) -> Program:
    a, b = rng.randint(2, 9), rng.randint(2, 9)
    nums = rng.sample([1, 2, 5, 9, 10, 15, 21, 100, 33], 5)
    src = _fill(
        """\
const a = "$a";
const b = $b;
console.log(a + b, a - b, a * "2", true + b);
const nums = [$nums];
console.log(nums.sort().join(","));
console.log(nums.sort((x, y) => x - y).join(","));
console.log(0.1 + 0.2 === 0.3, (0.1 + 0.2).toFixed(2));
console.log([] + [], [1] + [2], "3" + 4 + 5, 3 + 4 + "5");
console.log(parseInt("$a$b.9px"), Number(""), Number("12px"));
""",
        a=a,
        b=b,
        nums=", ".join(map(str, nums)),
    )
    return Program(
        "js-coercion",
        "javascript",
        src,
        (src.replace("nums.sort().join", "nums.sort((x, y) => x - y).join", 1),),
    )


def j_array_methods(rng: random.Random) -> Program:
    n = rng.randint(6, 8)
    s, c = rng.randint(1, 3), rng.randint(1, 3)
    src = _fill(
        """\
const strs = ["10", "10", "10", "$v"];
console.log(strs.map(parseInt).join(","));
const arr = Array.from({ length: $n }, (_, i) => i * 2);
const removed = arr.splice($s, $c);
console.log(arr.join(","), removed.join(","));
const copy = arr.slice(-$c);
console.log(copy.join(","), arr.length);
console.log([NaN].indexOf(NaN), [NaN].includes(NaN));
console.log([3, 1, 2].reverse().concat([9]).join(""));
""",
        v=rng.randint(2, 12),
        n=n,
        s=s,
        c=c,
    )
    return Program(
        "js-array-methods",
        "javascript",
        src,
        (src.replace("strs.map(parseInt)", "strs.map((x) => parseInt(x))", 1),),
    )


def j_hoisting(rng: random.Random) -> Program:
    v = rng.randint(2, 19)
    calls = rng.randint(1, 3)
    src = _fill(
        """\
console.log(typeof hoisted, typeof notYet, notYet);
var notYet = $v;
function hoisted() {
  return 1;
}
console.log(typeof null, typeof [], typeof undefined, typeof NaN);

function counter() {
  let count = $v;
  return () => ++count;
}
const next = counter();
$warmup
console.log(next(), next(), typeof counter());
""",
        v=v,
        warmup="\n".join(["next();"] * calls),
    )
    return Program("js-hoisting", "javascript", src, (src.replace("++count", "count++", 1),))


def j_object_keys(rng: random.Random) -> Program:
    ks = rng.sample(["b", "a", "z", "m"], 3)
    ns = rng.sample(["10", "2", "7", "01", "-1"], 3)
    src = _fill(
        """\
const o = {};
o.$k0 = 1;
o["$n0"] = 2;
o.$k1 = 3;
o["$n1"] = 4;
o["$n2"] = 5;
o.$k2 = 6;
console.log(Object.keys(o).join(","));
delete o.$k0;
o.$k0 = 7;
console.log(Object.values(o).join(","));
console.log(JSON.stringify({ b: 1, a: [undefined, 2], c: undefined }));
""",
        k0=ks[0],
        k1=ks[1],
        k2=ks[2],
        n0=ns[0],
        n1=ns[1],
        n2=ns[2],
    )
    return Program("js-object-keys", "javascript", src, ())


def j_equality(rng: random.Random) -> Program:
    exprs = [
        "null == undefined",
        "null === undefined",
        "NaN === NaN",
        '[1, 2] == "1,2"',
        '0 == ""',
        '"0" == false',
        "[] == false",
        "null == 0",
        "undefined == 0",
        '"b" > "a"',
        '"10" < "9"',
        "10 < 9",
    ]
    picks = rng.sample(exprs, 8)
    src = _fill(
        """\
const results = [
  $e0,
  $e1,
  $e2,
  $e3,
];
console.log(results.join(" "));
const more = [$e4, $e5, $e6, $e7];
console.log(more.filter(Boolean).length, more.indexOf(true));
console.log(Object.is(-0, 0), -0 === 0);
""",
        **{f"e{i}": e for i, e in enumerate(picks)},
    )
    return Program("js-equality", "javascript", src, ())


def j_event_loop(rng: random.Random) -> Program:
    tags = _pick(rng, ["alpha", "beta", "gamma", "delta", "omega"], 3)
    src = _fill(
        """\
const out = [];
setTimeout(() => {
  out.push("timeout");
  console.log(out.join(","));
}, 0);
Promise.resolve()
  .then(() => out.push("$t0"))
  .then(() => out.push("$t1"));
queueMicrotask(() => out.push("$t2"));
(async () => {
  out.push("async-start");
  await null;
  out.push("async-end");
})();
out.push("sync");
""",
        t0=tags[0],
        t1=tags[1],
        t2=tags[2],
    )
    return Program(
        "js-event-loop",
        "javascript",
        src,
        (src.replace("  await null;\n", "", 1),),
    )


def j_rounding(rng: random.Random) -> Program:
    h = rng.choice(["2.5", "-2.5", "0.5", "-0.5", "1.5", "-1.5"])
    a = rng.randint(5, 15)
    src = _fill(
        """\
console.log(Math.round($h), Math.round(-0.4), Math.round($h + 1));
console.log((1.005).toFixed(2), (2.345).toFixed(2), (0.5).toFixed(0));
console.log(Math.floor(-$a / 2), Math.trunc(-$a / 2), -$a % 2);
console.log(($a / 4) | 0, ~~(-$a / 4), $a >> 1);
const cents = [0.1, 0.2, 0.7].reduce((s, x) => s + x, 0);
console.log(cents, cents.toFixed(1), Number.isInteger($a / 1));
""",
        h=h,
        a=a,
    )
    return Program(
        "js-rounding",
        "javascript",
        src,
        (src.replace("Math.floor(-", "Math.trunc(-", 1),),
    )


TEMPLATES: dict[str, Callable[[random.Random], Program]] = {
    "py-grid-alias": p_grid_alias,
    "py-closure-loop": p_closure_loop,
    "py-default-mutable": p_default_mutable,
    "py-int-div-round": p_int_div_round,
    "py-slicing": p_slicing,
    "py-dict-order": p_dict_order,
    "py-string-methods": p_string_methods,
    "py-short-circuit": p_short_circuit,
    "py-exceptions-loop": p_exceptions_loop,
    "py-generators": p_generators,
    "py-augmented-alias": p_augmented_alias,
    "py-sort-stability": p_sort_stability,
    "py-fromkeys-alias": p_fromkeys_alias,
    "py-class-attr": p_class_attr,
    "py-scoping": p_scoping,
    "py-for-else": p_for_else,
    "py-formatting": p_formatting,
    "py-truthiness": p_truthiness,
    "py-copy": p_copy,
    "py-ranges": p_ranges,
    "py-memo-default": p_memo_default,
    "py-finally-return": p_finally_return,
    "py-remove-iter": p_remove_iter,
    "py-string-sorting": p_string_sorting,
    "py-comprehension": p_comprehension,
    "py-bit-ops": p_bit_ops,
    "py-while-walrus": p_while_walrus,
    "js-var-let": j_var_let,
    "js-coercion": j_coercion,
    "js-array-methods": j_array_methods,
    "js-hoisting": j_hoisting,
    "js-object-keys": j_object_keys,
    "js-equality": j_equality,
    "js-event-loop": j_event_loop,
    "js-rounding": j_rounding,
}
