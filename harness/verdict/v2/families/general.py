"""General-pillar families for Verdict v2: logic, math, tables and policy.

Eight families, all generated from a ``random.Random`` seeded by the build
seed and the family id, so a build is reproducible and no item exists
anywhere else. Every answer comes from the generator's own solver, model
checker or rule engine, never from a model.

Balancing: choice answers sit at a cycling position, ``noul`` items
alternate true and false, and ``score`` items cycle through the levels. For
each item the builder makes several candidates and keeps the one that holds
every recorded shortcut baseline closest to chance (``_Balancer``).

``generate(family, ctx)`` returns each item with the structured spec it was
built from, so the tests can re-check answers with an independent solver.
``BUILDERS`` is what the registry imports.
"""

from __future__ import annotations

import functools
import itertools
import math
import random
import re
import statistics
from collections import Counter
from collections.abc import Callable, Sequence
from dataclasses import dataclass, field
from fractions import Fraction
from typing import Any

from harness.verdict.v2.items import (
    Item,
    answer_label,
    choice_question,
    labels_for,
    make_item,
    noul_question,
    score_question,
)
from harness.verdict.v2.registry import BuildContext

REFERENCE = "generator"
MIN_UNITS = 40
ITEMS_PER_UNIT = 5
LETTERS = "ABCDEFGHIJKLMNOPQRSTUVWXYZ"
CANDIDATE_TRIES = 400


# --------------------------------------------------------------------------
# Shared plumbing
# --------------------------------------------------------------------------


@dataclass
class Draft:
    """An item before its id and split are fixed, plus the spec tests re-check."""

    state: str
    question: dict[str, Any]
    answer: Any
    shortcuts: dict[str, str]
    source: dict[str, Any]
    spec: dict[str, Any] = field(default_factory=dict)


def unit_plan(per_family: int) -> list[int]:
    """Seed group of each item: at least 40 groups, about five items each."""
    n_units = min(per_family, max(MIN_UNITS, math.ceil(per_family / ITEMS_PER_UNIT)))
    return [i * n_units // per_family for i in range(per_family)]


def _family_rng(ctx: BuildContext, family: str) -> random.Random:
    return random.Random(f"verdict-v2:{ctx.seed}:{family}")


class _Balancer:
    """Chooses among candidate drafts so each shortcut tracks chance.

    It keeps, per shortcut, the running count of items the shortcut would
    get right and the running sum of chance rates, and picks the candidate
    that keeps the total absolute gap smallest.
    """

    def __init__(self) -> None:
        self.hits: Counter[str] = Counter()
        self.expected = 0.0

    def cost(self, draft: Draft) -> float:
        truth = answer_label(draft.question, draft.answer)
        target = self.expected + 1 / len(labels_for(draft.question))
        return sum(
            abs(self.hits[name] + (guess == truth) - target)
            for name, guess in draft.shortcuts.items()
        )

    def pick(
        self, drafts: Sequence[Draft], penalty: Callable[[Draft], float] | None = None
    ) -> Draft:
        if not drafts:
            raise RuntimeError("no candidate drafts to pick from")
        best = min(drafts, key=lambda d: self.cost(d) + (penalty(d) if penalty else 0.0))
        truth = answer_label(best.question, best.answer)
        for name, guess in best.shortcuts.items():
            self.hits[name] += int(guess == truth)
        self.expected += 1 / len(labels_for(best.question))
        return best


def _collect(
    make: Callable[[], Draft | None], want: int, tries: int = CANDIDATE_TRIES
) -> list[Draft]:
    out: list[Draft] = []
    for _ in range(tries):
        draft = make()
        if draft is not None:
            out.append(draft)
            if len(out) >= want:
                break
    if not out:
        raise RuntimeError("generator produced no valid candidate")
    return out


def _cap(text: str) -> str:
    return text[:1].upper() + text[1:]


def _article(word: str) -> str:
    return "an" if word[:1].lower() in "aeiou" else "a"


def _join_and(parts: Sequence[str]) -> str:
    if len(parts) == 1:
        return parts[0]
    return ", ".join(parts[:-1]) + " and " + parts[-1]


def _place(
    rng: random.Random, correct: str, distractors: Sequence[str], position: int
) -> tuple[dict[str, str], str]:
    """Letter-labelled options with the correct text at ``position``."""
    others = list(distractors)
    rng.shuffle(others)
    texts = [*others[:position], correct, *others[position:]]
    if len(set(texts)) != len(texts):
        raise ValueError("options must be distinct")
    labels = LETTERS[: len(texts)]
    return {labels[i]: text for i, text in enumerate(texts)}, labels[position]


def _add_length_shortcut(drafts: Sequence[Draft], statement: Callable[[Draft], str]) -> None:
    """Noul shortcut: say true when the statement is longer than the median."""
    median = statistics.median(len(statement(d)) for d in drafts)
    for d in drafts:
        d.shortcuts["long_statement"] = "true" if len(statement(d)) > median else "false"


# Nonsense words. A small blocklist keeps common English words out.
_ONSETS = (
    "b", "br", "d", "dr", "f", "fl", "g", "gl", "gr", "k", "kl", "kr", "l", "m", "n",
    "p", "pl", "pr", "r", "s", "sk", "sl", "sn", "st", "t", "tr", "v", "vr", "z",
)  # fmt: skip
_VOWELS = ("a", "e", "i", "o", "u", "a", "e", "o", "ai", "ou", "ee", "y")
_CODAS = ("", "", "", "n", "r", "l", "k", "m", "sk", "nd", "rn", "lt", "x", "st")
_STEM_CODAS = ("n", "m", "k", "p", "t", "d", "g", "l", "r", "nd", "mp", "rk", "lt")
_BLOCK = frozenset(
    [
        "bake",
        "base",
        "bike",
        "bone",
        "bore",
        "care",
        "case",
        "core",
        "dame",
        "dare",
        "dome",
        "done",
        "fade",
        "fame",
        "fare",
        "fate",
        "game",
        "gate",
        "gone",
        "gore",
        "hide",
        "kite",
        "lake",
        "late",
        "like",
        "lore",
        "made",
        "make",
        "mane",
        "mare",
        "mate",
        "mile",
        "mole",
        "more",
        "name",
        "nine",
        "none",
        "pale",
        "pane",
        "pike",
        "pole",
        "pore",
        "rake",
        "rare",
        "rate",
        "ride",
        "role",
        "rose",
        "safe",
        "sale",
        "same",
        "side",
        "sore",
        "take",
        "tale",
        "tame",
        "tide",
        "time",
        "tone",
        "tore",
        "vale",
        "vase",
        "vote",
        "wake",
        "ware",
        "wide",
        "zone",
        "salt",
        "melt",
        "bolt",
        "kilt",
        "silk",
        "dusk",
        "desk",
        "disk",
        "risk",
        "mask",
        "task",
        "tusk",
        "band",
        "bend",
        "bond",
        "fond",
        "fund",
        "land",
        "lend",
        "mend",
        "pond",
        "sand",
        "send",
        "tend",
        "wand",
        "wind",
        "brand",
        "grand",
        "stand",
        "drum",
        "from",
        "grim",
        "prom",
        "slim",
        "trim",
        "skim",
        "swim",
        "stem",
        "spin",
        "grin",
        "twin",
        "gun",
        "sun",
        "run",
        "fun",
        "bun",
        "stop",
        "drop",
        "trap",
        "slap",
        "plan",
        "plot",
        "spot",
        "slot",
        "slip",
        "trip",
        "skip",
        "snap",
        "stun",
        "grab",
        "grip",
        "flat",
        "flip",
        "plum",
        "glad",
        "glum",
        "bred",
        "sled",
        "sped",
        "trot",
        "blot",
        "clot",
        "snag",
        "drag",
        "brag",
        "flag",
        "slag",
        "stag",
        "grin",
        "grit",
        "slit",
        "spit",
        "knit",
        "trek",
        "pelt",
        "belt",
        "felt",
        "kelt",
        "dent",
        "rent",
        "tent",
        "vent",
        "went",
        "bent",
        "sent",
        "lent",
        "pant",
        "rant",
        "want",
        "pint",
        "mint",
        "lint",
        "tint",
        "hint",
        "bump",
        "dump",
        "lump",
        "pump",
        "rump",
        "sump",
        "dark",
        "bark",
        "lark",
        "mark",
        "park",
        "fork",
        "pork",
        "cork",
        "lurk",
        "murk",
        "silt",
        "tilt",
        "gilt",
        "hilt",
        "wilt",
        "kind",
        "mind",
        "find",
        "bind",
        "rind",
        "wild",
        "mild",
        "gold",
        "bold",
        "fold",
        "hold",
        "told",
        "sold",
        "cold",
        "mold",
    ]
)


class _Namer:
    def __init__(self, rng: random.Random) -> None:
        self.rng = rng
        self.used: set[str] = set()

    def _fresh(self, make: Callable[[], str]) -> str:
        for _ in range(2000):
            word = make()
            if word not in self.used and word not in _BLOCK:
                self.used.add(word)
                return word
        raise RuntimeError("nonsense name space exhausted")

    def word(self, syllables: int = 2) -> str:
        rng = self.rng

        def make() -> str:
            body = "".join(rng.choice(_ONSETS) + rng.choice(_VOWELS) for _ in range(syllables))
            word = body + rng.choice(_CODAS)
            return word if 4 <= len(word) <= 10 else ""

        return self._fresh(lambda: make() or make() or "zzzz")

    def words(self, n: int, syllables: int = 2) -> list[str]:
        return [self.word(syllables) for _ in range(n)]

    def names(self, n: int, syllables: int = 2) -> list[str]:
        return [w.capitalize() for w in self.words(n, syllables)]

    def fixed(self, n: int) -> list[str]:
        """Four-letter consonant-vowel words, so option texts have equal length."""
        rng = self.rng
        cons, vows = "bdfgklmnprstvz", "aeiou"
        return [
            self._fresh(lambda: "".join(rng.choice(cons) + rng.choice(vows) for _ in range(2)))
            for _ in range(n)
        ]

    def stem(self) -> str:
        """A one-syllable verb stem that takes a plain -s."""
        rng = self.rng
        return self._fresh(
            lambda: rng.choice(_ONSETS) + rng.choice("aeiou") + rng.choice(_STEM_CODAS)
        )


# --------------------------------------------------------------------------
# constraint-pick
# --------------------------------------------------------------------------

Rule = tuple[str, tuple[int, ...]]
Arrangement = tuple[tuple[int, ...], tuple[int, ...]]  # (slot of entity, badge of entity)

_CP_FRAMES: tuple[dict[str, str], ...] = (
    {
        "id": "windows",
        "intro": "{count} couriers, {names}, each have a different delivery window. "
        "The windows are numbered 1 (earliest) to {n} (latest).",
        "person": "courier",
        "slot": "Window",
        "before": "{a} has an earlier window than {b}",
        "imm": "{a}'s window comes immediately before {b}'s",
        "adj": "{a} and {b} have neighbouring windows",
        "nadj": "{a} and {b} do not have neighbouring windows",
        "gap": "there {are} exactly {k} other window{pl} between {a}'s window and {b}'s",
        "at": "{a} has window {s}",
        "not_at": "{a} does not have window {s}",
        "at_end": "{a} has window 1 or window {n}",
        "even": "{a} has an even-numbered window",
        "holder": "the courier with window {s}",
        "group_before": "has an earlier window than",
    },
    {
        "id": "stalls",
        "intro": "{count} traders, {names}, each rent a different stall in a single row. "
        "The stalls are numbered 1 (westmost) to {n} (eastmost).",
        "person": "trader",
        "slot": "Stall",
        "before": "{a}'s stall is west of {b}'s",
        "imm": "{a}'s stall is immediately west of {b}'s",
        "adj": "{a} and {b} rent stalls next to each other",
        "nadj": "{a} and {b} do not rent stalls next to each other",
        "gap": "there {are} exactly {k} other stall{pl} between {a}'s stall and {b}'s",
        "at": "{a} rents stall {s}",
        "not_at": "{a} does not rent stall {s}",
        "at_end": "{a} rents stall 1 or stall {n}",
        "even": "{a} rents an even-numbered stall",
        "holder": "the trader in stall {s}",
        "group_before": "has a stall west of the stall of",
    },
    {
        "id": "floors",
        "intro": "{count} tenants, {names}, each live on a different floor of a tower. "
        "The floors are numbered 1 (lowest) to {n} (highest).",
        "person": "tenant",
        "slot": "Floor",
        "before": "{a} lives on a lower floor than {b}",
        "imm": "{a} lives on the floor directly below {b}'s",
        "adj": "{a} and {b} live on adjacent floors",
        "nadj": "{a} and {b} do not live on adjacent floors",
        "gap": "there {are} exactly {k} other floor{pl} between {a}'s floor and {b}'s",
        "at": "{a} lives on floor {s}",
        "not_at": "{a} does not live on floor {s}",
        "at_end": "{a} lives on floor 1 or floor {n}",
        "even": "{a} lives on an even-numbered floor",
        "holder": "the tenant on floor {s}",
        "group_before": "lives on a lower floor than",
    },
    {
        "id": "race",
        "intro": "{count} runners, {names}, finish a race with no ties. "
        "The places are numbered 1 (first to finish) to {n} (last).",
        "person": "runner",
        "slot": "Place",
        "before": "{a} finishes ahead of {b}",
        "imm": "{a} finishes immediately ahead of {b}",
        "adj": "{a} and {b} finish in consecutive places",
        "nadj": "{a} and {b} do not finish in consecutive places",
        "gap": "there {are} exactly {k} other runner{pl} finishing between {a} and {b}",
        "at": "{a} finishes in place {s}",
        "not_at": "{a} does not finish in place {s}",
        "at_end": "{a} finishes in place 1 or place {n}",
        "even": "{a} finishes in an even-numbered place",
        "holder": "the runner in place {s}",
        "group_before": "finishes ahead of",
    },
)
_CP_BADGES = (
    ("pennant", "carries", "carry"),
    ("banner", "flies", "fly"),
    ("lantern", "holds", "hold"),
    ("sash", "wears", "wear"),
)
_CP_POS_KINDS = ("before", "imm", "adj", "nadj", "gap", "not_at", "at_end", "cond_even", "xor_end")
_CP_BADGE_KINDS = ("badge_is", "badge_diff", "badge_order", "holder_not", "count_max", "cond_badge")
CP_UNARY = frozenset({"not_at", "at_end", "badge_is", "holder_not"})


def cp_holds(rule: Rule, arr: Arrangement, n: int) -> bool:  # noqa: PLR0911, PLR0912, one branch per rule kind
    """Truth of one constraint rule in an arrangement (slots are 1-based)."""
    kind, a = rule
    slot, badge = arr
    if kind == "before":
        return slot[a[0]] < slot[a[1]]
    if kind == "imm":
        return slot[a[1]] == slot[a[0]] + 1
    if kind == "adj":
        return abs(slot[a[0]] - slot[a[1]]) == 1
    if kind == "nadj":
        return abs(slot[a[0]] - slot[a[1]]) != 1
    if kind == "gap":
        return abs(slot[a[0]] - slot[a[1]]) == a[2] + 1
    if kind == "not_at":
        return slot[a[0]] != a[1]
    if kind == "at_end":
        return slot[a[0]] in (1, n)
    if kind == "cond_even":
        return slot[a[0]] % 2 == 1 or slot[a[1]] < slot[a[2]]
    if kind == "xor_end":
        return (slot[a[0]] == 1) != (slot[a[1]] == n)
    if kind == "badge_is":
        return badge[a[0]] == a[1]
    if kind == "badge_diff":
        return badge[a[0]] != badge[a[1]]
    if kind == "badge_order":
        early = [slot[e] for e in range(n) if badge[e] == a[0]]
        late = [slot[e] for e in range(n) if badge[e] == a[1]]
        return all(x < y for x in early for y in late)
    if kind == "holder_not":
        return badge[slot.index(a[0])] != a[1]
    if kind == "count_max":
        return sum(b == a[0] for b in badge) <= a[1]
    if kind == "cond_badge":
        return badge[a[0]] != a[1] or slot[a[2]] < slot[a[3]]
    raise ValueError(f"unknown rule kind {kind}")


def _cp_vacuous(rule: Rule, options: Sequence[Arrangement], n: int) -> bool:
    """Group-order rules are only used when both groups exist in every option."""
    if rule[0] != "badge_order":
        return False
    return any(
        not any(b == rule[1][0] for b in badge) or not any(b == rule[1][1] for b in badge)
        for _, badge in options
    )


def _cp_random_rule(rng: random.Random, n: int, nb: int) -> Rule:  # noqa: PLR0911, one branch per rule kind
    kinds = _CP_POS_KINDS + (_CP_BADGE_KINDS if nb else ())
    kind = rng.choice(kinds)
    e = rng.sample(range(n), 3)
    if kind in ("before", "imm", "adj", "nadj", "xor_end", "badge_diff"):
        return kind, (e[0], e[1])
    if kind == "gap":
        return kind, (e[0], e[1], rng.randint(1, n - 2))
    if kind == "not_at":
        return kind, (e[0], rng.randint(1, n))
    if kind == "at_end":
        return kind, (e[0],)
    if kind == "cond_even":
        return kind, (e[0], e[1], e[2])
    if kind == "badge_is":
        return kind, (e[0], rng.randrange(nb))
    if kind == "badge_order":
        v, w = rng.sample(range(nb), 2)
        return kind, (v, w)
    if kind == "holder_not":
        return kind, (rng.randint(1, n), rng.randrange(nb))
    if kind == "count_max":
        return kind, (rng.randrange(nb), rng.randint(1, n - 2))
    return kind, (e[0], rng.randrange(nb), e[1], e[2])  # cond_badge


def _cp_phrase(rule: Rule, world: dict[str, Any]) -> str:  # noqa: PLR0911, one branch per rule kind
    kind, a = rule
    fr: dict[str, str] = world["frame"]
    names: list[str] = world["names"]
    n: int = world["n"]
    if kind in ("before", "imm", "adj", "nadj"):
        return fr[kind].format(a=names[a[0]], b=names[a[1]])
    if kind == "gap":
        k = a[2]
        return fr["gap"].format(
            a=names[a[0]],
            b=names[a[1]],
            k=k,
            pl="" if k == 1 else "s",
            are="is" if k == 1 else "are",
        )
    if kind == "not_at":
        return fr["not_at"].format(a=names[a[0]], s=a[1])
    if kind == "at_end":
        return fr["at_end"].format(a=names[a[0]], n=n)
    if kind == "cond_even":
        return (
            f"if {fr['even'].format(a=names[a[0]])}, "
            f"then {fr['before'].format(a=names[a[1]], b=names[a[2]])}"
        )
    if kind == "xor_end":
        return (
            f"either {fr['at'].format(a=names[a[0]], s=1)} "
            f"or {fr['at'].format(a=names[a[1]], s=n)}, but not both"
        )
    noun, verb3, verb = world["badge"]
    vals: list[str] = world["bvals"]
    if kind == "badge_is":
        return f"{names[a[0]]} {verb3} the {vals[a[1]]} {noun}"
    if kind == "badge_diff":
        return f"{names[a[0]]} and {names[a[1]]} do not {verb} the same kind of {noun}"
    if kind == "badge_order":
        return (
            f"everyone who {verb3} the {vals[a[0]]} {noun} {fr['group_before']} "
            f"everyone who {verb3} the {vals[a[1]]} {noun}"
        )
    if kind == "holder_not":
        return f"{fr['holder'].format(s=a[0])} does not {verb} the {vals[a[1]]} {noun}"
    if kind == "count_max":
        return f"at most {a[1]} of the {fr['person']}s {verb} the {vals[a[0]]} {noun}"
    if kind == "cond_badge":
        return (
            f"if {names[a[0]]} {verb3} the {vals[a[1]]} {noun}, "
            f"then {fr['before'].format(a=names[a[2]], b=names[a[3]])}"
        )
    raise ValueError(kind)


def _cp_describe(arr: Arrangement, world: dict[str, Any]) -> str:
    slot, badge = arr
    names: list[str] = world["names"]
    by_slot = sorted(range(world["n"]), key=lambda e: slot[e])
    parts = []
    for e in by_slot:
        part = f"{world['frame']['slot']} {slot[e]}: {names[e]}"
        if world["nb"]:
            part += f" ({world['bvals'][badge[e]]})"
        parts.append(part)
    return "; ".join(parts)


def _cp_perturb(rng: random.Random, arr: Arrangement, n: int, nb: int) -> Arrangement:
    slot, badge_t = arr
    order = sorted(range(n), key=lambda e: slot[e])
    badge = list(badge_t)
    for _ in range(rng.choice((1, 1, 2))):
        op = rng.choice(("swap", "move", "badge") if nb else ("swap", "move"))
        if op == "swap":
            i, j = rng.sample(range(n), 2)
            order[i], order[j] = order[j], order[i]
        elif op == "move":
            i, j = rng.sample(range(n), 2)
            order.insert(j, order.pop(i))
        else:
            e = rng.randrange(n)
            badge[e] = rng.choice([v for v in range(nb) if v != badge[e]])
    new_slot = [0] * n
    for pos, e in enumerate(order):
        new_slot[e] = pos + 1
    return tuple(new_slot), tuple(badge)


def _cp_world(rng: random.Random) -> dict[str, Any]:
    namer = _Namer(rng)
    n = rng.choice((4, 5, 5, 6, 6))
    nb = rng.choice((0, 2, 3, 3))
    return {
        "frame": rng.choice(_CP_FRAMES),
        "n": n,
        "nb": nb,
        "names": namer.names(n),
        "badge": rng.choice(_CP_BADGES),
        "bvals": namer.fixed(nb),
    }


def _cp_hamming(x: Arrangement, y: Arrangement) -> int:
    return sum(a != b for a, b in zip(x[0], y[0], strict=True)) + sum(
        a != b for a, b in zip(x[1], y[1], strict=True)
    )


def _cp_candidate(rng: random.Random, world: dict[str, Any], position: int) -> Draft | None:
    n, nb = world["n"], world["nb"]
    order = rng.sample(range(n), n)
    base_slot = [0] * n
    for pos, e in enumerate(order):
        base_slot[e] = pos + 1
    base: Arrangement = (tuple(base_slot), tuple(rng.randrange(nb) for _ in range(n)) if nb else ())
    options: list[Arrangement] = [base] if rng.random() < 0.5 else []
    for _ in range(60):
        if len(options) == 4:
            break
        other = _cp_perturb(rng, base, n, nb)
        if other not in options:
            options.append(other)
    if len(options) < 4:
        return None
    rng.shuffle(options)
    solution, distractors = options[0], options[1:]
    pool = list(dict.fromkeys(_cp_random_rule(rng, n, nb) for _ in range(320)))
    usable = [r for r in pool if not _cp_vacuous(r, options, n)]
    truth = {r: [cp_holds(r, o, n) for o in options] for r in usable}
    killers: list[Rule] = []
    used_kinds: set[str] = set()
    for d in range(1, 4):
        eligible = [
            r
            for r in usable
            if r[0] not in CP_UNARY
            and r not in killers
            and truth[r][0]
            and not truth[r][d]
            and all(truth[r][o] for o in range(1, 4) if o != d)
        ]
        if not eligible:
            return None
        fresh = [r for r in eligible if r[0] not in used_kinds] or eligible
        rule = rng.choice(fresh)
        killers.append(rule)
        used_kinds.add(rule[0])
    total = rng.randint(5, 8)
    filler_pool = [r for r in usable if all(truth[r]) and r not in killers]
    if len(filler_pool) < total - 3:
        return None
    rules = killers + rng.sample(filler_pool, total - 3)
    rng.shuffle(rules)
    texts = [_cap(_cp_phrase(r, world)) + "." for r in rules]
    if len(set(texts)) != len(texts):
        return None

    fr = world["frame"]
    descriptions, answer = _place(
        rng, _cp_describe(solution, world), [_cp_describe(d, world) for d in distractors], position
    )
    by_text = {_cp_describe(o, world): o for o in options}
    arr_of = {label: by_text[text] for label, text in descriptions.items()}
    labels = list(descriptions)
    medoid = min(labels, key=lambda lab: sum(_cp_hamming(arr_of[lab], arr_of[o]) for o in labels))
    longest = max(labels, key=lambda lab: len(descriptions[lab]))

    lines = [fr["intro"].format(count=_cap(_number_word(n)), names=_join_and(world["names"]), n=n)]
    if nb:
        noun, verb3, _ = world["badge"]
        kinds = _join_and(world["bvals"]).replace(" and ", " or ")
        lines.append(f"Each {fr['person']} also {verb3} exactly one {noun}, of kind {kinds}.")
    lines += ["", "Rules:"] + [f"{i}. {t}" for i, t in enumerate(texts, 1)]
    note = f"Each option lists the {fr['slot'].lower()}s from 1 to {n}"
    if nb:
        note += f", with each {fr['person']}'s {world['badge'][0]} in brackets"
    lines += ["", note + "."]
    return Draft(
        state="\n".join(lines),
        question=choice_question("Which option satisfies every rule?", dict(descriptions)),
        answer=answer,
        shortcuts={"first_option": labels[0], "longest_option": longest, "medoid_option": medoid},
        source={
            "template": fr["id"],
            "entities": n,
            "badge_kinds": nb,
            "rules": len(rules),
            "killer_kinds": sorted(r[0] for r in killers),
        },
        spec={
            "n": n,
            "nb": nb,
            "names": world["names"],
            "bvals": world["bvals"],
            "rules": rules,
            "options": arr_of,
            "killers": killers,
        },
    )


def _number_word(n: int) -> str:
    return (
        ["zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten"]
    )[n]


def _gen_constraint(ctx: BuildContext) -> list[Draft]:
    rng = _family_rng(ctx, "constraint-pick")
    worlds: dict[int, dict[str, Any]] = {}
    balancer = _Balancer()
    drafts = []
    for i, g in enumerate(unit_plan(ctx.per_family)):
        world = worlds.setdefault(g, _cp_world(rng))
        drafts.append(
            balancer.pick(_collect(functools.partial(_cp_candidate, rng, world, i % 4), 4))
        )
    return drafts


# --------------------------------------------------------------------------
# entailment
# --------------------------------------------------------------------------

Lit = tuple[int, bool]
PropStmt = tuple[str, tuple[Lit, ...]]

PROP_TEXT = {
    "if": "If {0}, then {1}.",
    "only_if": "{0} only if {1}.",
    "unless": "{0} unless {1}.",
    "or": "Either {0} or {1}, or both.",
    "xor": "Either {0} or {1}, but not both.",
    "nand": "It is not the case that both {0} and {1}.",
    "iff": "{0} if and only if {1}.",
    "if_and": "If {0} and {1}, then {2}.",
    "if_or": "If {0}, then {1} or {2} (or both).",
    "fact": "{0}.",
}
_PROP_FORMS = ("imp", "or", "lit", "nand")
_MON_FORMS = ("all_pos", "some_neg", "all_neg", "some_pos")
_PROP_PREAMBLE = (
    "Each sentence about the objects below is either true or false. "
    '"P unless Q" means: if Q is false, then P is true. '
    '"P only if Q" means: if P is true, then Q is true.'
)
_MON_PREAMBLE = (
    "The statements below are about the things in one closed collection, which "
    'contains at least one thing. "Every X is a Y" does not by itself say that any X '
    'exists. "Some X is a Y" means at least one thing is both an X and a Y.'
)


def prop_truth(stmt: PropStmt, val: Sequence[bool]) -> bool:  # noqa: PLR0911, one branch per wording
    """Truth of a propositional statement under a valuation of the atoms."""
    style, lits = stmt
    v = [val[a] == pos for a, pos in lits]
    if style in ("if", "only_if"):
        return not v[0] or v[1]
    if style in ("unless", "or"):
        return v[0] or v[1]
    if style == "xor":
        return v[0] != v[1]
    if style == "nand":
        return not (v[0] and v[1])
    if style == "iff":
        return v[0] == v[1]
    if style == "if_and":
        return not (v[0] and v[1]) or v[2]
    if style == "if_or":
        return not v[0] or v[1] or v[2]
    if style == "fact":
        return v[0]
    raise ValueError(style)


def _neg(lit: Lit) -> Lit:
    return lit[0], not lit[1]


def _prop_phrase(lit: Lit, words: Sequence[tuple[str, str]]) -> str:
    noun, verb = words[lit[0]]
    return f"the {noun} {verb}s" if lit[1] else f"the {noun} does not {verb}"


def _prop_text(stmt: PropStmt, words: Sequence[tuple[str, str]]) -> str:
    style, lits = stmt
    return _cap(PROP_TEXT[style].format(*[_prop_phrase(lit, words) for lit in lits]))


def _prop_implication(rng: random.Random, a: Lit, b: Lit) -> PropStmt:
    """One of six wordings, all equivalent to 'if a then b'."""
    style = rng.choice(("if", "if", "only_if", "contra", "unless", "or", "nand"))
    if style == "contra":
        return "if", (_neg(b), _neg(a))
    if style == "unless":
        return "unless", (b, _neg(a))
    if style == "or":
        return "or", (_neg(a), b)
    if style == "nand":
        return "nand", (a, _neg(b))
    return style, (a, b)


def _prop_premises(rng: random.Random, k: int, need_fact: bool) -> list[PropStmt]:
    order = rng.sample(range(k), k)
    chain: list[Lit] = [(atom, rng.random() < 0.6) for atom in order]
    premises = [
        _prop_implication(rng, chain[i], chain[i + 1])
        for i in range(rng.randint(max(2, k - 2), k - 1))
    ]
    extras = rng.randint(1, 2)
    if need_fact:
        premises.append(("fact", (chain[0] if rng.random() < 0.7 else rng.choice(chain),)))
        extras -= 1
    for _ in range(extras):
        kind = rng.choice(("xor", "iff", "if_and", "if_or", "imp", "fact"))
        lits = [(atom, rng.random() < 0.5) for atom in rng.sample(range(k), 3)]
        if kind == "imp":
            premises.append(_prop_implication(rng, lits[0], lits[1]))
        elif kind in ("if_and", "if_or"):
            premises.append((kind, tuple(lits)))
        elif kind == "fact":
            premises.append(("fact", (lits[0],)))
        else:
            premises.append((kind, (lits[0], lits[1])))
    premises = premises[:6]
    rng.shuffle(premises)
    return premises


def _prop_candidates(form: str, k: int) -> list[PropStmt]:
    lits = [(a, p) for a in range(k) for p in (True, False)]
    if form == "lit":
        return [("fact", (lit,)) for lit in lits]
    if form == "imp":
        return [("if", (x, y)) for x in lits for y in lits if x[0] != y[0]]
    style = "or" if form == "or" else "nand"
    return [(style, (x, y)) for x, y in itertools.combinations(lits, 2) if x[0] != y[0]]


def _prop_mutants(stmt: PropStmt) -> list[PropStmt]:
    style, lits = stmt
    out: list[PropStmt] = []
    for i in range(len(lits)):
        changed = list(lits)
        changed[i] = _neg(changed[i])
        out.append((style, tuple(changed)))
    if style == "if":
        out.append(("if", (lits[1], lits[0])))
    return out


def _prop_models(stmts: Sequence[PropStmt], k: int) -> list[tuple[bool, ...]]:
    return [
        v
        for v in itertools.product((False, True), repeat=k)
        if all(prop_truth(s, v) for s in stmts)
    ]


@dataclass(frozen=True)
class MonStmt:
    """A monadic statement: 'all' (every thing meeting conds meets concl) or 'some'."""

    kind: str
    conds: tuple[Lit, ...]
    concl: Lit | None = None


def _msat(lit: Lit, t: int) -> bool:
    return bool((t >> lit[0]) & 1) == lit[1]


def _mforbids(q: MonStmt, t: int) -> bool:
    return (
        q.kind == "all"
        and q.concl is not None
        and all(_msat(c, t) for c in q.conds)
        and not _msat(q.concl, t)
    )


def _mwitness(q: MonStmt, t: int) -> bool:
    return q.kind == "some" and all(_msat(c, t) for c in q.conds)


def mon_satisfiable(premises: Sequence[MonStmt], k: int) -> bool:
    allowed = [t for t in range(2**k) if not any(_mforbids(q, t) for q in premises)]
    exists = [q for q in premises if q.kind == "some"]
    return bool(allowed) and all(any(_mwitness(e, t) for t in allowed) for e in exists)


def mon_entails(premises: Sequence[MonStmt], concl: MonStmt, k: int) -> bool:
    """Monadic entailment by reasoning over which predicate combinations may be inhabited."""
    allowed = [t for t in range(2**k) if not any(_mforbids(q, t) for q in premises)]
    exists = [q for q in premises if q.kind == "some"]
    if concl.kind == "all":
        room = allowed
        needs_counter = [t for t in allowed if _mforbids(concl, t)]
        counter = bool(needs_counter)
    else:
        room = [t for t in allowed if not _mwitness(concl, t)]
        counter = bool(room)
    counter = counter and all(any(_mwitness(e, t) for t in room) for e in exists)
    return not counter


def _mon_noun(p: int, nouns: Sequence[str]) -> str:
    return f"{_article(nouns[p])} {nouns[p]}"


def _mon_text(q: MonStmt, nouns: Sequence[str]) -> str:
    if q.kind == "some":
        (a, apos), (b, bpos) = q.conds
        subject = f"Some {nouns[a]}" if apos else f"Something that is not {_mon_noun(a, nouns)}"
        return f"{subject} is {'' if bpos else 'not '}{_mon_noun(b, nouns)}."
    assert q.concl is not None
    b, bpos = q.concl
    if len(q.conds) == 2:
        (a, _), (c, _) = q.conds
        head = "Every" if bpos else "No"
        return f"{head} {nouns[a]} that is {_mon_noun(c, nouns)} is {_mon_noun(b, nouns)}."
    ((a, apos),) = q.conds
    if apos:
        return f"{'Every' if bpos else 'No'} {nouns[a]} is {_mon_noun(b, nouns)}."
    head = "Everything" if bpos else "Nothing"
    return f"{head} that is not {_mon_noun(a, nouns)} is {_mon_noun(b, nouns)}."


def _mon_premises(rng: random.Random, k: int, n_exist: int) -> list[MonStmt]:
    order = rng.sample(range(k), k)
    chain: list[Lit] = [(order[0], True)] + [(p, rng.random() < 0.65) for p in order[1:]]
    premises: list[MonStmt] = []
    for i in range(rng.randint(max(2, k - 2), k - 1)):
        a, b = chain[i], chain[i + 1]
        if not a[1] and not b[1]:
            premises.append(MonStmt("all", ((b[0], True),), (a[0], True)))
        else:
            premises.append(MonStmt("all", (a,), b))
    for _ in range(n_exist):
        x, y = rng.sample(range(k), 2)
        premises.append(MonStmt("some", ((x, True), (y, rng.random() < 0.6))))
    if rng.random() < 0.45 and len(premises) < 6:
        x, y, z = rng.sample(range(k), 3)
        premises.append(MonStmt("all", ((x, True), (y, True)), (z, rng.random() < 0.6)))
    while len(premises) < 3:
        x, y = rng.sample(range(k), 2)
        premises.append(MonStmt("all", ((x, True),), (y, rng.random() < 0.5)))
    rng.shuffle(premises)
    return premises[:6]


def _mon_candidate_for(form: str, a: int, b: int) -> MonStmt:
    if form == "all_pos":
        return MonStmt("all", ((a, True),), (b, True))
    if form == "all_neg":
        return MonStmt("all", ((a, True),), (b, False))
    return MonStmt("some", ((a, True), (b, form == "some_pos")))


def _mon_pair(q: MonStmt) -> tuple[int, int]:
    if q.kind == "all":
        assert q.concl is not None
        return q.conds[0][0], q.concl[0]
    return q.conds[0][0], q.conds[1][0]


def _ent_atoms_prop(stmt: PropStmt) -> set[int]:
    return {a for a, _ in stmt[1]}


def _ent_atoms_mon(q: MonStmt) -> set[int]:
    atoms = {a for a, _ in q.conds}
    if q.concl is not None:
        atoms.add(q.concl[0])
    return atoms


_NEGATION = re.compile(r"\bnot\b|^No\b|^Nothing\b", re.IGNORECASE)


def _ent_draft(
    style: str,
    premise_texts: list[str],
    concl_text: str,
    label: bool,
    cooccur: bool,
    source: dict[str, Any],
    spec: dict[str, Any],
) -> Draft:
    preamble = _PROP_PREAMBLE if style == "prop" else _MON_PREAMBLE
    lines = [preamble, "", "Premises:"]
    lines += [f"{i}. {t}" for i, t in enumerate(premise_texts, 1)]
    lines += ["", f"Conclusion: {concl_text}"]
    return Draft(
        state="\n".join(lines),
        question=noul_question(
            "Is the conclusion guaranteed to be true whenever every premise is true?"
        ),
        answer=label,
        shortcuts={
            "negation_false": "false" if _NEGATION.search(concl_text) else "true",
            "cooccur_true": "true" if cooccur else "false",
        },
        source=source,
        spec={**spec, "conclusion_text": concl_text},
    )


def _prop_candidate(
    rng: random.Random, words: Sequence[tuple[str, str]], form: str, label: bool
) -> Draft | None:
    k = rng.randint(3, 5)
    premises = _prop_premises(rng, k, need_fact=form == "lit")
    models = _prop_models(premises, k)
    if not models or len(models) > 2 ** (k - 1):
        return None
    single = [_prop_models([p], k) for p in premises]

    def entailed(c: PropStmt) -> bool:
        return all(prop_truth(c, m) for m in models)

    candidates = _prop_candidates(form, k)
    if label:
        pool = [
            c
            for c in candidates
            if entailed(c) and not any(all(prop_truth(c, m) for m in sm) for sm in single)
        ]
    else:
        seeds = [c for c in candidates if entailed(c)]
        pool = list(dict.fromkeys(m for s in seeds for m in _prop_mutants(s) if not entailed(m)))
    if not pool:
        return None
    concl = rng.choice(pool)
    cooccur = any(_ent_atoms_prop(concl) <= _ent_atoms_prop(p) for p in premises)
    return _ent_draft(
        "prop",
        [_prop_text(p, words[:k]) for p in premises],
        _prop_text(concl, words[:k]),
        label,
        cooccur,
        {"template": f"prop-{form}", "atoms": k, "premises": len(premises)},
        {"style": "prop", "k": k, "premises": premises, "conclusion": concl},
    )


def _mon_candidate(
    rng: random.Random, nouns: Sequence[str], form: str, label: bool
) -> Draft | None:
    k = rng.randint(3, 5)
    n_exist = rng.randint(1, 2) if form.startswith("some") else rng.randint(0, 1)
    premises = _mon_premises(rng, k, n_exist)
    if not mon_satisfiable(premises, k):
        return None
    candidates = [_mon_candidate_for(form, a, b) for a in range(k) for b in range(k) if a != b]
    if label:
        pool = [
            c
            for c in candidates
            if mon_entails(premises, c, k)
            and not any(mon_entails([p], c, k) for p in premises)
            and c not in premises
        ]
    else:
        seeds = [c for c in candidates if mon_entails(premises, c, k)]
        mutants: list[MonStmt] = []
        for s in seeds:
            a, b = _mon_pair(s)
            mutants.append(_mon_candidate_for(form, b, a))
            for c in range(k):
                if c not in (a, b):
                    mutants.extend([_mon_candidate_for(form, c, b), _mon_candidate_for(form, a, c)])
        pool = list(dict.fromkeys(m for m in mutants if not mon_entails(premises, m, k)))
    if not pool:
        return None
    concl = rng.choice(pool)
    cooccur = any(_ent_atoms_mon(concl) <= _ent_atoms_mon(p) for p in premises)
    return _ent_draft(
        "mon",
        [_mon_text(p, nouns) for p in premises],
        _mon_text(concl, nouns),
        label,
        cooccur,
        {"template": f"mon-{form}", "predicates": k, "premises": len(premises)},
        {"style": "mon", "k": k, "premises": premises, "conclusion": concl},
    )


def _gen_entailment(ctx: BuildContext) -> list[Draft]:
    rng = _family_rng(ctx, "entailment")
    vocab: dict[int, dict[str, Any]] = {}
    balancer = _Balancer()
    drafts = []
    for i, g in enumerate(unit_plan(ctx.per_family)):
        if g not in vocab:
            namer = _Namer(rng)
            vocab[g] = {
                "prop": [(namer.word(), namer.stem()) for _ in range(5)],
                "mon": namer.words(5),
            }
        label = i % 2 == 0
        style = ("prop", "mon")[(i // 2) % 2]
        if style == "prop":
            form = _PROP_FORMS[(i // 4) % len(_PROP_FORMS)]
            make = functools.partial(_prop_candidate, rng, vocab[g]["prop"], form, label)
        else:
            form = _MON_FORMS[(i // 4) % len(_MON_FORMS)]
            make = functools.partial(_mon_candidate, rng, vocab[g]["mon"], form, label)
        drafts.append(balancer.pick(_collect(make, 4)))
    _add_length_shortcut(drafts, lambda d: str(d.spec["conclusion_text"]))
    return drafts


# --------------------------------------------------------------------------
# word-problem
# --------------------------------------------------------------------------


@dataclass
class _Problem:
    text: str
    correct: Fraction
    slips: dict[str, Fraction]
    money: bool
    params: dict[str, Any]


def _wp_shop(rng: random.Random, w: dict[str, Any]) -> _Problem | None:
    q1, q2 = rng.randint(2, 12), rng.randint(2, 12)
    p1, p2 = rng.sample(range(3, 61), 2)
    d = rng.choice((10, 15, 20, 25, 30, 40))
    fee = rng.randint(3, 18)
    keep = Fraction(100 - d, 100)
    sub = q1 * p1 + q2 * p2
    person, (g1, g2), cur = rng.choice(w["people"]), rng.sample(w["goods"], 2), w["cur"]
    text = (
        f"{person} buys {q1} {g1}s at {p1} {cur} each and {q2} {g2}s at {p2} {cur} each. "
        f"The shop takes {d}% off the whole order, and then adds a delivery charge of "
        f"{fee} {cur}. How many {cur} does {person} pay in total?"
    )
    return _Problem(
        text,
        sub * keep + fee,
        {
            "dropped_fee": sub * keep,
            "fee_before_discount": (sub + fee) * keep,
            "discount_on_first_only": q1 * p1 * keep + q2 * p2 + fee,
            "percent_added": sub * Fraction(100 + d, 100) + fee,
            "percent_as_amount": Fraction(sub - d + fee),
            "dropped_second_item": q1 * p1 * keep + fee,
        },
        True,
        {"q1": q1, "p1": p1, "q2": q2, "p2": p2, "d": d, "fee": fee},
    )


def _wp_trip(rng: random.Random, w: dict[str, Any]) -> _Problem | None:
    speeds = (30, 36, 40, 45, 48, 50, 54, 60, 72, 75, 80, 90)
    v1, v2 = rng.sample(speeds, 2)
    t1 = rng.choice([t for t in range(12, 150) if v1 * t % 60 == 0])
    t2 = rng.choice([t for t in range(12, 150) if v2 * t % 60 == 0])
    d1, d2 = v1 * t1 // 60, v2 * t2 // 60
    stop = rng.randint(5, 45)
    text = (
        f"A {w['vehicle']} travels {d1} km at {v1} km/h, stops for {stop} minutes, and then "
        f"travels {d2} km at {v2} km/h. How many minutes does the whole trip take?"
    )
    return _Problem(
        text,
        Fraction(t1 + stop + t2),
        {
            "dropped_stop": Fraction(t1 + t2),
            "stop_counted_twice": Fraction(t1 + t2 + 2 * stop),
            "first_leg_only": Fraction(t1 + stop),
            "speeds_swapped": Fraction(60 * d1, v2) + stop + Fraction(60 * d2, v1),
            "mean_of_speeds": Fraction(120 * (d1 + d2), v1 + v2) + stop,
            "hours_plus_minutes": Fraction(d1, v1) + Fraction(d2, v2) + stop,
        },
        False,
        {"d1": d1, "v1": v1, "d2": d2, "v2": v2, "stop": stop},
    )


def _wp_production(rng: random.Random, w: dict[str, Any]) -> _Problem | None:
    a, b = rng.randint(12, 60), rng.randint(12, 60)
    h1, h2 = rng.randint(2, 6), rng.randint(1, 5)
    p = rng.choice((5, 10, 15, 20, 25))
    total = a * h1 + (a + b) * h2
    if total * p % 100:
        return None
    keep = Fraction(100 - p, 100)
    m1, m2 = rng.sample(w["machines"], 2)
    text = (
        f"Machine {m1} makes {a} parts per hour and machine {m2} makes {b} parts per hour. "
        f"{m1} runs alone for {h1} hours, and then both machines run together for {h2} more "
        f"hours. Inspection rejects {p}% of all the parts made. How many parts pass inspection?"
    )
    return _Problem(
        text,
        total * keep,
        {
            "dropped_inspection": Fraction(total),
            "counted_rejects": Fraction(total * p, 100),
            "percent_as_count": Fraction(total - p),
            "dropped_first_machine_later": (a * h1 + b * h2) * keep,
            "both_whole_time": (a + b) * (h1 + h2) * keep,
            "together_phase_only": (a + b) * h2 * keep,
        },
        False,
        {"a": a, "b": b, "h1": h1, "h2": h2, "p": p},
    )


def _wp_posts(rng: random.Random, w: dict[str, Any]) -> _Problem | None:
    s = rng.choice((2, 3, 4, 5, 6, 8, 10, 12, 15))
    c = rng.randint(3, 25)
    cur = w["cur"]
    if rng.random() < 0.5:
        m = rng.randint(8, 40)
        text = (
            f"A straight path is {s * m} m long. Posts are placed every {s} m along one side, "
            f"with a post at both ends, and the same is done along the other side. Each post "
            f"costs {c} {cur}. What is the total cost of the posts, in {cur}?"
        )
        return _Problem(
            text,
            Fraction(2 * (m + 1) * c),
            {
                "off_by_one": Fraction(2 * m * c),
                "one_side_only": Fraction((m + 1) * c),
                "extra_post": Fraction(2 * (m + 2) * c),
                "short_by_one": Fraction(2 * (m - 1) * c),
                "one_side_off_by_one": Fraction(m * c),
            },
            False,
            {"variant": "path", "s": s, "m": m, "c": c},
        )
    ma, mb = rng.sample(range(3, 16), 2)
    posts = 2 * (ma + mb)
    text = (
        f"A rectangular field measures {s * ma} m by {s * mb} m. Fence posts are placed every "
        f"{s} m around its whole edge, with a post at each corner. Each post costs {c} {cur}. "
        f"What is the total cost of the posts, in {cur}?"
    )
    return _Problem(
        text,
        Fraction(posts * c),
        {
            "corners_counted_twice": Fraction((posts + 4) * c),
            "plus_one": Fraction((posts + 1) * c),
            "half_perimeter": Fraction((ma + mb) * c),
            "corners_dropped": Fraction((posts - 4) * c),
            "half_perimeter_plus_one": Fraction((ma + mb + 1) * c),
        },
        False,
        {"variant": "field", "s": s, "ma": ma, "mb": mb, "c": c},
    )


def _wp_mixture(rng: random.Random, w: dict[str, Any]) -> _Problem | None:
    tot = rng.choice((20, 25, 40, 50, 60, 80, 100))
    y = rng.randint(2, tot // 3)
    x = tot - y
    c1 = rng.choice((10, 20, 25, 30, 40, 50, 60))
    z = rng.randint(2, tot // 2)
    top = rng.randint(1, 6)
    amount = Fraction(x * c1, 100)
    correct = amount * (1 - Fraction(z, tot)) + top
    if (correct * 100).denominator != 1:
        return None
    chem = w["chem"]
    text = (
        f"A tank holds {x} litres of a solution that is {c1}% {chem}. {y} litres of pure water "
        f"are stirred in. Then {z} litres of the mixture are drained off, and finally {top} "
        f"litres of pure {chem} are added. How many litres of {chem} does the tank now contain?"
    )
    return _Problem(
        text,
        correct,
        {
            "dropped_drain": amount + top,
            "drain_before_water": amount * (1 - Fraction(z, x)) + top,
            "dropped_top_up": amount * (1 - Fraction(z, tot)),
            "top_up_before_drain": (amount + top) * (1 - Fraction(z, tot)),
            "drained_pure": amount - z + top,
            "decimal_slip": amount * 10 * (1 - Fraction(z, tot)) + top,
        },
        True,
        {"x": x, "c1": c1, "y": y, "z": z, "top": top},
    )


def _wp_savings(rng: random.Random, w: dict[str, Any]) -> _Problem | None:
    a, d, n = rng.randint(5, 40), rng.randint(2, 15), rng.randint(6, 16)
    total = n * a + d * n * (n - 1) // 2
    spend = rng.randint(total // 5, total // 2)
    person, cur, thing = rng.choice(w["people"]), w["cur"], rng.choice(w["goods"])
    text = (
        f"{person} saves {a} {cur} in week 1, and each week after that saves {d} {cur} more "
        f"than the week before. At the end of week {n}, {person} spends {spend} {cur} on a "
        f"{thing}. How many {cur} does {person} have left?"
    )
    return _Problem(
        text,
        Fraction(total - spend),
        {
            "series_off_by_one": Fraction(n * a + d * n * (n + 1) // 2 - spend),
            "dropped_spend": Fraction(total),
            "final_rate_every_week": Fraction(n * (a + (n - 1) * d) - spend),
            "one_week_short": Fraction((n - 1) * a + d * (n - 1) * (n - 2) // 2 - spend),
            "dropped_increase": Fraction(n * a - spend),
        },
        False,
        {"a": a, "d": d, "n": n, "spend": spend},
    )


def _wp_percent(rng: random.Random, w: dict[str, Any]) -> _Problem | None:
    price = rng.randint(40, 400)
    p = rng.choice((10, 20, 25, 30, 40, 50))
    q = rng.choice((10, 20, 25, 40))
    t = rng.choice((5, 10, 15, 20))
    up, down, tax = Fraction(100 + p, 100), Fraction(100 - q, 100), Fraction(100 + t, 100)
    correct = price * up * down * tax
    if (correct * 100).denominator != 1:
        return None
    item, cur = rng.choice(w["goods"]), w["cur"]
    text = (
        f"A {item} costs {price} {cur}. Its price rises by {p}%, and later the new price falls "
        f"by {q}%. A tax of {t}% is then added to that final price. How many {cur} does the "
        f"{item} cost with tax?"
    )
    return _Problem(
        text,
        correct,
        {
            "percents_added": price * Fraction(100 + p - q + t, 100),
            "dropped_tax": price * up * down,
            "fall_on_original": (price * up - price * Fraction(q, 100)) * tax,
            "tax_on_original": price * up * down + price * Fraction(t, 100),
            "rise_and_fall_swapped": price * Fraction(100 - p, 100) * Fraction(100 + q, 100) * tax,
            "fall_read_as_rise": price * up * Fraction(100 + q, 100) * tax,
        },
        True,
        {"price": price, "p": p, "q": q, "t": t},
    )


_WP_TEMPLATES: dict[str, Callable[[random.Random, dict[str, Any]], _Problem | None]] = {
    "shop": _wp_shop,
    "trip": _wp_trip,
    "production": _wp_production,
    "posts": _wp_posts,
    "mixture": _wp_mixture,
    "savings": _wp_savings,
    "percent": _wp_percent,
}


def _fmt_number(value: Fraction, money: bool) -> str | None:
    if money:
        if (value * 100).denominator != 1:
            return None
        return f"{float(value):.2f}"
    if value.denominator != 1:
        return None
    return str(value.numerator)


def numeric_shortcuts(descriptions: dict[str, str]) -> dict[str, str]:
    labels = list(descriptions)
    values = {lab: float(descriptions[lab]) for lab in labels}
    mean = sum(values.values()) / len(values)
    shapes = {
        lab: (len(descriptions[lab].split(".")[0]), "." in descriptions[lab]) for lab in labels
    }
    counts = Counter(shapes.values())
    top = max(counts.values())
    return {
        "first_option": labels[0],
        "nearest_mean": min(labels, key=lambda lab: abs(values[lab] - mean)),
        "modal_shape": next(lab for lab in labels if counts[shapes[lab]] == top),
        "largest": max(labels, key=lambda lab: values[lab]),
        "smallest": min(labels, key=lambda lab: values[lab]),
    }


def _wp_candidates(
    rng: random.Random, template: str, world: dict[str, Any], position: int
) -> list[Draft]:
    problem = _WP_TEMPLATES[template](rng, world)
    if problem is None:
        return []
    correct = _fmt_number(problem.correct, problem.money)
    if correct is None:
        return []
    slips: dict[str, str] = {}
    for name, value in problem.slips.items():
        text = _fmt_number(value, problem.money)
        if value > 0 and text is not None and text != correct and text not in slips.values():
            slips[name] = text
    if len(slips) < 3:
        return []
    out = []
    for combo in itertools.combinations(sorted(slips), 3):
        descriptions, answer = _place(rng, correct, [slips[c] for c in combo], position)
        out.append(
            Draft(
                state=problem.text,
                question=choice_question(
                    "Which option is the correct answer to the problem?", dict(descriptions)
                ),
                answer=answer,
                shortcuts=numeric_shortcuts(descriptions),
                source={"template": template, "slips": list(combo)},
                spec={
                    "template": template,
                    "params": problem.params,
                    "correct": correct,
                    "slips": {c: slips[c] for c in combo},
                },
            )
        )
    return out


def _gen_word_problem(ctx: BuildContext) -> list[Draft]:
    rng = _family_rng(ctx, "word-problem")
    worlds: dict[int, dict[str, Any]] = {}
    names = list(_WP_TEMPLATES)
    offset = rng.randrange(len(names))
    balancer = _Balancer()
    drafts = []
    for i, g in enumerate(unit_plan(ctx.per_family)):
        if g not in worlds:
            namer = _Namer(rng)
            worlds[g] = {
                "template": names[(g + offset) % len(names)],
                "people": namer.names(3),
                "goods": namer.words(4),
                "machines": namer.names(3),
                "cur": namer.word(1) if rng.random() < 0.5 else namer.word(2),
                "vehicle": namer.word(),
                "chem": namer.word(),
            }
        world = worlds[g]
        candidates: list[Draft] = []
        for _ in range(CANDIDATE_TRIES):
            candidates += _wp_candidates(rng, world["template"], world, i % 4)
            if len(candidates) >= 12:
                break
        drafts.append(balancer.pick(candidates))
    return drafts


# --------------------------------------------------------------------------
# estimate-band
# --------------------------------------------------------------------------


@dataclass
class _Estimate:
    text: str
    quantity: str
    unit: str
    value: float
    slip: float
    params: dict[str, Any]


def _es_growth(rng: random.Random, w: dict[str, Any]) -> _Estimate | None:
    p0 = rng.randint(20, 400) * 10
    r = rng.choice((4, 5, 6, 8, 10, 12, 15, 18))
    n = rng.randint(3, 7)
    grown = p0 * (1 + r / 100) ** n
    moved = rng.randint(1, max(1, int(grown * 0.3) // 10)) * 10
    text = (
        f"A colony of {w['species']} starts with {p0} individuals. Each cycle the colony grows "
        f"by {r}% of its size at the start of that cycle. After {n} cycles, {moved} individuals "
        f"are moved to another enclosure."
    )
    return _Estimate(
        text,
        "the number of individuals left in the colony",
        "individuals",
        grown - moved,
        p0 * (1 + r * n / 100) - moved,
        {"p0": p0, "r": r, "n": n, "moved": moved},
    )


def _es_tank(rng: random.Random, w: dict[str, Any]) -> _Estimate | None:
    cap = rng.randint(30, 200) * 10
    start = rng.randint(0, cap // 40) * 10
    inflow = rng.randint(20, 90)
    leak = rng.randint(3, max(3, inflow // 3))
    t1 = rng.randint(5, 30)
    extra = rng.randint(10, 60)
    after = start + (inflow - leak) * t1
    if after > cap * 0.6:
        return None
    value = t1 + (cap - after) / (inflow + extra - leak)
    slip = t1 + (cap - start - inflow * t1) / (inflow + extra)
    text = (
        f"A tank holds {cap} litres when full and starts with {start} litres. A pipe fills it at "
        f"{inflow} litres per minute while a crack leaks {leak} litres per minute. After {t1} "
        f"minutes a second pipe adding {extra} litres per minute is opened, and both pipes run "
        f"until the tank is full."
    )
    return _Estimate(
        text,
        "the total time from the start until the tank is full",
        "minutes",
        value,
        slip,
        {"cap": cap, "start": start, "inflow": inflow, "leak": leak, "t1": t1, "extra": extra},
    )


def _es_blend(rng: random.Random, w: dict[str, Any]) -> _Estimate | None:
    masses = [rng.randint(2, 30) for _ in range(3)]
    conc = [rng.randint(5, 60) for _ in range(3)]
    value = sum(m * c for m, c in zip(masses, conc, strict=True)) / sum(masses)
    sub = w["chem"]
    parts = [f"{m} kg at {c}% {sub}" for m, c in zip(masses, conc, strict=True)]
    text = f"Three batches of {w['goods']} are blended: {parts[0]}, {parts[1]}, and {parts[2]}."
    return _Estimate(
        text,
        f"the {sub} content of the blend, as a percentage by mass",
        "%",
        value,
        sum(conc) / 3,
        {"masses": masses, "conc": conc},
    )


def _es_decay(rng: random.Random, w: dict[str, Any]) -> _Estimate | None:
    h = rng.choice((2, 4, 6, 8))
    a0, a1 = rng.randint(20, 200) * 10, rng.randint(10, 150) * 10
    t1 = h * rng.randint(1, 3)
    t2 = t1 + int(h * rng.choice((1, 1.5, 2, 2.5, 3)))
    value = a0 * 0.5 ** (t2 / h) + a1 * 0.5 ** ((t2 - t1) / h)
    text = (
        f"A sample of {w['chem']} loses half of its mass every {h} hours. {a0} mg is placed in "
        f"a sealed chamber at time zero, and another {a1} mg is added {t1} hours later."
    )
    return _Estimate(
        text,
        f"the mass of {w['chem']} in the chamber {t2} hours after time zero",
        "mg",
        value,
        (a0 + a1) * 0.5 ** (t2 / h),
        {"h": h, "a0": a0, "a1": a1, "t1": t1, "t2": t2},
    )


def _es_speed(rng: random.Random, w: dict[str, Any]) -> _Estimate | None:
    dist = [rng.randint(10, 120) for _ in range(3)]
    speeds = [rng.choice((20, 30, 40, 45, 50, 60, 75, 80, 90, 100, 120)) for _ in range(3)]
    if len(set(speeds)) < 2:
        return None
    value = sum(dist) / sum(d / v for d, v in zip(dist, speeds, strict=True))
    legs = [f"{d} km at {v} km/h" for d, v in zip(dist, speeds, strict=True)]
    text = f"A {w['vehicle']} covers three legs of a route: {legs[0]}, {legs[1]}, then {legs[2]}."
    return _Estimate(
        text,
        "its average speed over the whole route",
        "km/h",
        value,
        sum(speeds) / 3,
        {"dist": dist, "speeds": speeds},
    )


def _es_savings(rng: random.Random, w: dict[str, Any]) -> _Estimate | None:
    principal = rng.randint(0, 50) * 100
    deposit = rng.randint(5, 60) * 10
    r = rng.choice((1, 1.5, 2, 2.5, 3))
    n = rng.randint(6, 24)
    g = 1 + r / 100
    value = principal * g**n + deposit * (g**n - 1) / (r / 100)
    cur = w["cur"]
    text = (
        f"An account starts with {principal} {cur}. At the end of each month it first earns "
        f"{r}% interest on its balance, and then a deposit of {deposit} {cur} is made. This "
        f"happens for {n} months."
    )
    return _Estimate(
        text,
        "the balance just after the last deposit",
        cur,
        value,
        principal + deposit * n,
        {"principal": principal, "deposit": deposit, "r": r, "n": n},
    )


def _es_work(rng: random.Random, w: dict[str, Any]) -> _Estimate | None:
    a, b, c = rng.sample(range(3, 17), 3)
    t = rng.randint(1, 4)
    rate_ab = 1 / a + 1 / b
    if t * rate_ab > 0.7:
        return None
    value = 60 * (t + (1 - t * rate_ab) / (rate_ab + 1 / c))
    p1, p2, p3 = w["people"]
    text = (
        f"Working alone, {p1} can finish a job in {a} hours, {p2} in {b} hours and {p3} in {c} "
        f"hours. {p1} and {p2} start together; after {t} hours {p3} joins them, and all three "
        f"work until the job is done."
    )
    return _Estimate(
        text,
        "the total time from the start until the job is done",
        "minutes",
        value,
        60 / (rate_ab + 1 / c),
        {"a": a, "b": b, "c": c, "t": t},
    )


_ES_TEMPLATES: dict[str, Callable[[random.Random, dict[str, Any]], _Estimate | None]] = {
    "growth": _es_growth,
    "tank": _es_tank,
    "blend": _es_blend,
    "decay": _es_decay,
    "speed": _es_speed,
    "savings": _es_savings,
    "work": _es_work,
}


def band_edges(rng: random.Random, value: float, level: int) -> tuple[list[float], int] | None:
    """Four increasing edges for five bands, with ``value`` well inside band ``level``.

    Returns the edges and the number of decimals they are written with.
    """
    for _ in range(30):
        raw = value * rng.uniform(0.1, 0.22)
        step = 10.0 ** math.floor(math.log10(raw))
        width = max(step, round(raw / step) * step)
        decimals = max(0, -math.floor(math.log10(step)))
        if level in (1, 2, 3):
            edge = round((value - rng.uniform(0.25, 0.75) * width) / step) * step
            frac = (value - edge) / width
            first = edge - (level - 1) * width
            ok = 0.2 <= frac <= 0.8
        elif level == 0:
            edge = round((value + rng.uniform(0.25, 1.0) * width) / step) * step
            frac = (edge - value) / width
            first = edge
            ok = 0.2 <= frac <= 1.0
        else:
            edge = round((value - rng.uniform(0.25, 1.0) * width) / step) * step
            frac = (value - edge) / width
            first = edge - 3 * width
            ok = 0.2 <= frac <= 1.0
        if ok and first > 0:
            return [round(first + k * width, decimals) for k in range(4)], decimals
    return None


def band_of(value: float, edges: Sequence[float]) -> int:
    return sum(value >= e for e in edges)


def _band_levels(edges: Sequence[float], decimals: int, unit: str) -> list[str]:
    def f(x: float) -> str:
        num = f"{x:.{decimals}f}"
        return f"{num}%" if unit == "%" else f"{num} {unit}"

    return [
        f"under {f(edges[0])}",
        f"{f(edges[0])} to {f(edges[1])}",
        f"{f(edges[1])} to {f(edges[2])}",
        f"{f(edges[2])} to {f(edges[3])}",
        f"{f(edges[3])} or more",
    ]


_NUMBER = re.compile(r"\d+(?:\.\d+)?")


def _es_candidate(
    rng: random.Random, template: str, world: dict[str, Any], level: int
) -> Draft | None:
    est = _ES_TEMPLATES[template](rng, world)
    if est is None or est.value <= 0:
        return None
    bands = band_edges(rng, est.value, level)
    if bands is None:
        return None
    edges, decimals = bands
    levels = _band_levels(edges, decimals, est.unit)
    numbers = [float(x) for x in _NUMBER.findall(est.text)]
    state = f"{est.text}\n\nQuantity: {est.quantity}."
    return Draft(
        state=state,
        question=score_question(
            "Which band contains the exact value of the quantity described?", levels
        ),
        answer=levels[level],
        shortcuts={
            "middle_level": levels[2],
            "naive_slip": levels[band_of(est.slip, edges)],
            "largest_number": levels[band_of(max(numbers), edges)],
            "first_number": levels[band_of(numbers[0], edges)],
        },
        source={
            "template": template,
            "band_width_share": round((edges[1] - edges[0]) / est.value, 3),
        },
        spec={
            "template": template,
            "params": est.params,
            "value": est.value,
            "edges": edges,
            "unit": est.unit,
        },
    )


def _gen_estimate(ctx: BuildContext) -> list[Draft]:
    rng = _family_rng(ctx, "estimate-band")
    worlds: dict[int, dict[str, Any]] = {}
    names = list(_ES_TEMPLATES)
    offset = rng.randrange(len(names))
    balancer = _Balancer()
    drafts = []
    for i, g in enumerate(unit_plan(ctx.per_family)):
        if g not in worlds:
            namer = _Namer(rng)
            worlds[g] = {
                "template": names[(g + offset) % len(names)],
                "species": namer.word() + "s",
                "chem": namer.word(),
                "goods": namer.word(),
                "vehicle": namer.word(),
                "cur": namer.word(),
                "people": namer.names(3),
            }
        world = worlds[g]
        drafts.append(
            balancer.pick(
                _collect(functools.partial(_es_candidate, rng, world["template"], world, i % 5), 6)
            )
        )
    return drafts


# --------------------------------------------------------------------------
# Tables (shared by table-lookup and table-count-band)
# --------------------------------------------------------------------------

_T_ID = ("code", "tag", "ref", "lot")
_T_CATS = ("depot", "guild", "district", "yard", "ward", "crew", "branch")
_T_NUMS = ("units", "weight", "price", "days", "crates", "score", "hours", "volume")


@dataclass
class Table:
    id_col: str
    cats: list[str]
    nums: list[str]
    values: dict[str, list[str]]
    rows: list[dict[str, Any]]
    cols: list[str]
    fmt: str

    def render(self) -> str:
        if self.fmt == "csv":
            lines = [",".join(self.cols)]
            lines += [",".join(str(r[c]) for c in self.cols) for r in self.rows]
        else:
            lines = ["| " + " | ".join(self.cols) + " |", "|" + "---|" * len(self.cols)]
            lines += ["| " + " | ".join(str(r[c]) for c in self.cols) + " |" for r in self.rows]
        return "\n".join(lines)


@dataclass(frozen=True)
class Cond:
    col: str
    op: str  # eq, ne, gt, le, between
    a: Any
    b: Any = None

    def holds(self, row: dict[str, Any]) -> bool:
        v = row[self.col]
        if self.op == "eq":
            return bool(v == self.a)
        if self.op == "ne":
            return bool(v != self.a)
        if self.op == "gt":
            return bool(v > self.a)
        if self.op == "le":
            return bool(v <= self.a)
        return bool(self.a <= v <= self.b)

    def text(self) -> str:
        if self.op == "eq":
            return f"{self.col} is {self.a}"
        if self.op == "ne":
            return f"{self.col} is not {self.a}"
        if self.op == "gt":
            return f"{self.col} is greater than {self.a}"
        if self.op == "le":
            return f"{self.col} is at most {self.a}"
        return f"{self.col} is between {self.a} and {self.b} inclusive"


def _make_table(rng: random.Random, n_rows: int) -> Table:
    namer = _Namer(rng)
    id_col = rng.choice(_T_ID)
    cats = rng.sample(_T_CATS, 2)
    nums = rng.sample(_T_NUMS, 3)
    values = {cats[0]: namer.names(rng.randint(4, 7)), cats[1]: namer.names(rng.randint(3, 4))}
    weights = {c: [rng.uniform(0.6, 2.4) for _ in values[c]] for c in cats}
    ranges = {c: (rng.randint(1, 5), rng.choice((30, 40, 60, 80, 99))) for c in nums}
    prefix = "".join(rng.choice("BCDFGHJKLMNPQRSTVWXZ") for _ in range(2))
    rows: list[dict[str, Any]] = []
    for x in rng.sample(range(100, 1000), n_rows):
        row: dict[str, Any] = {id_col: f"{prefix}-{x}"}
        for c in cats:
            row[c] = rng.choices(values[c], weights[c])[0]
        for c in nums:
            row[c] = rng.randint(*ranges[c])
        rows.append(row)
    others = cats + nums
    rng.shuffle(others)
    return Table(
        id_col, cats, nums, values, rows, [id_col, *others], rng.choice(("markdown", "csv"))
    )


def _rand_cond(rng: random.Random, table: Table, col: str) -> Cond:
    if col in table.cats:
        return Cond(col, "eq" if rng.random() < 0.65 else "ne", rng.choice(table.values[col]))
    vals: list[int] = sorted(int(r[col]) for r in table.rows)

    def q(f: float) -> int:
        return vals[int(f * (len(vals) - 1))]

    op = rng.choice(("gt", "le", "between"))
    if op == "gt":
        return Cond(col, "gt", q(rng.uniform(0.2, 0.7)))
    if op == "le":
        return Cond(col, "le", q(rng.uniform(0.3, 0.8)))
    lo, hi = q(rng.uniform(0.1, 0.4)), q(rng.uniform(0.6, 0.9))
    return Cond(col, "between", lo, max(hi, lo + 1))


def _table_state(table: Table, what: str) -> str:
    kind = "CSV" if table.fmt == "csv" else "Markdown table"
    return f"{what} ({len(table.rows)} rows, {kind}):\n\n{table.render()}"


# --------------------------------------------------------------------------
# table-lookup
# --------------------------------------------------------------------------

TL_AGGS = ("sum_max", "sum_min", "mean_max", "mean_min", "count_max", "range_max")
_TL_PHRASE = {
    "sum_max": "which {g} has the largest total {num}",
    "sum_min": "which {g} has the smallest total {num}",
    "mean_max": "which {g} has the highest average {num}",
    "mean_min": "which {g} has the lowest average {num}",
    "count_max": "which {g} has the most matching rows",
    "range_max": "which {g} has the widest spread of {num} (largest value minus smallest value)",
}


def table_group_scores(
    rows: Sequence[dict[str, Any]], gcol: str, groups: Sequence[str], agg: str, num: str
) -> dict[str, float] | None:
    """Per-group aggregate, or None when some group lacks the rows the question needs."""
    by: dict[str, list[int]] = {g: [] for g in groups}
    for r in rows:
        by[r[gcol]].append(r[num])
    kind = agg.split("_", maxsplit=1)[0]
    need = {"sum": 1 if agg == "sum_min" else 0, "mean": 2, "range": 2, "count": 0}[kind]
    if any(len(v) < need for v in by.values()) or sum(bool(v) for v in by.values()) < 2:
        return None
    if kind == "sum":
        return {g: float(sum(v)) for g, v in by.items()}
    if kind == "mean":
        return {g: sum(v) / len(v) for g, v in by.items()}
    if kind == "range":
        return {g: float(max(v) - min(v)) for g, v in by.items()}
    return {g: float(len(v)) for g, v in by.items()}


def _tl_winner(scores: dict[str, float], agg: str) -> str | None:
    want_max = agg.endswith("max")
    ordered = sorted(scores.items(), key=lambda kv: kv[1], reverse=want_max)
    best, second = ordered[0][1], ordered[1][1]
    kind = agg.split("_", maxsplit=1)[0]
    need = {"sum": max(3.0, 0.06 * abs(best)), "mean": 1.0, "count": 1.0, "range": 2.0}[kind]
    return ordered[0][0] if abs(best - second) >= need else None


def _tl_candidate(rng: random.Random, table: Table, counters: Counter[int]) -> Draft | None:
    gcol = rng.choice(table.cats)
    agg = rng.choice(TL_AGGS)
    num = rng.choice(table.nums)
    others = [c for c in table.cats + table.nums if c not in (gcol, num)]
    conds = [_rand_cond(rng, table, c) for c in rng.sample(others, rng.choice((1, 2, 2)))]
    rows = [r for r in table.rows if all(c.holds(r) for c in conds)]
    if not 8 <= len(rows) <= 0.8 * len(table.rows):
        return None
    groups = table.values[gcol]
    scores = table_group_scores(rows, gcol, groups, agg, num)
    if scores is None:
        return None
    answer = _tl_winner(scores, agg)
    if answer is None:
        return None
    k = len(groups)
    position = counters[k] % k
    rest = [g for g in groups if g != answer]
    rng.shuffle(rest)
    options = [*rest[:position], answer, *rest[position:]]

    want_max = agg.endswith("max")
    full = table_group_scores(table.rows, gcol, groups, agg, num) or {g: 0.0 for g in groups}
    ignore = (
        max(options, key=lambda g: full[g]) if want_max else min(options, key=lambda g: full[g])
    )
    ext_col = num
    if agg == "count_max":
        num_conds = [c.col for c in conds if c.col in table.nums]
        ext_col = num_conds[0] if num_conds else num
    ext_values = [r[ext_col] for r in table.rows]
    ext_target = max(ext_values) if want_max else min(ext_values)
    extreme = next(r[gcol] for r in table.rows if r[ext_col] == ext_target)
    sizes = Counter(r[gcol] for r in table.rows)
    most = (
        max(options, key=lambda g: sizes[g]) if want_max else min(options, key=lambda g: sizes[g])
    )
    where = " and ".join(c.text() for c in conds)
    instruction = f"Considering only rows where {where}, {_TL_PHRASE[agg].format(g=gcol, num=num)}?"
    return Draft(
        state=_table_state(table, "Records"),
        question=choice_question(_cap(instruction), {g: None for g in options}),
        answer=answer,
        shortcuts={
            "first_option": options[0],
            "first_row": table.rows[0][gcol],
            "extreme_row": extreme,
            "most_rows": most,
            "ignore_filter": ignore,
        },
        source={
            "template": agg,
            "rows": len(table.rows),
            "matching_rows": len(rows),
            "conditions": len(conds),
            "format": table.fmt,
        },
        spec={"gcol": gcol, "agg": agg, "num": num, "conds": conds, "table": table},
    )


def _gen_table_lookup(ctx: BuildContext) -> list[Draft]:
    rng = _family_rng(ctx, "table-lookup")
    tables: dict[int, Table] = {}
    counters: Counter[int] = Counter()
    balancer = _Balancer()
    drafts = []
    for g in unit_plan(ctx.per_family):
        table = tables.setdefault(g, _make_table(rng, rng.randint(30, 200)))
        chosen = balancer.pick(_collect(functools.partial(_tl_candidate, rng, table, counters), 8))
        counters[len(chosen.question["options"])] += 1
        drafts.append(chosen)
    return drafts


# --------------------------------------------------------------------------
# table-count-band
# --------------------------------------------------------------------------

TC_SHAPES = ("and2", "and3", "or2", "or_and")


def count_matches(shape: str, conds: Sequence[Cond], row: dict[str, Any]) -> bool:
    h = [c.holds(row) for c in conds]
    if shape in ("and2", "and3"):
        return all(h)
    if shape == "or2":
        return h[0] or h[1]
    return (h[0] or h[1]) and h[2]


def count_edges(rng: random.Random, count: int, level: int) -> list[int] | None:
    """Four edges for five count bands with ``count`` at least one away from its band's ends."""
    for _ in range(12):
        width = rng.randint(max(3, round(0.12 * count)), max(3, round(0.3 * count)))
        if level in (1, 2, 3):
            first = count - rng.randint(1, width - 2) - (level - 1) * width
        elif level == 0:
            first = count + rng.randint(2, width)
        else:
            first = count - rng.randint(1, width - 1) - 3 * width
        if first >= 2:
            return [first + k * width for k in range(4)]
    return None


def count_levels(edges: Sequence[int]) -> list[str]:
    return [
        f"fewer than {edges[0]}",
        f"{edges[0]} to {edges[1] - 1}",
        f"{edges[1]} to {edges[2] - 1}",
        f"{edges[2]} to {edges[3] - 1}",
        f"{edges[3]} or more",
    ]


def _tc_candidate(rng: random.Random, table: Table, level: int) -> Draft | None:
    shape = rng.choice(TC_SHAPES)
    n_conds = 2 if shape in ("and2", "or2") else 3
    conds = [_rand_cond(rng, table, c) for c in rng.sample(table.cats + table.nums, n_conds)]
    count = sum(count_matches(shape, conds, r) for r in table.rows)
    if count < 3 or count > 0.7 * len(table.rows):
        return None
    edges = count_edges(rng, count, level)
    if edges is None:
        return None
    levels = count_levels(edges)
    first_only = sum(conds[0].holds(r) for r in table.rows)
    t = [c.text() for c in conds]
    if shape == "and2":
        where = f"{t[0]} and {t[1]}"
    elif shape == "and3":
        where = f"{t[0]}, {t[1]} and {t[2]}"
    elif shape == "or2":
        where = f"{t[0]} or {t[1]} (or both)"
    else:
        where = f"{t[2]}, and also either {t[0]} or {t[1]}"
    instruction = f"How many rows are there where {where}?"
    return Draft(
        state=_table_state(table, "Records"),
        question=score_question(instruction, levels),
        answer=levels[level],
        shortcuts={
            "middle_level": levels[2],
            "first_condition_only": levels[band_of(first_only, edges)],
            "fifth_of_rows": levels[band_of(len(table.rows) / 5, edges)],
        },
        source={
            "template": shape,
            "rows": len(table.rows),
            "band_width": edges[1] - edges[0],
            "format": table.fmt,
        },
        spec={"shape": shape, "conds": conds, "count": count, "edges": edges, "table": table},
    )


def _gen_table_count(ctx: BuildContext) -> list[Draft]:
    rng = _family_rng(ctx, "table-count-band")
    tables: dict[int, Table] = {}
    balancer = _Balancer()
    drafts = []
    for i, g in enumerate(unit_plan(ctx.per_family)):
        table = tables.setdefault(g, _make_table(rng, rng.randint(40, 200)))
        drafts.append(
            balancer.pick(_collect(functools.partial(_tc_candidate, rng, table, i % 5), 8))
        )
    return drafts


# --------------------------------------------------------------------------
# Policies (shared by policy-decision and policy-clause)
# --------------------------------------------------------------------------

_POLICY_FRAMES: tuple[dict[str, Any], ...] = (
    {
        "id": "loans",
        "title": "Instrument loan policy of the {org} depot",
        "c1": ("Requester role", "the requester is {v}", "the requester is {v} or {w}"),
        "c2": ("Destination", "the destination is {v}", "the destination is not {v}"),
        "n1": (
            "Instruments requested", 1, 40,
            "more than {x} instruments are requested", "at most {x} instruments are requested",
        ),
        "n2": (
            "Loan length in days", 1, 60,
            "the loan is for more than {x} days", "the loan is for {x} days or fewer",
        ),
        "flag": ("Sponsor on file", "a sponsor is on file", "no sponsor is on file"),
    },
    {
        "id": "archive",
        "title": "Reading room access policy of the {org} archive",
        "c1": ("Applicant rank", "the applicant is {v}", "the applicant is {v} or {w}"),
        "c2": ("Vault section", "the section is {v}", "the section is not {v}"),
        "n1": (
            "Items to view", 1, 30,
            "more than {x} items are requested", "at most {x} items are requested",
        ),
        "n2": (
            "Prior visits", 0, 20,
            "the applicant has made more than {x} prior visits",
            "the applicant has made {x} or fewer prior visits",
        ),
        "flag": ("Escort booked", "an escort is booked", "no escort is booked"),
    },
    {
        "id": "claims",
        "title": "Expense claim policy of the {org} guild",
        "c1": ("Claimant grade", "the claimant is {v}", "the claimant is {v} or {w}"),
        "c2": ("Expense type", "the expense type is {v}", "the expense type is not {v}"),
        "n1": (
            "Amount in marks", 5, 900,
            "the amount is more than {x} marks", "the amount is at most {x} marks",
        ),
        "n2": (
            "Days since purchase", 1, 120,
            "the claim is filed more than {x} days after purchase",
            "the claim is filed within {x} days of purchase",
        ),
        "flag": ("Receipt attached", "a receipt is attached", "no receipt is attached"),
    },
    {
        "id": "berths",
        "title": "Berth policy of {org} harbour",
        "c1": ("Vessel class", "the vessel is {v}", "the vessel is {v} or {w}"),
        "c2": ("Requested quay", "the quay requested is {v}", "the quay requested is not {v}"),
        "n1": (
            "Length in metres", 8, 120,
            "the vessel is longer than {x} metres", "the vessel is {x} metres long or shorter",
        ),
        "n2": (
            "Nights requested", 1, 30,
            "more than {x} nights are requested", "at most {x} nights are requested",
        ),
        "flag": ("Hazard cargo declared", "hazard cargo is declared", "no hazard cargo is declared"),
    },
)  # fmt: skip
_POLICY_KEYS = ("c1", "c2", "n1", "n2", "flag")
_STOP = frozenset(
    [
        "the",
        "this",
        "that",
        "with",
        "from",
        "than",
        "more",
        "have",
        "been",
        "will",
        "does",
        "clause",
        "request",
        "requests",
        "any",
        "applies",
        "apply",
        "unless",
        "other",
        "what",
        "which",
        "when",
        "where",
        "there",
        "their",
        "into",
        "also",
        "only",
    ]
)


@dataclass(frozen=True)
class Atom:
    kind: str  # c1_in, c2_is, c2_not, n_gt, n_le, flag
    key: str
    value: Any

    def holds(self, case: dict[str, Any]) -> bool:
        v = case[self.key]
        if self.kind == "c1_in":
            return bool(v in self.value)
        if self.kind == "c2_is":
            return bool(v == self.value)
        if self.kind == "c2_not":
            return bool(v != self.value)
        if self.kind == "n_gt":
            return bool(v > self.value)
        if self.kind == "n_le":
            return bool(v <= self.value)
        return bool(v == self.value)


@dataclass(frozen=True)
class Clause:
    number: int
    conds: tuple[Atom, ...]
    unless: Atom | None
    approve: bool
    default: bool
    beats: tuple[int, ...]
    style: int

    def applies(self, case: dict[str, Any]) -> bool:
        if self.default:
            return True
        if not all(a.holds(case) for a in self.conds):
            return False
        return not (self.unless is not None and self.unless.holds(case))


@dataclass
class Policy:
    frame: dict[str, Any]
    org: str
    regime: str  # "first": lowest applicable number decides; "last": highest
    clauses: list[Clause]
    order: list[int]  # precedence, highest first; the default clause is last
    roles: list[str]
    zones: list[str]
    line_order: list[str]

    @property
    def default(self) -> Clause:
        return next(c for c in self.clauses if c.default)

    def decide(self, case: dict[str, Any]) -> tuple[int, bool, list[int]]:
        applicable = [c.number for c in self.clauses if c.applies(case)]
        winner = next(n for n in self.order if n in applicable)
        return winner, self.clauses[winner - 1].approve, applicable

    def base_winner(self, applicable: Sequence[int]) -> int:
        """Who would decide by the numbering rule alone, ignoring precedence statements."""
        live = [n for n in applicable if n != self.default.number] or [self.default.number]
        return min(live) if self.regime == "first" else max(live)


def _atom_text(atom: Atom, fr: dict[str, Any]) -> str:
    if atom.kind == "c1_in":
        vals = [f"{_article(v)} {v}" for v in atom.value]
        template = fr["c1"][1] if len(vals) == 1 else fr["c1"][2]
        return str(template.format(v=vals[0], w=vals[-1]))
    if atom.kind == "c2_is":
        return str(fr["c2"][1].format(v=atom.value))
    if atom.kind == "c2_not":
        return str(fr["c2"][2].format(v=atom.value))
    if atom.kind == "n_gt":
        return str(fr[atom.key][3].format(x=atom.value))
    if atom.kind == "n_le":
        return str(fr[atom.key][4].format(x=atom.value))
    return str(fr["flag"][1] if atom.value else fr["flag"][2])


def clause_text(clause: Clause, fr: dict[str, Any]) -> str:
    word = "approved" if clause.approve else "refused"
    if clause.default:
        body = f"If no other clause applies, the request is {word}."
    else:
        conds = " and ".join(_atom_text(a, fr) for a in clause.conds)
        unless = f", unless {_atom_text(clause.unless, fr)}" if clause.unless else ""
        if clause.style == 0:
            body = f"A request is {word} if {conds}{unless}."
        elif clause.style == 1:
            body = f"If {conds}, the request is {word}{unless}."
        else:
            verb = "Approve" if clause.approve else "Refuse"
            body = f"{verb} any request where {conds}{unless}."
    if clause.beats:
        noun = "clause" if len(clause.beats) == 1 else "clauses"
        body += f" This clause takes precedence over {noun} {_join_and([str(b) for b in clause.beats])}."
    return f"Clause {clause.number}. {body}"


def _rand_atom(
    rng: random.Random, key: str, fr: dict[str, Any], roles: list[str], zones: list[str]
) -> Atom:
    if key == "c1":
        return Atom("c1_in", key, tuple(rng.sample(roles, rng.choice((1, 1, 2)))))
    if key == "c2":
        return Atom(rng.choice(("c2_is", "c2_is", "c2_not")), key, rng.choice(zones))
    if key == "flag":
        return Atom("flag", key, rng.random() < 0.5)
    lo, hi = fr[key][1], fr[key][2]
    x = rng.randint(lo + (hi - lo) // 4, hi - (hi - lo) // 4)
    return Atom(rng.choice(("n_gt", "n_le")), key, x)


def _random_case(rng: random.Random, pol: Policy) -> dict[str, Any]:
    fr = pol.frame
    return {
        "c1": rng.choice(pol.roles),
        "c2": rng.choice(pol.zones),
        "n1": rng.randint(fr["n1"][1], fr["n1"][2]),
        "n2": rng.randint(fr["n2"][1], fr["n2"][2]),
        "flag": rng.random() < 0.5,
    }


def _make_policy(rng: random.Random) -> Policy:
    namer = _Namer(rng)
    fr = rng.choice(_POLICY_FRAMES)
    n = rng.randint(6, 12)
    regime = rng.choice(("first", "last"))
    roles, zones = namer.words(rng.randint(3, 5)), namer.names(rng.randint(3, 4))
    default_number = n if regime == "first" else 1
    numbers = [x for x in range(1, n + 1) if x != default_number]
    order = list(numbers) if regime == "first" else numbers[::-1]
    promoted: list[int] = []
    for _ in range(rng.randint(2, 3 if n < 9 else 4)):
        movable = [x for x in order[2:] if x not in promoted]
        if not movable:
            break
        x = rng.choice(movable)
        i = order.index(x)
        order.remove(x)
        order.insert(rng.randrange(0, i - 1), x)
        promoted.append(x)
    final = [*order, default_number]

    def base_prefers(a: int, b: int) -> bool:
        return a < b if regime == "first" else a > b

    verdicts = [j % 2 == 0 for j in range(len(numbers))]
    rng.shuffle(verdicts)
    clauses: list[Clause] = []
    for number in range(1, n + 1):
        if number == default_number:
            clauses.append(Clause(number, (), None, rng.random() < 0.5, True, (), 0))
            continue
        k = rng.choices((1, 2, 3), (0.4, 0.45, 0.15))[0]
        has_unless = rng.random() < 0.3
        keys = rng.sample(_POLICY_KEYS, k + int(has_unless))
        conds = tuple(_rand_atom(rng, key, fr, roles, zones) for key in keys[:k])
        unless = _rand_atom(rng, keys[k], fr, roles, zones) if has_unless else None
        beats = tuple(
            sorted(
                a
                for a in numbers
                if base_prefers(a, number) and final.index(number) < final.index(a)
            )
        )
        clauses.append(
            Clause(number, conds, unless, verdicts.pop(), False, beats, rng.randrange(3))
        )
    lines = list(_POLICY_KEYS)
    rng.shuffle(lines)
    return Policy(fr, namer.word().capitalize(), regime, clauses, final, roles, zones, lines)


def _policy_ok(rng: random.Random, pol: Policy) -> bool:
    """Both outcomes reachable, conflicts common, most clauses able to decide."""
    approvals = 0
    conflicts = 0
    deciders: set[int] = set()
    for _ in range(300):
        case = _random_case(rng, pol)
        winner, approve, applicable = pol.decide(case)
        approvals += approve
        live = [a for a in applicable if a != pol.default.number]
        conflicts += len({pol.clauses[a - 1].approve for a in live}) == 2
        deciders.add(winner)
    return 75 <= approvals <= 225 and conflicts >= 45 and len(deciders) >= 0.6 * len(pol.clauses)


def _case_text(pol: Policy, case: dict[str, Any]) -> str:
    fr = pol.frame
    rendered = {
        "c1": case["c1"],
        "c2": case["c2"],
        "n1": str(case["n1"]),
        "n2": str(case["n2"]),
        "flag": "yes" if case["flag"] else "no",
    }
    return "\n".join(f"- {fr[key][0]}: {rendered[key]}" for key in pol.line_order)


def _policy_state(pol: Policy, case: dict[str, Any]) -> str:
    fr = pol.frame
    which = "lowest" if pol.regime == "first" else "highest"
    lines = [
        fr["title"].format(org=pol.org),
        "",
        f"When more than one clause applies to a request, the applicable clause with the {which} "
        "number decides it, except that a clause which says it takes precedence over another "
        "clause wins whenever both apply. A clause whose unless condition is met does not apply.",
        "",
        *[clause_text(c, fr) for c in pol.clauses],
        "",
        "Request under review:",
        _case_text(pol, case),
    ]
    return "\n".join(lines)


def _tokens(text: str) -> set[str]:
    return {w for w in re.findall(r"[a-z]+", text.lower()) if len(w) > 3 and w not in _STOP}


def _policy_meta(pol: Policy, case: dict[str, Any]) -> dict[str, Any]:
    winner, _, applicable = pol.decide(case)
    live = [a for a in applicable if a != pol.default.number]
    return {
        "template": pol.frame["id"],
        "regime": pol.regime,
        "clauses": len(pol.clauses),
        "applicable": len(live),
        "conflict": len({pol.clauses[a - 1].approve for a in live}) == 2,
        # A precedence statement changes the outcome of the numbering rule.
        "override_used": winner != pol.base_winner(applicable),
        # A clause with a precedence statement applies but still loses.
        "precedence_trap": any(pol.clauses[a - 1].beats for a in live if a != winner),
    }


def _precedence_penalty(d: Draft) -> float:
    """Prefer cases where precedence matters and at least two clauses conflict."""
    relevant = d.source["override_used"] or d.source["precedence_trap"]
    return (0.0 if relevant else 0.6) + (0.0 if d.source["applicable"] >= 2 else 0.6)


def _policy_spec(pol: Policy, case: dict[str, Any]) -> dict[str, Any]:
    return {"policy": pol, "case": case}


def _policy_world(rng: random.Random) -> Policy:
    for _ in range(200):
        pol = _make_policy(rng)
        if _policy_ok(rng, pol):
            return pol
    raise RuntimeError("could not generate a usable policy")


def _pd_candidate(rng: random.Random, pol: Policy, label: bool) -> Draft | None:
    case = _random_case(rng, pol)
    _, approve, _ = pol.decide(case)
    if approve != label:
        return None
    fr = pol.frame
    case_tokens = {case["c1"].lower(), case["c2"].lower()}
    votes = Counter(
        c.approve
        for c in pol.clauses
        if not c.default and case_tokens & set(re.findall(r"[a-z]+", clause_text(c, fr).lower()))
    )
    tied = votes[True] == votes[False]
    mention = pol.default.approve if tied else votes[True] > votes[False]
    return Draft(
        state=_policy_state(pol, case),
        question=noul_question("Under this policy, is the request under review approved?"),
        answer=label,
        shortcuts={
            "default_verdict": "true" if pol.default.approve else "false",
            "mention_majority": "true" if mention else "false",
        },
        source=_policy_meta(pol, case),
        spec={**_policy_spec(pol, case), "case_text": _case_text(pol, case)},
    )


def _gen_policy_decision(ctx: BuildContext) -> list[Draft]:
    rng = _family_rng(ctx, "policy-decision")
    policies: dict[int, Policy] = {}
    balancer = _Balancer()
    drafts = []

    for i, g in enumerate(unit_plan(ctx.per_family)):
        pol = policies.setdefault(g, _policy_world(rng))
        make = functools.partial(_pd_candidate, rng, pol, i % 2 == 0)
        drafts.append(balancer.pick(_collect(make, 10, 3000), _precedence_penalty))
    _add_length_shortcut(drafts, lambda d: str(d.spec["case_text"]))
    return drafts


def _pc_candidate(rng: random.Random, pol: Policy, target: int) -> Draft | None:
    case = _random_case(rng, pol)
    winner, _, _ = pol.decide(case)
    if winner != target:
        return None
    fr = pol.frame
    texts = {c.number: clause_text(c, fr) for c in pol.clauses}
    case_tokens = _tokens(_case_text(pol, case))
    live = [c.number for c in pol.clauses if not c.default]
    overlap = max(live, key=lambda n: (len(_tokens(texts[n]) & case_tokens), -n))
    longest = max(live, key=lambda n: (len(texts[n]), -n))
    with_beats = [c.number for c in pol.clauses if c.beats]
    options = [str(c.number) for c in pol.clauses]
    return Draft(
        state=_policy_state(pol, case),
        question=choice_question(
            "Which clause decides the request under review?",
            {o: f"Clause {o}" for o in options},
        ),
        answer=str(target),
        shortcuts={
            "first_option": options[0],
            "last_option": options[-1],
            "default_clause": str(pol.default.number),
            "precedence_clause": str(min(with_beats)) if with_beats else options[0],
            "most_overlap": str(overlap),
            "longest_clause": str(longest),
        },
        source=_policy_meta(pol, case),
        spec=_policy_spec(pol, case),
    )


def _gen_policy_clause(ctx: BuildContext) -> list[Draft]:
    rng = _family_rng(ctx, "policy-clause")
    policies: dict[int, Policy] = {}
    balancer = _Balancer()
    drafts = []

    counts: Counter[str] = Counter()
    floor = [0]

    def penalty(d: Draft) -> float:
        # Spread answers over clause numbers so the majority answer stays near chance.
        base = 0.0 if d.answer == str(d.spec["policy"].default.number) else _precedence_penalty(d)
        return base + 0.4 * (counts[str(d.answer)] - floor[0])

    for g in unit_plan(ctx.per_family):
        pol = policies.setdefault(g, _policy_world(rng))
        numbers = [c.number for c in pol.clauses]
        rng.shuffle(numbers)
        numbers.sort(key=lambda n: counts[str(n)])
        candidates: list[Draft] = []
        for target in numbers:
            make = functools.partial(_pc_candidate, rng, pol, target)
            try:
                candidates += _collect(make, 4, 400)
            except RuntimeError:
                continue
            if len({d.answer for d in candidates}) >= 3:
                break
        floor[0] = min(counts[str(d.answer)] for d in candidates)
        chosen = balancer.pick(candidates, penalty)
        counts[str(chosen.answer)] += 1
        drafts.append(chosen)
    return drafts


# --------------------------------------------------------------------------
# Registry entry points
# --------------------------------------------------------------------------

_GENERATORS: dict[str, Callable[[BuildContext], list[Draft]]] = {
    "constraint-pick": _gen_constraint,
    "entailment": _gen_entailment,
    "word-problem": _gen_word_problem,
    "estimate-band": _gen_estimate,
    "table-lookup": _gen_table_lookup,
    "table-count-band": _gen_table_count,
    "policy-decision": _gen_policy_decision,
    "policy-clause": _gen_policy_clause,
}


def generate(family: str, ctx: BuildContext) -> list[tuple[Item, Draft]]:
    """Items for one family, each with the draft (and structured spec) it came from."""
    drafts = _GENERATORS[family](ctx)
    out = []
    for i, (draft, g) in enumerate(zip(drafts, unit_plan(ctx.per_family), strict=True)):
        item = make_item(
            family=family,
            key=f"{ctx.seed}:{family}:{i}",
            source_unit=f"{family}-g{g:02d}",
            state=draft.state,
            question=draft.question,
            answer=draft.answer,
            reference=REFERENCE,
            shortcuts=draft.shortcuts,
            source={**draft.source, "seed_group": g},
        )
        out.append((item, draft))
    return out


def _builder(family: str) -> Callable[[BuildContext], list[Item]]:
    def build(ctx: BuildContext) -> list[Item]:
        return [item for item, _ in generate(family, ctx)]

    build.__name__ = f"build_{family.replace('-', '_')}"
    return build


BUILDERS: dict[str, Callable[[BuildContext], list[Item]]] = {
    family: _builder(family) for family in _GENERATORS
}
