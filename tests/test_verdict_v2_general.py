"""General-pillar Verdict v2 families: construction, balance, shortcuts, determinism.

Answers are re-derived here with solvers written independently of the
generator (truth tables, small-model enumeration, table parsing from the
rendered state, a pairwise precedence engine), so a generator bug that
produces a wrong answer shows up as a disagreement.
"""

import itertools
import re
from collections import Counter
from fractions import Fraction
from pathlib import Path

import pytest

from harness.verdict.v2.families import general
from harness.verdict.v2.items import MAX_STATE_CHARS, answer_label, labels_for, validate
from harness.verdict.v2.registry import BY_ID, BuildContext, builder_for

REPO = Path(__file__).resolve().parents[1]
FAMILIES = tuple(general.BUILDERS)
FULL = 250
SMALL = 60


def _ctx(seed, per_family):
    return BuildContext(seed=seed, repo=REPO, per_family=per_family)


@pytest.fixture(scope="module")
def full():
    return {f: general.generate(f, _ctx(11, FULL)) for f in FAMILIES}


@pytest.fixture(scope="module")
def small():
    return {f: general.generate(f, _ctx(3, SMALL)) for f in FAMILIES}


# --------------------------------------------------------------------------
# Registry, validity, determinism
# --------------------------------------------------------------------------


def test_builders_cover_the_eight_general_families():
    expected = {f for f, fam in BY_ID.items() if fam.pillar == "general"}
    assert set(general.BUILDERS) == expected
    for family in expected:
        assert builder_for(family) is general.BUILDERS[family]


def test_items_validate_and_fit(full):
    ids = set()
    for family, built in full.items():
        assert len(built) == FULL
        for item, _ in built:
            validate(item)
            assert item.family == family
            assert item.reference == "generator"
            assert len(item.state) < MAX_STATE_CHARS
            instructions = item.question["instructions"]
            assert "\n" not in instructions
            assert instructions.endswith("?")
            ids.add(item.item_id)
    assert len(ids) == FULL * len(FAMILIES)


def test_source_units_and_splits(full):
    for family, built in full.items():
        units = Counter(item.source_unit for item, _ in built)
        assert len(units) >= 40, family
        assert all(unit.startswith(f"{family}-g") for unit in units)
        assert {item.split for item, _ in built} == {"dev", "test"}
        by_unit = {}
        for item, _ in built:
            assert by_unit.setdefault(item.source_unit, item.split) == item.split


def test_no_dashes_in_any_text(full):
    banned = re.compile("[" + chr(0x2013) + chr(0x2014) + "]")  # en and em dash
    for built in full.values():
        for item, _ in built:
            q = item.question
            texts = [item.state, q["instructions"], *q.get("levels", [])]
            texts += [d for d in (q.get("descriptions") or {}).values() if d]
            assert not any(banned.search(t) for t in texts)


def test_builds_are_deterministic():
    for family in FAMILIES:
        a = [i.to_json() for i in general.BUILDERS[family](_ctx(5, 12))]
        b = [i.to_json() for i in general.BUILDERS[family](_ctx(5, 12))]
        c = [i.to_json() for i in general.BUILDERS[family](_ctx(6, 12))]
        assert a == b, family
        assert a != c, family


def test_small_builds_still_work():
    for family in FAMILIES:
        items = general.BUILDERS[family](_ctx(9, 7))
        assert len(items) == 7
        assert len({i.source_unit for i in items}) == 7


# --------------------------------------------------------------------------
# Balance and shortcuts
# --------------------------------------------------------------------------


def _truths(built):
    return [answer_label(item.question, item.answer) for item, _ in built]


def test_noul_families_are_exactly_half_true(full):
    for family in ("entailment", "policy-decision"):
        assert Counter(_truths(full[family])) == {"true": FULL // 2, "false": FULL // 2}


def test_score_families_hit_every_level_equally(full):
    for family in ("estimate-band", "table-count-band"):
        levels = Counter(item.question["levels"].index(item.answer) for item, _ in full[family])
        assert levels == {k: FULL // 5 for k in range(5)}, family


def test_letter_choice_answers_are_spread_over_positions(full):
    for family in ("constraint-pick", "word-problem"):
        counts = Counter(item.answer for item, _ in full[family])
        assert set(counts) == set("ABCD")
        assert max(counts.values()) - min(counts.values()) <= 2, family


def test_table_lookup_positions_are_balanced_per_option_count(full):
    by_k = {}
    for item, _ in full["table-lookup"]:
        options = item.question["options"]
        by_k.setdefault(len(options), Counter())[options.index(item.answer)] += 1
    for k, counts in by_k.items():
        assert len(counts) == k or sum(counts.values()) < k
        assert max(counts.values()) - min(counts.get(p, 0) for p in range(k)) <= 1


def test_policy_clause_answers_are_spread(full):
    counts = Counter(item.answer for item, _ in full["policy-clause"])
    assert max(counts.values()) / FULL <= 0.15
    assert len(counts) >= 10


def test_every_shortcut_and_the_majority_stay_near_chance(full):
    for family, built in full.items():
        chance = sum(1 / len(labels_for(item.question)) for item, _ in built) / FULL
        truths = _truths(built)
        majority = Counter(truths).most_common(1)[0][1] / FULL
        assert majority <= chance + 0.10, family
        names = {name for item, _ in built for name in item.shortcuts}
        assert names, family
        for name in names:
            hits = sum(
                item.shortcuts[name] == truth
                for (item, _), truth in zip(built, truths, strict=True)
            )
            assert hits / FULL <= chance + 0.10, (family, name, hits / FULL, chance)


def test_noul_families_record_the_length_shortcut(full):
    for family in ("entailment", "policy-decision"):
        assert all("long_statement" in item.shortcuts for item, _ in full[family])


# --------------------------------------------------------------------------
# constraint-pick: independent evaluator
# --------------------------------------------------------------------------

ENTITY_ARGS = {
    "before": (0, 1),
    "imm": (0, 1),
    "adj": (0, 1),
    "nadj": (0, 1),
    "gap": (0, 1),
    "xor_end": (0, 1),
    "badge_diff": (0, 1),
    "not_at": (0,),
    "at_end": (0,),
    "badge_is": (0,),
    "cond_even": (0, 1, 2),
    "cond_badge": (0, 2, 3),
    "holder_not": (),
    "count_max": (),
    "badge_order": (),
}


def _cp_ref(rule, arr, n):
    kind, a = rule
    slot, badge = arr
    who = {slot[e]: e for e in range(n)}
    place = {e: slot[e] for e in range(n)}
    table = {
        "before": lambda: place[a[0]] < place[a[1]],
        "imm": lambda: place[a[0]] + 1 == place[a[1]],
        "adj": lambda: place[a[1]] - place[a[0]] in (-1, 1),
        "nadj": lambda: place[a[1]] - place[a[0]] not in (-1, 1),
        "gap": lambda: abs(place[a[1]] - place[a[0]]) - 1 == a[2],
        "not_at": lambda: who[a[1]] != a[0],
        "at_end": lambda: place[a[0]] == 1 or place[a[0]] == n,
        "cond_even": lambda: place[a[0]] % 2 != 0 or place[a[1]] < place[a[2]],
        "xor_end": lambda: (who[1] == a[0]) ^ (who[n] == a[1]),
        "badge_is": lambda: badge[a[0]] == a[1],
        "badge_diff": lambda: badge[a[0]] != badge[a[1]],
        "badge_order": lambda: (
            max((place[e] for e in range(n) if badge[e] == a[0]), default=0)
            < min((place[e] for e in range(n) if badge[e] == a[1]), default=n + 1)
        ),
        "holder_not": lambda: badge[who[a[0]]] != a[1],
        "count_max": lambda: list(badge).count(a[0]) <= a[1],
        "cond_badge": lambda: badge[a[0]] != a[1] or place[a[2]] < place[a[3]],
    }
    return table[kind]()


def test_constraint_pick_has_exactly_one_valid_option(small):
    for item, draft in small["constraint-pick"]:
        spec = draft.spec
        n, rules, options = spec["n"], spec["rules"], spec["options"]
        assert 4 <= n <= 6
        assert 5 <= len(rules) <= 8
        valid = [lab for lab, arr in options.items() if all(_cp_ref(r, arr, n) for r in rules)]
        assert valid == [item.answer]
        for label, arr in options.items():
            if label != item.answer:
                broken = [r for r in rules if not _cp_ref(r, arr, n)]
                assert len(broken) == 1
                assert broken[0][0] not in general.CP_UNARY


def test_constraint_pick_text_matches_the_structure(small):
    for item, draft in small["constraint-pick"]:
        spec = draft.spec
        rule_lines = re.findall(r"^\d+\. (.*)$", item.state, re.MULTILINE)
        assert len(rule_lines) == len(spec["rules"])
        for line, (kind, args) in zip(rule_lines, spec["rules"], strict=True):
            for idx in ENTITY_ARGS[kind]:
                assert spec["names"][args[idx]] in line
        for label, text in item.question["descriptions"].items():
            parsed = re.findall(r"(\d+): (\w+)(?: \((\w+)\))?", text)
            slot, badge = spec["options"][label]
            assert len(parsed) == spec["n"]
            for s, name, b in parsed:
                e = spec["names"].index(name)
                assert slot[e] == int(s)
                if spec["nb"]:
                    assert spec["bvals"][badge[e]] == b


# --------------------------------------------------------------------------
# entailment: truth tables and small-model enumeration
# --------------------------------------------------------------------------

PROP_MEANING = {
    "if": lambda v: (not v[0]) or v[1],
    "only_if": lambda v: (not v[0]) or v[1],
    "unless": lambda v: v[1] or v[0],
    "or": lambda v: v[0] or v[1],
    "xor": lambda v: v[0] ^ v[1],
    "nand": lambda v: not (v[0] and v[1]),
    "iff": lambda v: v[0] is v[1],
    "if_and": lambda v: (not (v[0] and v[1])) or v[2],
    "if_or": lambda v: (not v[0]) or v[1] or v[2],
    "fact": lambda v: v[0],
}


def _prop_value(stmt, valuation):
    style, lits = stmt
    return PROP_MEANING[style]([valuation[a] if pos else not valuation[a] for a, pos in lits])


def _mon_holds(stmt, model):
    def meets(lits, t):
        return all(((t >> p) & 1 == 1) == pos for p, pos in lits)

    if stmt.kind == "all":
        return all(meets([stmt.concl], t) for t in model if meets(stmt.conds, t))
    return any(meets(stmt.conds, t) for t in model)


def test_entailment_labels_match_exhaustive_model_checking(small):
    for item, draft in small["entailment"]:
        spec = draft.spec
        k, premises, concl = spec["k"], spec["premises"], spec["conclusion"]
        assert 3 <= k <= 5
        assert 3 <= len(premises) <= 6
        if spec["style"] == "prop":
            models = [
                v
                for v in itertools.product((False, True), repeat=k)
                if all(_prop_value(p, v) for p in premises)
            ]
            assert models
            follows = all(_prop_value(concl, v) for v in models)
        else:
            # A countermodel needs at most one witness per existential plus one element.
            assert sum(p.kind == "some" for p in premises) <= 2
            models = [
                m
                for size in (1, 2, 3)
                for m in itertools.combinations(range(2**k), size)
                if all(_mon_holds(p, m) for p in premises)
            ]
            assert models
            follows = all(_mon_holds(concl, m) for m in models)
        assert follows is item.answer
        assert f"Conclusion: {spec['conclusion_text']}" in item.state


def test_true_conclusions_need_more_than_one_premise(small):
    for item, draft in small["entailment"]:
        if not item.answer:
            continue
        spec = draft.spec
        k = spec["k"]
        for p in spec["premises"]:
            if spec["style"] == "prop":
                alone = [v for v in itertools.product((False, True), repeat=k) if _prop_value(p, v)]
                assert not all(_prop_value(spec["conclusion"], v) for v in alone)
            else:
                assert not general.mon_entails([p], spec["conclusion"], k)


def test_entailment_pairs_share_a_conclusion_form(full):
    built = full["entailment"]
    for j in range(0, FULL, 2):
        (a, da), (b, db) = built[j], built[j + 1]
        assert a.answer is True
        assert b.answer is False
        assert da.source["template"] == db.source["template"]


# --------------------------------------------------------------------------
# word-problem: recompute from parameters
# --------------------------------------------------------------------------


def _wp_expected(template, p):  # noqa: PLR0911, one branch per template
    if template == "shop":
        total = Fraction(p["q1"] * p["p1"] + p["q2"] * p["p2"]) * (100 - p["d"]) / 100 + p["fee"]
        return f"{float(total):.2f}"
    if template == "trip":
        minutes = Fraction(p["d1"] * 60, p["v1"]) + p["stop"] + Fraction(p["d2"] * 60, p["v2"])
        return str(int(minutes))
    if template == "production":
        made = p["a"] * p["h1"] + (p["a"] + p["b"]) * p["h2"]
        return str(int(Fraction(made) * (100 - p["p"]) / 100))
    if template == "posts":
        if p["variant"] == "path":
            posts_per_side = p["s"] * p["m"] // p["s"] + 1
            return str(2 * posts_per_side * p["c"])
        perimeter = 2 * (p["s"] * p["ma"] + p["s"] * p["mb"])
        return str(perimeter // p["s"] * p["c"])
    if template == "mixture":
        volume, chem = Fraction(p["x"]), Fraction(p["x"] * p["c1"], 100)
        volume += p["y"]
        chem -= chem * p["z"] / volume
        volume -= p["z"]
        chem += p["top"]
        return f"{float(chem):.2f}"
    if template == "savings":
        saved = sum(p["a"] + p["d"] * (week - 1) for week in range(1, p["n"] + 1))
        return str(saved - p["spend"])
    price = Fraction(p["price"]) * (100 + p["p"]) / 100 * (100 - p["q"]) / 100
    return f"{float(price * (100 + p['t']) / 100):.2f}"


def test_word_problem_answers_recompute(small):
    for item, draft in small["word-problem"]:
        descriptions = item.question["descriptions"]
        assert len(set(descriptions.values())) == 4
        expected = _wp_expected(draft.spec["template"], draft.spec["params"])
        assert descriptions[item.answer] == expected
        assert expected == draft.spec["correct"]
        assert expected not in draft.spec["slips"].values()


# --------------------------------------------------------------------------
# estimate-band: recompute and check the band from the rendered level text
# --------------------------------------------------------------------------


def _es_value(template, p):  # noqa: PLR0911, one branch per template
    if template == "growth":
        return p["p0"] * (1 + p["r"] / 100) ** p["n"] - p["moved"]
    if template == "tank":
        level = p["start"] + (p["inflow"] - p["leak"]) * p["t1"]
        return p["t1"] + (p["cap"] - level) / (p["inflow"] + p["extra"] - p["leak"])
    if template == "blend":
        return sum(m * c for m, c in zip(p["masses"], p["conc"], strict=True)) / sum(p["masses"])
    if template == "decay":
        return p["a0"] / 2 ** (p["t2"] / p["h"]) + p["a1"] / 2 ** ((p["t2"] - p["t1"]) / p["h"])
    if template == "speed":
        hours = sum(d / v for d, v in zip(p["dist"], p["speeds"], strict=True))
        return sum(p["dist"]) / hours
    if template == "savings":
        balance = p["principal"]
        for _ in range(p["n"]):
            balance = balance * (1 + p["r"] / 100) + p["deposit"]
        return balance
    rate_ab = 1 / p["a"] + 1 / p["b"]
    left = 1 - p["t"] * rate_ab
    return 60 * (p["t"] + left / (rate_ab + 1 / p["c"]))


def _level_bounds(level_text):
    nums = [float(x) for x in re.findall(r"\d+(?:\.\d+)?", level_text)]
    if level_text.startswith("under") or level_text.startswith("fewer"):
        return None, nums[0]
    if level_text.endswith("or more"):
        return nums[0], None
    return nums[0], nums[1]


def test_estimate_band_value_sits_inside_its_band(small):
    for item, draft in small["estimate-band"]:
        value = _es_value(draft.spec["template"], draft.spec["params"])
        assert value == pytest.approx(draft.spec["value"], rel=1e-9)
        levels = item.question["levels"]
        lo, hi = _level_bounds(item.answer)
        edges = [float(x) for x in re.findall(r"\d+(?:\.\d+)?", levels[1])]
        width = edges[1] - edges[0]
        assert lo is None or value >= lo + 0.15 * width
        assert hi is None or value <= hi - 0.15 * width
        for other in levels:
            if other != item.answer:
                olo, ohi = _level_bounds(other)
                assert (olo is not None and value < olo) or (ohi is not None and value >= ohi)


# --------------------------------------------------------------------------
# Tables: parse the rendered table and recompute
# --------------------------------------------------------------------------


def _parse_table(state):
    body = state.split("\n\n", 1)[1].strip().splitlines()
    if body[0].startswith("|"):
        rows = [[c.strip() for c in line.strip("|").split("|")] for line in body]
        rows = [r for r in rows if not set("".join(r)) <= {"-"}]
    else:
        rows = [line.split(",") for line in body]
    header, data = rows[0], rows[1:]
    return [
        {h: int(v) if v.isdigit() else v for h, v in zip(header, row, strict=True)} for row in data
    ]


def _cond_ok(cond, row):
    v = row[cond.col]
    return {
        "eq": lambda: v == cond.a,
        "ne": lambda: v != cond.a,
        "gt": lambda: v > cond.a,
        "le": lambda: not v > cond.a,
        "between": lambda: cond.a <= v <= cond.b,
    }[cond.op]()


def test_table_lookup_answers_recompute_from_the_rendered_table(small):
    for item, draft in small["table-lookup"]:
        spec = draft.spec
        rows = _parse_table(item.state)
        assert 30 <= len(rows) <= 200
        instruction = item.question["instructions"]
        for cond in spec["conds"]:
            assert cond.col in instruction
            assert str(cond.a) in instruction
        groups = {}
        for row in rows:
            if all(_cond_ok(c, row) for c in spec["conds"]):
                groups.setdefault(row[spec["gcol"]], []).append(row[spec["num"]])
        options = item.question["options"]
        kind, direction = spec["agg"].split("_")
        score = {
            "sum": sum,
            "mean": lambda v: sum(v) / len(v),
            "count": len,
            "range": lambda v: max(v) - min(v),
        }[kind]
        values = {g: score(groups[g]) if groups.get(g) else 0 for g in options}
        best = sorted(values.values(), reverse=direction == "max")
        assert values[item.answer] == best[0]
        assert best[0] != best[1]


def test_table_count_band_counts_recompute(small):
    for item, draft in small["table-count-band"]:
        spec = draft.spec
        rows = _parse_table(item.state)
        assert 30 <= len(rows) <= 200
        c = spec["conds"]
        shape = spec["shape"]

        def match(row, c=c, shape=shape):
            h = [_cond_ok(x, row) for x in c]
            if shape.startswith("and"):
                return all(h)
            if shape == "or2":
                return h[0] or h[1]
            return h[2] and (h[0] or h[1])

        count = sum(match(r) for r in rows)
        assert count == spec["count"]
        lo, hi = _level_bounds(item.answer)
        assert lo is None or count >= lo + 1
        # "fewer than X" excludes X; "A to B" includes B; keep one row of margin.
        if item.answer.startswith("fewer"):
            assert count <= hi - 2
        elif hi is not None:
            assert count <= hi - 1


# --------------------------------------------------------------------------
# Policies: pairwise precedence engine and rendering
# --------------------------------------------------------------------------


def _atom_ok(atom, case):
    v = case[atom.key]
    return {
        "c1_in": lambda: v in atom.value,
        "c2_is": lambda: v == atom.value,
        "c2_not": lambda: v != atom.value,
        "n_gt": lambda: v > atom.value,
        "n_le": lambda: v <= atom.value,
        "flag": lambda: v is atom.value,
    }[atom.kind]()


def _ref_winner(policy, case):
    live = [
        c
        for c in policy.clauses
        if c.default
        or (
            all(_atom_ok(a, case) for a in c.conds)
            and not (c.unless is not None and _atom_ok(c.unless, case))
        )
    ]

    def beats(x, y):
        if x.default or y.default:
            return y.default and not x.default
        if y.number in x.beats:
            return True
        if x.number in y.beats:
            return False
        return x.number < y.number if policy.regime == "first" else x.number > y.number

    winners = [x for x in live if all(beats(x, y) for y in live if y is not x)]
    assert len(winners) == 1
    return winners[0]


def _check_policy_text(item, policy, case):
    lines = dict(re.findall(r"^Clause (\d+)\. (.*)$", item.state, re.MULTILINE))
    assert sorted(map(int, lines)) == list(range(1, len(policy.clauses) + 1))
    assert 6 <= len(policy.clauses) <= 12
    for clause in policy.clauses:
        found = re.search(
            r"takes precedence over clauses? ([\d, and]+)\.", lines[str(clause.number)]
        )
        stated = tuple(sorted(int(x) for x in re.findall(r"\d+", found.group(1)))) if found else ()
        assert stated == clause.beats
    shown = dict(re.findall(r"^- ([^:]+): (.*)$", item.state, re.MULTILINE))
    frame = policy.frame
    assert shown[frame["c1"][0]] == case["c1"]
    assert shown[frame["c2"][0]] == case["c2"]
    assert shown[frame["n1"][0]] == str(case["n1"])
    assert shown[frame["n2"][0]] == str(case["n2"])
    assert shown[frame["flag"][0]] == ("yes" if case["flag"] else "no")


def test_policy_decision_matches_the_reference_engine(small):
    for item, draft in small["policy-decision"]:
        policy, case = draft.spec["policy"], draft.spec["case"]
        assert _ref_winner(policy, case).approve is item.answer
        _check_policy_text(item, policy, case)


def test_policy_clause_matches_the_reference_engine(small):
    for item, draft in small["policy-clause"]:
        policy, case = draft.spec["policy"], draft.spec["case"]
        assert str(_ref_winner(policy, case).number) == item.answer
        assert item.question["options"] == [str(n) for n in range(1, len(policy.clauses) + 1)]
        _check_policy_text(item, policy, case)


def test_policy_cases_usually_need_precedence(full):
    for family in ("policy-decision", "policy-clause"):
        sources = [item.source for item, _ in full[family]]
        relevant = sum(s["override_used"] or s["precedence_trap"] for s in sources)
        several = sum(s["applicable"] >= 2 for s in sources)
        assert relevant / FULL >= 0.4, family
        assert several / FULL >= 0.6, family
