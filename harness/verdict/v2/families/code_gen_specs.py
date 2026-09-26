"""Docstring specs for ``expected-value``.

Each spec has a signature and docstring (what the model sees), a hidden
reference implementation, a few "slip" implementations that encode the usual
mistakes (inclusive where the spec says exclusive, banker's rounding, lower
median, dropped partial chunk), and an argument generator that favours the
boundary cases where those slips disagree with the reference. Everything is
executed in the sandbox to label items: a true item asserts the reference's
return value, a false item asserts a slip's value that differs from it.
"""

from __future__ import annotations

import json
import random
from collections.abc import Callable
from dataclasses import dataclass
from datetime import date, timedelta

from harness.verdict.v2.families.code_gen_exec import Job, runner_for
from harness.verdict.v2.families.code_gen_select import above_median, balance_noul, numbers
from harness.verdict.v2.items import Item, make_item, noul_question
from harness.verdict.v2.registry import BuildContext

INSTANCES_PER_SPEC = 40


@dataclass(frozen=True)
class Spec:
    name: str
    signature: str
    doc: str
    reference: str
    slips: tuple[str, ...]
    args: Callable[[random.Random], tuple[object, ...]]
    imports: str = ""


def _words(rng: random.Random, k: int) -> list[str]:
    pool = ["red", "blue", "green", "gold", "teal", "plum", "rose", "sand", "mint", "gray"]
    return [rng.choice(pool) for _ in range(k)]


def _date(rng: random.Random, year: int | None = None) -> str:
    y = year or rng.randint(2019, 2027)
    m = rng.randint(1, 12)
    d = rng.randint(1, 28)
    return f"{y:04d}-{m:02d}-{d:02d}"


def _weighted_args(rng: random.Random) -> tuple[object, ...]:
    n = rng.randint(3, 5)
    return ([rng.randint(1, 20) for _ in range(n)], [rng.randint(1, 4) for _ in range(n)])


def _shift(iso: str, days: int) -> str:
    return (date.fromisoformat(iso) + timedelta(days=days)).isoformat()


def _roman(n: int) -> str:
    parts = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ]
    out = []
    for value, sym in parts:
        while n >= value:
            out.append(sym)
            n -= value
    return "".join(out)


def _parens(rng: random.Random) -> str:
    out = []
    depth = 0
    for _ in range(rng.randint(6, 14)):
        if depth and rng.random() < 0.45:
            out.append(")")
            depth -= 1
        elif rng.random() < 0.3:
            out.append(rng.choice("ab+"))
        else:
            out.append("(")
            depth += 1
    out.extend(")" * depth)
    return "".join(out)


SPECS: tuple[Spec, ...] = (
    Spec(
        "count_multiples",
        "def count_multiples(lo: int, hi: int, k: int) -> int:",
        "Return how many integers n with lo <= n < hi are divisible by k.",
        "def count_multiples(lo, hi, k):\n    return sum(1 for n in range(lo, hi) if n % k == 0)",
        (
            "def count_multiples(lo, hi, k):\n    return sum(1 for n in range(lo, hi + 1) if n % k == 0)",
            "def count_multiples(lo, hi, k):\n    return sum(1 for n in range(lo + 1, hi) if n % k == 0)",
            "def count_multiples(lo, hi, k):\n    return (hi - lo) // k",
        ),
        lambda r: (
            (k := r.randint(3, 9)) * r.randint(0, 4),
            k * r.randint(6, 15) + r.choice([0, 0, 1, 2]),
            k,
        ),
    ),
    Spec(
        "median",
        "def median(values: list[float]) -> float:",
        "Return the median of values. For an even count, return the mean of the two middle "
        "values after sorting.",
        "def median(values):\n    s = sorted(values)\n    n = len(s)\n    mid = n // 2\n"
        "    return s[mid] if n % 2 else (s[mid - 1] + s[mid]) / 2",
        (
            "def median(values):\n    s = sorted(values)\n    return s[(len(s) - 1) // 2]",
            "def median(values):\n    s = sorted(values)\n    return s[len(s) // 2]",
            "def median(values):\n    s = values\n    n = len(s)\n    mid = n // 2\n"
            "    return s[mid] if n % 2 else (s[mid - 1] + s[mid]) / 2",
        ),
        lambda r: ([r.randint(1, 60) for _ in range(r.choice([4, 5, 6, 7, 8]))],),
    ),
    Spec(
        "round_half_up",
        "def round_half_up(x: float, digits: int) -> float:",
        "Round x to the given number of decimal places, rounding halves away from zero, as "
        "written in decimal (so 2.675 with 2 digits gives 2.68).",
        "def round_half_up(x, digits):\n    from decimal import Decimal, ROUND_HALF_UP\n"
        "    q = Decimal(1).scaleb(-digits)\n"
        "    return float(Decimal(str(x)).quantize(q, rounding=ROUND_HALF_UP))",
        (
            "def round_half_up(x, digits):\n    return round(x, digits)",
            "def round_half_up(x, digits):\n    import math\n    f = 10 ** digits\n"
            "    return math.trunc(x * f) / f",
            "def round_half_up(x, digits):\n    from decimal import Decimal, ROUND_HALF_EVEN\n"
            "    q = Decimal(1).scaleb(-digits)\n"
            "    return float(Decimal(str(x)).quantize(q, rounding=ROUND_HALF_EVEN))",
        ),
        lambda r: (
            round(
                r.choice([-1, 1])
                * (r.randint(0, 40) + r.choice([0.5, 0.25, 0.125, 0.675, 0.345, 0.05])),
                3,
            ),
            r.choice([0, 1, 2]),
        ),
    ),
    Spec(
        "chunk",
        "def chunk(items: list[int], size: int) -> list[list[int]]:",
        "Split items into consecutive chunks of length size, in order. The last chunk may be "
        "shorter. An empty list gives an empty list.",
        "def chunk(items, size):\n    return [items[i : i + size] for i in range(0, len(items), size)]",
        (
            "def chunk(items, size):\n    return [items[i : i + size] for i in range(0, len(items) - size + 1, size)]",
            "def chunk(items, size):\n    return [items[i : i + size] for i in range(0, len(items), size + 1)]",
            "def chunk(items, size):\n    return [items[i : i + size - 1] for i in range(0, len(items), size)]",
        ),
        lambda r: ([r.randint(1, 9) for _ in range(r.randint(4, 10))], r.randint(2, 4)),
    ),
    Spec(
        "rotate_right",
        "def rotate_right(items: list[str], k: int) -> list[str]:",
        "Return items rotated right by k positions: the last k items move to the front. k may "
        "be larger than the list length.",
        "def rotate_right(items, k):\n    if not items:\n        return []\n    k %= len(items)\n"
        "    return items[-k:] + items[:-k] if k else list(items)",
        (
            "def rotate_right(items, k):\n    if not items:\n        return []\n    k %= len(items)\n"
            "    return items[k:] + items[:k]",
            "def rotate_right(items, k):\n    return items[-k:] + items[:-k]",
            "def rotate_right(items, k):\n    if not items:\n        return []\n    k = (k + 1) % len(items)\n"
            "    return items[-k:] + items[:-k] if k else list(items)",
        ),
        lambda r: (list("abcdefgh"[: r.randint(3, 7)]), r.randint(1, 11)),
    ),
    Spec(
        "run_lengths",
        "def run_lengths(text: str) -> list[tuple[str, int]]:",
        "Return (character, length) for each run of identical consecutive characters, in order.",
        "def run_lengths(text):\n    out = []\n    for ch in text:\n"
        "        if out and out[-1][0] == ch:\n            out[-1] = (ch, out[-1][1] + 1)\n"
        "        else:\n            out.append((ch, 1))\n    return out",
        (
            "def run_lengths(text):\n    out = {}\n    for ch in text:\n        out[ch] = out.get(ch, 0) + 1\n"
            "    return list(out.items())",
            "def run_lengths(text):\n    out = []\n    for a, b in zip(text, text[1:]):\n"
            "        if out and out[-1][0] == a and a == b:\n            out[-1] = (a, out[-1][1] + 1)\n"
            "        elif a == b:\n            out.append((a, 1))\n        elif not out or out[-1][0] != a:\n"
            "            out.append((a, 1))\n    return out",
        ),
        lambda r: ("".join(r.choice("aab") * r.randint(1, 3) for _ in range(r.randint(3, 5))),),
    ),
    Spec(
        "moving_average",
        "def moving_average(values: list[int], window: int) -> list[float]:",
        "Return the average of every full window of `window` consecutive values, in order, "
        "each rounded to 2 decimal places.",
        "def moving_average(values, window):\n"
        "    return [round(sum(values[i : i + window]) / window, 2) for i in range(len(values) - window + 1)]",
        (
            "def moving_average(values, window):\n"
            "    return [round(sum(values[i : i + window]) / window, 2) for i in range(len(values) - window)]",
            "def moving_average(values, window):\n"
            "    return [round(sum(values[i : i + window]) / len(values[i : i + window]), 2) for i in range(len(values))]",
            "def moving_average(values, window):\n"
            "    return [sum(values[i : i + window]) // window for i in range(len(values) - window + 1)]",
        ),
        lambda r: ([r.randint(1, 20) for _ in range(r.randint(4, 7))], r.randint(2, 3)),
    ),
    Spec(
        "dedupe",
        "def dedupe(items: list[int]) -> list[int]:",
        "Remove repeated values, keeping the first occurrence of each, in original order.",
        "def dedupe(items):\n    return list(dict.fromkeys(items))",
        (
            "def dedupe(items):\n    return list(dict.fromkeys(reversed(items)))[::-1]",
            "def dedupe(items):\n    return sorted(set(items))",
            "def dedupe(items):\n    return [x for i, x in enumerate(items) if x not in items[i + 1 :]]",
        ),
        lambda r: ([r.randint(1, 7) for _ in range(r.randint(6, 9))],),
    ),
    Spec(
        "weekdays_between",
        "def weekdays_between(start: str, end: str) -> int:",
        "Count the days from start (inclusive) to end (exclusive) that fall on Monday to "
        "Friday. Dates are ISO strings (YYYY-MM-DD).",
        "def weekdays_between(start, end):\n    from datetime import date, timedelta\n"
        "    d, e = date.fromisoformat(start), date.fromisoformat(end)\n    n = 0\n"
        "    while d < e:\n        n += d.weekday() < 5\n        d += timedelta(days=1)\n    return n",
        (
            "def weekdays_between(start, end):\n    from datetime import date, timedelta\n"
            "    d, e = date.fromisoformat(start), date.fromisoformat(end)\n    n = 0\n"
            "    while d <= e:\n        n += d.weekday() < 5\n        d += timedelta(days=1)\n    return n",
            "def weekdays_between(start, end):\n    from datetime import date\n"
            "    return (date.fromisoformat(end) - date.fromisoformat(start)).days * 5 // 7",
            "def weekdays_between(start, end):\n    from datetime import date, timedelta\n"
            "    d, e = date.fromisoformat(start), date.fromisoformat(end)\n    n = 0\n"
            "    while d < e:\n        n += d.weekday() <= 5\n        d += timedelta(days=1)\n    return n",
        ),
        lambda r: (
            (s := f"2026-{r.randint(1, 12):02d}-{r.randint(1, 28):02d}"),
            _shift(s, r.randint(3, 20)),
        ),
    ),
    Spec(
        "count_leap_years",
        "def count_leap_years(first: int, last: int) -> int:",
        "Count the Gregorian leap years y with first <= y <= last. A year is a leap year if it "
        "is divisible by 4, except years divisible by 100 that are not divisible by 400.",
        "def count_leap_years(first, last):\n"
        "    return sum(1 for y in range(first, last + 1) if y % 4 == 0 and (y % 100 != 0 or y % 400 == 0))",
        (
            "def count_leap_years(first, last):\n    return sum(1 for y in range(first, last + 1) if y % 4 == 0)",
            "def count_leap_years(first, last):\n"
            "    return sum(1 for y in range(first, last) if y % 4 == 0 and (y % 100 != 0 or y % 400 == 0))",
            "def count_leap_years(first, last):\n"
            "    return sum(1 for y in range(first, last + 1) if y % 4 == 0 and y % 100 != 0)",
        ),
        lambda r: (
            (a := r.choice([1696, 1788, 1880, 1896, 1990, 2000, 2092])),
            a + r.randint(8, 24),
        ),
    ),
    Spec(
        "insert_position",
        "def insert_position(sorted_values: list[int], x: int) -> int:",
        "Return the index at which x would be inserted into the ascending list to keep it "
        "sorted, placing x after any values equal to it.",
        "def insert_position(sorted_values, x):\n    import bisect\n    return bisect.bisect_right(sorted_values, x)",
        (
            "def insert_position(sorted_values, x):\n    import bisect\n    return bisect.bisect_left(sorted_values, x)",
            "def insert_position(sorted_values, x):\n    return sum(1 for v in sorted_values if v < x) + 1",
        ),
        lambda r: (
            (vals := sorted(r.randint(1, 12) for _ in range(r.randint(5, 8)))),
            r.choice(vals) if r.random() < 0.7 else r.randint(0, 13),
        ),
    ),
    Spec(
        "merge_intervals",
        "def merge_intervals(intervals: list[list[int]]) -> list[list[int]]:",
        "Merge overlapping intervals [start, end] and return them sorted by start. Intervals "
        "that only touch (one ends where the next starts) are merged too.",
        "def merge_intervals(intervals):\n    out = []\n    for s, e in sorted(intervals):\n"
        "        if out and s <= out[-1][1]:\n            out[-1][1] = max(out[-1][1], e)\n"
        "        else:\n            out.append([s, e])\n    return out",
        (
            "def merge_intervals(intervals):\n    out = []\n    for s, e in sorted(intervals):\n"
            "        if out and s < out[-1][1]:\n            out[-1][1] = max(out[-1][1], e)\n"
            "        else:\n            out.append([s, e])\n    return out",
            "def merge_intervals(intervals):\n    out = []\n    for s, e in intervals:\n"
            "        if out and s <= out[-1][1]:\n            out[-1][1] = max(out[-1][1], e)\n"
            "        else:\n            out.append([s, e])\n    return out",
            "def merge_intervals(intervals):\n    out = []\n    for s, e in sorted(intervals):\n"
            "        if out and s <= out[-1][1]:\n            out[-1][1] = e\n"
            "        else:\n            out.append([s, e])\n    return out",
        ),
        lambda r: (
            [[s, s + r.randint(1, 5)] for s in (r.randint(0, 20) for _ in range(r.randint(3, 5)))],
        ),
    ),
    Spec(
        "roman_to_int",
        "def roman_to_int(numeral: str) -> int:",
        "Convert a valid Roman numeral (I, V, X, L, C, D, M, with subtractive pairs such as IV "
        "and CM) to an integer.",
        "def roman_to_int(numeral):\n    v = {'I': 1, 'V': 5, 'X': 10, 'L': 50, 'C': 100, 'D': 500, 'M': 1000}\n"
        "    total = 0\n    for a, b in zip(numeral, numeral[1:] + ' '):\n"
        "        total += -v[a] if b != ' ' and v[a] < v[b] else v[a]\n    return total",
        (
            "def roman_to_int(numeral):\n    v = {'I': 1, 'V': 5, 'X': 10, 'L': 50, 'C': 100, 'D': 500, 'M': 1000}\n"
            "    return sum(v[c] for c in numeral)",
            "def roman_to_int(numeral):\n    v = {'I': 1, 'V': 5, 'X': 10, 'L': 50, 'C': 100, 'D': 500, 'M': 1000}\n"
            "    total = 0\n    for a, b in zip(numeral, numeral[1:] + ' '):\n"
            "        total += -v[a] if b != ' ' and v[a] <= v[b] else v[a]\n    return total",
        ),
        lambda r: (_roman(r.randint(4, 3999)),),
    ),
    Spec(
        "digital_root",
        "def digital_root(n: int) -> int:",
        "Repeatedly replace n by the sum of its decimal digits until a single digit remains, "
        "and return it. n is non-negative.",
        "def digital_root(n):\n    while n >= 10:\n        n = sum(int(d) for d in str(n))\n    return n",
        (
            "def digital_root(n):\n    return sum(int(d) for d in str(n))",
            "def digital_root(n):\n    return n % 9",
        ),
        lambda r: (r.randint(10, 99999) if r.random() < 0.8 else 9 * r.randint(2, 999),),
    ),
    Spec(
        "count_primes_below",
        "def count_primes_below(n: int) -> int:",
        "Return how many prime numbers are strictly less than n.",
        "def count_primes_below(n):\n    return sum(1 for k in range(2, n) if all(k % d for d in range(2, int(k ** 0.5) + 1)))",
        (
            "def count_primes_below(n):\n    return sum(1 for k in range(2, n + 1) if all(k % d for d in range(2, int(k ** 0.5) + 1)))",
            "def count_primes_below(n):\n    return sum(1 for k in range(1, n) if all(k % d for d in range(2, int(k ** 0.5) + 1)))",
        ),
        lambda r: (
            r.choice([11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 30, 40, 50, 60]),
        ),
    ),
    Spec(
        "progressive_tax",
        "def progressive_tax(income: int, brackets: list[tuple[int | None, float]]) -> float:",
        "Each (upper_limit, rate) bracket taxes only the part of income between the previous "
        "limit (0 for the first) and upper_limit; None means no upper limit. Brackets are in "
        "ascending order. Return the total tax rounded to 2 decimal places.",
        "def progressive_tax(income, brackets):\n    tax, lower = 0.0, 0\n    for upper, rate in brackets:\n"
        "        top = income if upper is None else min(income, upper)\n        if top > lower:\n"
        "            tax += (top - lower) * rate\n        if upper is None or income <= upper:\n"
        "            break\n        lower = upper\n    return round(tax, 2)",
        (
            "def progressive_tax(income, brackets):\n    for upper, rate in brackets:\n"
            "        if upper is None or income <= upper:\n            return round(income * rate, 2)\n    return 0.0",
            "def progressive_tax(income, brackets):\n    tax, lower = 0.0, 0\n    for upper, rate in brackets:\n"
            "        top = income if upper is None else min(income, upper)\n        if top > lower:\n"
            "            tax += top * rate\n        if upper is None or income <= upper:\n"
            "            break\n        lower = upper\n    return round(tax, 2)",
        ),
        lambda r: (
            r.randint(5, 90) * 1000 + r.choice([0, 500]),
            [(10000, 0.1), (r.choice([30000, 40000]), 0.2), (None, r.choice([0.3, 0.35, 0.4]))],
        ),
    ),
    Spec(
        "caesar",
        "def caesar(text: str, shift: int) -> str:",
        "Shift each ASCII letter forward by shift places in the alphabet, wrapping from z to a "
        "and keeping case. Other characters are unchanged.",
        "def caesar(text, shift):\n    out = []\n    for ch in text:\n        if ch.isascii() and ch.isalpha():\n"
        "            base = ord('A') if ch.isupper() else ord('a')\n"
        "            out.append(chr((ord(ch) - base + shift) % 26 + base))\n"
        "        else:\n            out.append(ch)\n    return ''.join(out)",
        (
            "def caesar(text, shift):\n    return ''.join(chr(ord(c) + shift) if c.isalpha() else c for c in text)",
            "def caesar(text, shift):\n    out = []\n    for ch in text:\n        if ch.isalpha():\n"
            "            out.append(chr((ord(ch.lower()) - 97 + shift) % 26 + 97))\n"
            "        else:\n            out.append(ch)\n    return ''.join(out)",
            "def caesar(text, shift):\n    out = []\n    for ch in text:\n        if ch.isalpha():\n"
            "            base = ord('A') if ch.isupper() else ord('a')\n"
            "            out.append(chr((ord(ch) - base - shift) % 26 + base))\n"
            "        else:\n            out.append(ch)\n    return ''.join(out)",
        ),
        lambda r: (
            r.choice(["Zebra-9", "xyz", "Hello, World", "Quiz Box", "yak", "Vex Zap"]),
            r.randint(1, 5),
        ),
    ),
    Spec(
        "top_words",
        "def top_words(text: str, k: int) -> list[str]:",
        "Return the k most frequent words in text (words are separated by spaces and compared "
        "lowercased), most frequent first; ties are broken alphabetically.",
        "def top_words(text, k):\n    from collections import Counter\n    c = Counter(w.lower() for w in text.split())\n"
        "    return [w for w, _ in sorted(c.items(), key=lambda p: (-p[1], p[0]))[:k]]",
        (
            "def top_words(text, k):\n    from collections import Counter\n    c = Counter(w.lower() for w in text.split())\n"
            "    return [w for w, _ in c.most_common(k)]",
            "def top_words(text, k):\n    from collections import Counter\n    c = Counter(text.split())\n"
            "    return [w for w, _ in sorted(c.items(), key=lambda p: (-p[1], p[0]))[:k]]",
        ),
        lambda r: (
            " ".join(w.upper() if r.random() < 0.2 else w for w in _words(r, r.randint(6, 10))),
            r.randint(2, 3),
        ),
    ),
    Spec(
        "sum_multiples_3_or_5",
        "def sum_multiples_3_or_5(n: int) -> int:",
        "Return the sum of the positive integers below n that are multiples of 3 or 5.",
        "def sum_multiples_3_or_5(n):\n    return sum(k for k in range(1, n) if k % 3 == 0 or k % 5 == 0)",
        (
            "def sum_multiples_3_or_5(n):\n    return sum(k for k in range(1, n + 1) if k % 3 == 0 or k % 5 == 0)",
            "def sum_multiples_3_or_5(n):\n    return sum(range(3, n, 3)) + sum(range(5, n, 5))",
        ),
        lambda r: (r.choice([10, 15, 16, 20, 21, 25, 30, 31, 35, 45, 46]),),
    ),
    Spec(
        "format_duration",
        "def format_duration(seconds: int) -> str:",
        "Format a non-negative number of seconds as H:MM:SS, with minutes and seconds padded to "
        "two digits and hours not padded.",
        "def format_duration(seconds):\n    h, rest = divmod(seconds, 3600)\n    m, s = divmod(rest, 60)\n"
        "    return f'{h}:{m:02d}:{s:02d}'",
        (
            "def format_duration(seconds):\n    h, rest = divmod(seconds, 3600)\n    m, s = divmod(rest, 60)\n"
            "    return f'{h}:{m}:{s:02d}'",
            "def format_duration(seconds):\n    h = seconds // 3600\n    m = seconds // 60\n    s = seconds % 60\n"
            "    return f'{h}:{m:02d}:{s:02d}'",
            "def format_duration(seconds):\n    h, rest = divmod(seconds, 3600)\n    m, s = divmod(rest, 60)\n"
            "    return f'{h:02d}:{m:02d}:{s:02d}'",
        ),
        lambda r: (r.randint(0, 5) * 3600 + r.randint(0, 12) * 60 + r.randint(0, 59),),
    ),
    Spec(
        "page_summary",
        "def page_summary(total_items: int, per_page: int) -> tuple[int, int]:",
        "Return (number of pages, items on the last page) when total_items are shown per_page "
        "at a time. Zero items gives (0, 0).",
        "def page_summary(total_items, per_page):\n    if total_items == 0:\n        return (0, 0)\n"
        "    pages = -(-total_items // per_page)\n    return (pages, total_items - (pages - 1) * per_page)",
        (
            "def page_summary(total_items, per_page):\n    return (total_items // per_page, total_items % per_page)",
            "def page_summary(total_items, per_page):\n    if total_items == 0:\n        return (0, 0)\n"
            "    pages = total_items // per_page + 1\n    return (pages, total_items % per_page)",
        ),
        lambda r: ((p := r.randint(5, 12)) * r.randint(1, 6) + r.choice([0, 0, 1, 3]), p),
    ),
    Spec(
        "max_depth",
        "def max_depth(text: str) -> int:",
        "Return the maximum nesting depth of parentheses in text, which is balanced. Text with "
        "no parentheses has depth 0.",
        "def max_depth(text):\n    d = best = 0\n    for ch in text:\n        if ch == '(':\n            d += 1\n"
        "            best = max(best, d)\n        elif ch == ')':\n            d -= 1\n    return best",
        (
            "def max_depth(text):\n    return text.count('(')",
            "def max_depth(text):\n    d = best = 0\n    for ch in text:\n        best = max(best, d)\n"
            "        if ch == '(':\n            d += 1\n        elif ch == ')':\n            d -= 1\n    return best",
        ),
        lambda r: (_parens(r),),
    ),
    Spec(
        "binary_gap",
        "def binary_gap(n: int) -> int:",
        "Return the length of the longest run of 0 bits that has a 1 bit on both sides in the "
        "binary form of n (0 if there is none).",
        "def binary_gap(n):\n    s = bin(n)[2:].strip('0')\n    return max((len(p) for p in s.split('1')), default=0)",
        (
            "def binary_gap(n):\n    s = bin(n)[2:]\n    return max((len(p) for p in s.split('1')), default=0)",
            "def binary_gap(n):\n    s = bin(n)[2:].strip('0')\n    return max((len(p) for p in s.split('0')), default=0)",
        ),
        lambda r: (r.randint(9, 1100),),
    ),
    Spec(
        "weighted_mean",
        "def weighted_mean(values: list[int], weights: list[int]) -> float:",
        "Return sum(value * weight) / sum(weights), rounded to 3 decimal places.",
        "def weighted_mean(values, weights):\n    return round(sum(v * w for v, w in zip(values, weights)) / sum(weights), 3)",
        (
            "def weighted_mean(values, weights):\n    return round(sum(v * w for v, w in zip(values, weights)) / len(weights), 3)",
            "def weighted_mean(values, weights):\n    return round(sum(values) / len(values), 3)",
        ),
        _weighted_args,
    ),
    Spec(
        "full_years",
        "def full_years(born: str, on: str) -> int:",
        "Return the number of complete years between the ISO dates born and on (an age). A "
        "birthday that falls on the date itself counts as completed.",
        "def full_years(born, on):\n    from datetime import date\n    b, d = date.fromisoformat(born), date.fromisoformat(on)\n"
        "    return d.year - b.year - ((d.month, d.day) < (b.month, b.day))",
        (
            "def full_years(born, on):\n    return int(on[:4]) - int(born[:4])",
            "def full_years(born, on):\n    from datetime import date\n    b, d = date.fromisoformat(born), date.fromisoformat(on)\n"
            "    return d.year - b.year - ((d.month, d.day) <= (b.month, b.day))",
            "def full_years(born, on):\n    from datetime import date\n"
            "    return (date.fromisoformat(on) - date.fromisoformat(born)).days // 365",
        ),
        lambda r: (
            (b := _date(r, r.randint(1950, 2005))),
            f"{int(b[:4]) + r.randint(10, 40)}{b[4:]}"
            if r.random() < 0.3
            else _date(r, int(b[:4]) + r.randint(10, 40)),
        ),
    ),
    Spec(
        "histogram",
        "def histogram(values: list[int], width: int) -> list[int]:",
        "Count values into bins [0, width), [width, 2*width), and so on, up to the bin that "
        "holds the largest value. Values are non-negative.",
        "def histogram(values, width):\n    bins = [0] * (max(values) // width + 1)\n    for v in values:\n"
        "        bins[v // width] += 1\n    return bins",
        (
            "def histogram(values, width):\n    import math\n    bins = [0] * max(1, math.ceil(max(values) / width))\n"
            "    for v in values:\n        bins[min(max(v - 1, 0) // width, len(bins) - 1)] += 1\n    return bins",
            "def histogram(values, width):\n    bins = [0] * (max(values) // width)\n    for v in values:\n"
            "        if v // width < len(bins):\n            bins[v // width] += 1\n    return bins",
        ),
        lambda r: (
            [r.choice([0, 5, 10]) + r.randint(0, 14) for _ in range(r.randint(5, 8))],
            r.choice([5, 10]),
        ),
    ),
    Spec(
        "snake_case",
        "def snake_case(name: str) -> str:",
        "Convert a CamelCase identifier to snake_case. A run of capitals is one word, except "
        "that its last capital starts the next word when a lowercase letter follows "
        "(HTTPServer gives http_server).",
        "def snake_case(name):\n    import re\n    s = re.sub(r'([A-Z]+)([A-Z][a-z])', r'\\1_\\2', name)\n"
        "    s = re.sub(r'([a-z0-9])([A-Z])', r'\\1_\\2', s)\n    return s.lower()",
        (
            "def snake_case(name):\n    import re\n    return re.sub(r'(?<!^)([A-Z])', r'_\\1', name).lower()",
            "def snake_case(name):\n    import re\n    return re.sub(r'([a-z0-9])([A-Z])', r'\\1_\\2', name).lower()",
        ),
        lambda r: (
            r.choice(
                [
                    "HTTPServerError",
                    "parseJSONData",
                    "UserID",
                    "XMLHttpRequest",
                    "getHTTPSUrl",
                    "SimpleName",
                    "IOError",
                    "loadCSVFile2",
                    "MyAPIKey",
                    "PDFReaderV2",
                ]
            ),
        ),
    ),
    Spec(
        "percent_change",
        "def percent_change(old: float, new: float) -> float:",
        "Return the percentage change from old to new, (new - old) / old * 100, rounded to 1 "
        "decimal place.",
        "def percent_change(old, new):\n    return round((new - old) / old * 100, 1)",
        (
            "def percent_change(old, new):\n    return round((new - old) / new * 100, 1)",
            "def percent_change(old, new):\n    return round(new / old * 100, 1)",
            "def percent_change(old, new):\n    return round((old - new) / old * 100, 1)",
        ),
        lambda r: (r.randint(20, 400), r.randint(10, 500)),
    ),
)


RUNNER_SCRIPT = """\
import copy
import json

SOURCES = json.loads({sources!r})
ARGS = json.loads({args!r})
results = []
for args in ARGS:
    row = []
    for source in SOURCES:
        namespace = {{}}
        exec(source, namespace)
        fn = namespace[{name!r}]
        try:
            row.append(repr(fn(*copy.deepcopy(args))))
        except Exception as error:
            row.append("!" + type(error).__name__)
    results.append(row)
print(json.dumps(results))
"""


@dataclass(frozen=True)
class _Candidate:
    spec: Spec
    label: bool
    value: str
    line: str


def _literal(value: object) -> str:
    return repr(value)


def is_round(value_repr: str) -> bool:
    nums = numbers(value_repr)
    return bool(nums) and all(float(n).is_integer() and int(float(n)) % 5 == 0 for n in nums)


def assertion(spec: Spec, args: tuple[object, ...], value_repr: str) -> str:
    call = ", ".join(_literal(a) for a in args)
    return f"assert {spec.name}({call}) == {value_repr}"


def build_expected_value(ctx: BuildContext) -> list[Item]:
    family = "expected-value"
    runner = runner_for(ctx, family)
    rng = random.Random(f"{ctx.seed}:{family}")
    jobs = []
    plans: list[tuple[Spec, list[tuple[object, ...]]]] = []
    for spec in SPECS:
        spec_rng = random.Random(f"{ctx.seed}:{family}:{spec.name}")
        arg_sets: list[tuple[object, ...]] = []
        seen: set[str] = set()
        for _ in range(INSTANCES_PER_SPEC * 4):
            args = spec.args(spec_rng)
            key = repr(args)
            if key not in seen:
                seen.add(key)
                arg_sets.append(args)
            if len(arg_sets) >= INSTANCES_PER_SPEC:
                break
        script = RUNNER_SCRIPT.format(
            sources=json.dumps([spec.reference, *spec.slips]),
            args=json.dumps([list(a) for a in arg_sets]),
            name=spec.name,
        )
        jobs.append(Job(files={"run.py": script}, argv=("python", "run.py"), timeout=20.0))
        plans.append((spec, arg_sets))
    results = runner.run_many(jobs)
    pool: list[_Candidate] = []
    for (spec, arg_sets), result in zip(plans, results, strict=True):
        if not result.ok:
            continue
        rows = json.loads(result.stdout)
        for args, row in zip(arg_sets, rows, strict=True):
            truth, *slips = row
            if truth.startswith("!"):
                continue
            wrong = [s for s in dict.fromkeys(slips) if s != truth and not s.startswith("!")]
            label = rng.random() < 0.5 if wrong else True
            value = truth if label else rng.choice(wrong)
            line = assertion(spec, args, value)
            pool.append(_Candidate(spec, label, value, line))
    if not pool:
        return []
    length_cut = above_median([len(c.line) for c in pool])
    chosen = balance_noul(
        pool,
        lambda c: c.label,
        lambda c: (is_round(c.value), length_cut(len(c.line))),
        ctx.per_family,
        rng,
        group=lambda c: c.spec.name,
    )
    final_cut = above_median([len(c.line) for c in chosen])
    items = []
    for c in chosen:
        spec = c.spec
        doc = _wrap(spec.doc)
        state = (
            "A function's specification (its implementation is not shown) and a test "
            "assertion for it:\n\n"
            f'```python\n{spec.signature}\n    """{doc}"""\n    ...\n\n\n{c.line}\n```\n'
        )
        items.append(
            make_item(
                family=family,
                key=f"{ctx.seed}:{c.line}",
                source_unit=f"{family}:{spec.name}",
                state=state,
                question=noul_question(
                    "Is the expected value in this assertion exactly what a correct "
                    "implementation of the docstring returns?"
                ),
                answer=c.label,
                reference="execution",
                shortcuts={
                    "round_value": "true" if is_round(c.value) else "false",
                    "assertion_above_median": "true" if final_cut(len(c.line)) else "false",
                },
                source={"spec": spec.name},
            )
        )
    return items


def _wrap(text: str, width: int = 84) -> str:
    words = text.split()
    lines: list[str] = []
    current = ""
    for word in words:
        if current and len(current) + 1 + len(word) > width:
            lines.append(current)
            current = word
        else:
            current = f"{current} {word}".strip()
    lines.append(current)
    return "\n    ".join(lines)
