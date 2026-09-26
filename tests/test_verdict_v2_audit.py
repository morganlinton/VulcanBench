"""Verdict v2 ground-truth audit: the parsers and solvers, on synthetic inputs.

The audit (scripts/verdict-v2/audit_ground_truth.py) re-derives answers from
rendered item text. These tests feed it small hand-written states whose
answers are worked out by hand, plus a wrong answer that it must flag.
"""

import importlib.util
import random
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
_spec = importlib.util.spec_from_file_location(
    "audit_ground_truth", ROOT / "scripts/verdict-v2/audit_ground_truth.py"
)
assert _spec and _spec.loader
audit = importlib.util.module_from_spec(_spec)
sys.modules["audit_ground_truth"] = audit
_spec.loader.exec_module(audit)


def _item(family, state, question, answer):
    return {
        "item_id": f"{family}-test",
        "family": family,
        "state": state,
        "question": question,
        "answer": answer,
    }


def _choice(instructions, descriptions):
    return {
        "type": "choice",
        "instructions": instructions,
        "options": list(descriptions),
        "descriptions": descriptions,
    }


def _noul(instructions="Does it hold?"):
    return {"type": "noul", "instructions": instructions}


def _score(instructions, levels):
    return {"type": "score", "instructions": instructions, "levels": levels}


def _status(item):
    return audit.verify(item, None).status


# --------------------------------------------------------------------------
# Sampling
# --------------------------------------------------------------------------


def test_quota_is_five_percent_with_a_floor_of_ten():
    assert audit.per_family_quota(300) == 15
    assert audit.per_family_quota(244) == 13
    assert audit.per_family_quota(100) == 10
    assert audit.per_family_quota(6) == 6


def test_sample_is_fixed_and_independent_of_file_order():
    items = [{"item_id": f"fam-{i:03d}", "family": "fam"} for i in range(300)]
    items += [{"item_id": f"other-{i:03d}", "family": "other"} for i in range(40)]
    a = audit.sample_items(items)
    shuffled = list(items)
    random.Random(1).shuffle(shuffled)
    b = audit.sample_items(shuffled)
    assert [i["item_id"] for i in a["fam"]] == [i["item_id"] for i in b["fam"]]
    assert len(a["fam"]) == 15
    assert len(a["other"]) == 10
    assert audit.sample_items(items, seed=7)["fam"] != a["fam"]


# --------------------------------------------------------------------------
# constraint-pick
# --------------------------------------------------------------------------

CP_STATE = (
    "Four tenants, Abel, Bram, Cato and Dima, each live on a different floor of a tower. "
    "The floors are numbered 1 (lowest) to 4 (highest).\n"
    "Each tenant also flies exactly one banner, of kind kefe or movi.\n\n"
    "Rules:\n"
    "1. Abel lives on a lower floor than Bram.\n"
    "2. There is exactly 1 other floor between Cato's floor and Dima's.\n"
    "3. Bram flies the movi banner.\n"
    "4. If Abel lives on an even-numbered floor, then Cato lives on a lower floor than Dima.\n"
    "5. At most 1 of the tenants fly the movi banner.\n\n"
    "Each option lists the floors from 1 to 4, with each tenant's banner in brackets."
)
CP_OPTIONS = {
    "A": "Floor 1: Abel (kefe); Floor 2: Cato (kefe); Floor 3: Bram (movi); Floor 4: Dima (kefe)",
    "B": "Floor 1: Bram (movi); Floor 2: Abel (kefe); Floor 3: Cato (kefe); Floor 4: Dima (kefe)",
    "C": "Floor 1: Abel (kefe); Floor 2: Cato (kefe); Floor 3: Bram (movi); Floor 4: Dima (movi)",
}


def test_constraint_rules_parse_into_kinds():
    world, rules = audit.cp_parse(CP_STATE)
    assert world.names == ["Abel", "Bram", "Cato", "Dima"]
    assert (world.verb3, world.verb, world.noun, world.kinds) == (
        "flies",
        "fly",
        "banner",
        ["kefe", "movi"],
    )
    assert [kind for kind, _ in rules] == ["before", "gap", "badge_is", "cond_even", "count_max"]


def test_constraint_pick_brute_force_finds_the_one_valid_option():
    q = _choice("Which option satisfies every rule?", CP_OPTIONS)
    assert _status(_item("constraint-pick", CP_STATE, q, "A")) == "verified"
    assert _status(_item("constraint-pick", CP_STATE, q, "B")) == "mismatch"


def test_constraint_pick_rejects_an_unreadable_rule():
    state = CP_STATE.replace("3. Bram flies the movi banner.", "3. Bram hums quietly.")
    q = _choice("Which option satisfies every rule?", CP_OPTIONS)
    assert _status(_item("constraint-pick", state, q, "A")) == "unverifiable"


# --------------------------------------------------------------------------
# entailment
# --------------------------------------------------------------------------

PROP_PREAMBLE = "Each sentence about the objects below is either true or false.\n\n"
MON_PREAMBLE = "The statements below are about the things in one closed collection.\n\n"


def _ent(preamble, premises, conclusion):
    body = "\n".join(f"{i}. {p}" for i, p in enumerate(premises, 1))
    return f"{preamble}Premises:\n{body}\n\nConclusion: {conclusion}"


@pytest.mark.parametrize(
    ("sentence", "style"),
    [
        ("If the brop glinds, then the tosk does not var.", "if"),
        ("The brop glinds only if the tosk vars.", "only_if"),
        ("The brop glinds unless the tosk vars.", "unless"),
        ("Either the brop glinds or the tosk vars, or both.", "or"),
        ("Either the brop glinds or the tosk vars, but not both.", "xor"),
        ("It is not the case that both the brop glinds and the tosk vars.", "nand"),
        ("The brop glinds if and only if the tosk vars.", "iff"),
        ("If the brop glinds and the tosk vars, then the plim does not zork.", "if_and"),
        ("If the brop glinds, then the tosk vars or the plim zorks (or both).", "if_or"),
        ("The brop does not glind.", "fact"),
    ],
)
def test_propositional_forms_parse(sentence, style):
    assert audit.prop_parse(sentence)[0] == style


def test_propositional_entailment_by_truth_table():
    mp = _ent(
        PROP_PREAMBLE,
        ["If the brop glinds, then the tosk vars.", "The brop glinds."],
        "The tosk vars.",
    )
    assert audit.entailment_follows(mp)[0] is True
    affirm = _ent(
        PROP_PREAMBLE,
        ["If the brop glinds, then the tosk vars.", "The tosk vars."],
        "The brop glinds.",
    )
    assert audit.entailment_follows(affirm)[0] is False
    unless = _ent(
        PROP_PREAMBLE,
        ["The brop glinds unless the tosk vars.", "The tosk does not var."],
        "The brop glinds.",
    )
    assert audit.entailment_follows(unless)[0] is True


def test_monadic_entailment_by_model_enumeration():
    chain = ["Every kizelt is a naikrond.", "Every naikrond is a traibryn."]
    assert (
        audit.entailment_follows(_ent(MON_PREAMBLE, chain, "Every kizelt is a traibryn."))[0]
        is True
    )
    # "Every X is a Y" does not say any X exists.
    assert (
        audit.entailment_follows(_ent(MON_PREAMBLE, chain, "Some kizelt is a traibryn."))[0]
        is False
    )
    witness = [*chain, "Some kizelt is an oblo."]
    assert (
        audit.entailment_follows(_ent(MON_PREAMBLE, witness, "Some oblo is a traibryn."))[0] is True
    )
    no = ["No kizelt is a naikrond.", "Nothing that is not a naikrond is a traibryn."]
    assert audit.entailment_follows(_ent(MON_PREAMBLE, no, "No kizelt is a traibryn."))[0] is True


# --------------------------------------------------------------------------
# word-problem and estimate-band
# --------------------------------------------------------------------------


def test_word_problem_templates_recompute():
    shop = (
        "Ana buys 2 zibs at 10 kro each and 3 zabs at 20 kro each. The shop takes 25% off the whole "
        "order, and then adds a delivery charge of 5 kro. How many kro does Ana pay in total?"
    )
    assert audit.word_problem_value(shop) == "65.00"
    trip = (
        "A plymer travels 40 km at 48 km/h, stops for 23 minutes, and then travels 63 km at 54 km/h. "
        "How many minutes does the whole trip take?"
    )
    assert audit.word_problem_value(trip) == "143"
    posts = (
        "A rectangular field measures 44 m by 20 m. Fence posts are placed every 4 m around its whole "
        "edge, with a post at each corner. Each post costs 12 kro. What is the total cost of the posts, in kro?"
    )
    assert audit.word_problem_value(posts) == "384"
    q = _choice(
        "Which option is the correct answer?", {"A": "25", "B": "143", "C": "120", "D": "73"}
    )
    assert _status(_item("word-problem", trip, q, "B")) == "verified"
    assert _status(_item("word-problem", trip, q, "A")) == "mismatch"


def test_estimate_band_value_and_bands():
    state = (
        "A klotralt covers three legs of a route: 60 km at 60 km/h, 60 km at 30 km/h, then 60 km at "
        "60 km/h.\n\nQuantity: its average speed over the whole route."
    )
    assert audit.estimate_value(state) == pytest.approx(45.0)
    levels = [
        "under 20 km/h",
        "20 km/h to 40 km/h",
        "40 km/h to 60 km/h",
        "60 km/h to 80 km/h",
        "80 km/h or more",
    ]
    assert audit.band_bounds(levels[0]) == (None, 20.0)
    assert audit.band_bounds(levels[2]) == (40.0, 60.0)
    assert audit.band_bounds(levels[4]) == (80.0, None)
    q = _score("Which band contains the exact value?", levels)
    assert _status(_item("estimate-band", state, q, levels[2])) == "verified"
    assert _status(_item("estimate-band", state, q, levels[1])) == "mismatch"


# --------------------------------------------------------------------------
# Tables
# --------------------------------------------------------------------------

CSV_STATE = (
    "Records (6 rows, CSV):\n\n"
    "lot,yard,units,guild\n"
    "AB-1,Kaido,10,Vree\n"
    "AB-2,Kaido,30,Timar\n"
    "AB-3,Peplee,50,Vree\n"
    "AB-4,Kaido,5,Vree\n"
    "AB-5,Peplee,7,Timar\n"
    "AB-6,Kaido,40,Timar"
)
MD_STATE = (
    "Records (3 rows, Markdown table):\n\n"
    "| lot | yard | units |\n|---|---|---|\n| X-1 | Kaido | 3 |\n| X-2 | Peplee | 9 |\n| X-3 | Kaido | 12 |"
)


def test_tables_parse_in_both_formats():
    rows = audit.parse_table(CSV_STATE)
    assert len(rows) == 6
    assert rows[1] == {"lot": "AB-2", "yard": "Kaido", "units": 30, "guild": "Timar"}
    md = audit.parse_table(MD_STATE)
    assert [r["units"] for r in md] == [3, 9, 12]


def test_table_lookup_recomputes_the_group():
    q = _choice(
        "Considering only rows where yard is Kaido and units is between 5 and 35 inclusive, "
        "which guild has the largest total units?",
        {"Vree": None, "Timar": None},
    )
    # Kaido and 5..35: Vree 10 + 5 = 15, Timar 30.
    assert _status(_item("table-lookup", CSV_STATE, q, "Timar")) == "verified"
    assert _status(_item("table-lookup", CSV_STATE, q, "Vree")) == "mismatch"


def test_table_count_band_structures():
    assert (
        audit.count_structure(
            "How many rows are there where yard is Kaido and units is at most 9?"
        )[0]
        == "and2"
    )
    shape, parts = audit.count_structure(
        "How many rows are there where units is greater than 6, and also either yard is Peplee or guild is not Vree?"
    )
    assert shape == "or_and"
    assert parts == ["units is greater than 6", "yard is Peplee", "guild is not Vree"]
    levels = ["fewer than 2", "2 to 2", "3 to 3", "4 to 4", "5 or more"]
    q = _score(
        "How many rows are there where units is greater than 6, and also either yard is Peplee or guild is not Vree?",
        levels,
    )
    # units > 6: AB-1, AB-2, AB-3, AB-5, AB-6; then Peplee or not Vree: AB-2, AB-3, AB-5, AB-6.
    assert _status(_item("table-count-band", CSV_STATE, q, "4 to 4")) == "verified"
    assert _status(_item("table-count-band", CSV_STATE, q, "3 to 3")) == "mismatch"


# --------------------------------------------------------------------------
# Policies
# --------------------------------------------------------------------------

POLICY_STATE = (
    "Instrument loan policy of the Tobrouk depot\n\n"
    "When more than one clause applies to a request, the applicable clause with the lowest number "
    "decides it, except that a clause which says it takes precedence over another clause wins "
    "whenever both apply. A clause whose unless condition is met does not apply.\n\n"
    "Clause 1. If more than 14 instruments are requested, the request is approved, unless the destination is Slezylt.\n"
    "Clause 2. A request is refused if the loan is for more than 42 days. This clause takes precedence over clause 1.\n"
    "Clause 3. Approve any request where the requester is a nouslaim or a laislom and no sponsor is on file.\n"
    "Clause 4. If no other clause applies, the request is refused.\n\n"
    "Request under review:\n"
    "- Instruments requested: {n1}\n"
    "- Sponsor on file: {flag}\n"
    "- Destination: {c2}\n"
    "- Loan length in days: {n2}\n"
    "- Requester role: {c1}"
)


def _policy(**case):
    values = {"n1": 20, "flag": "no", "c2": "Krasir", "n2": 10, "c1": "besend", **case}
    return POLICY_STATE.format(**values)


def test_policy_precedence_and_numbering():
    assert audit.policy_winner(_policy()).number == 1  # only clause 1 applies
    assert audit.policy_winner(_policy(n2=50)).number == 2  # 2 beats 1 by its statement
    assert audit.policy_winner(_policy(c2="Slezylt")).number == 4  # unless met: default
    assert audit.policy_winner(_policy(n1=3, c1="laislom")).number == 3
    assert audit.policy_winner(_policy(c1="laislom")).number == 1  # 1 and 3 apply: lowest wins


def test_policy_families_check_the_stored_answer():
    q = _choice(
        "Which clause decides the request under review?",
        {str(i): f"Clause {i}" for i in range(1, 5)},
    )
    assert _status(_item("policy-clause", _policy(n2=50), q, "2")) == "verified"
    assert _status(_item("policy-clause", _policy(n2=50), q, "1")) == "mismatch"
    assert _status(_item("policy-decision", _policy(n2=50), _noul(), False)) == "verified"
    assert _status(_item("policy-decision", _policy(), _noul(), False)) == "mismatch"


# --------------------------------------------------------------------------
# Code families: extraction, diffs, expected values
# --------------------------------------------------------------------------


def test_code_output_program_and_type_check_snippets_extract():
    state = "Language: Python 3 (CPython)\nThe program below runs once.\n\n```python\nprint(1)\nprint(2)\n```\n"
    assert audit.code_output_program(state) == ("python", "print(1)\nprint(2)\n")
    tc = (
        "Exactly one of them passes `mypy --strict` (mypy 2.1.0, --python-version 3.12) with no errors.\n\n"
        "Snippet A:\n```python\nx: int = 1\n```\n\nSnippet B:\n```python\nx: int = 'a'\n```\n"
    )
    flags, named, snippets = audit.type_check_parse(tc)
    assert flags == ["--strict", "--python-version", "3.12"]
    assert named == "mypy 2.1.0"
    assert snippets == {"A": "x: int = 1\n", "B": "x: int = 'a'\n"}


def test_unified_diff_applies_and_checks_context():
    original = "def f(x):\n    if x > 1:\n        return 1\n    return 0\n"
    diff = (
        "--- a/m.py\n+++ b/m.py\n@@ -1,4 +1,4 @@\n def f(x):\n-    if x > 1:\n+    if x >= 1:\n"
        "         return 1\n     return 0\n"
    )
    assert audit.apply_unified_diff(original, diff) == original.replace("x > 1", "x >= 1")
    with pytest.raises(audit.AuditError):
        audit.apply_unified_diff(original.replace("return 0", "return 9"), diff)


def test_changed_functions_names_the_edited_method():
    original = "class K:\n    def a(self):\n        return 1\n\n    def b(self):\n        return 2\n\n\ndef c():\n    return 3\n"
    shown = original.replace("return 2", "return 20")
    mutant, names = audit.changed_functions(original, shown)
    assert names == ["K.b"]
    assert mutant == shown
    # A dash normalised in the shown text is not an edit.
    dashed = original.replace("return 3", "return 3  # a " + chr(0x2014) + " b")
    _, names = audit.changed_functions(
        dashed, dashed.replace(chr(0x2014), "--").replace("return 1", "return 0")
    )
    assert names == ["K.a"]


def test_expected_value_assertion_parses_and_independent_impls_agree():
    state = (
        "A function's specification and a test assertion for it:\n\n```python\n"
        'def page_summary(total_items: int, per_page: int) -> tuple[int, int]:\n    """..."""\n    ...\n\n\n'
        "assert page_summary(23, 5) == (5, 3)\n```\n"
    )
    assert audit.expected_value_parse(state) == ("page_summary", [23, 5], (5, 3), "(5, 3)")
    impl = audit.EV_IMPLS
    assert impl["snake_case"]("HTTPServerError") == "http_server_error"
    assert impl["snake_case"]("loadCSVFile2") == "load_csv_file2"
    assert impl["binary_gap"](0b1001000) == 2
    assert impl["digital_root"](89960) == 5
    assert impl["round_half_up"](2.675, 2) == 2.68
    assert impl["count_leap_years"](1896, 1904) == 2  # 1896 and 1904; 1900 is not
    assert impl["merge_intervals"]([[5, 6], [1, 3], [3, 4]]) == [[1, 4], [5, 6]]


# --------------------------------------------------------------------------
# Archive and mined helpers
# --------------------------------------------------------------------------


def test_patch_sections_and_normalisation():
    state = (
        "## Issue\n\ntext\n\n## Patch A\n\n```diff\n--- a/x\n+++ b/x\n+a  \n```\n\n"
        "## Patch B\n\n```diff\nindex 1..2\n--- a/x\n+++ b/x\n+b\n```\n"
    )
    assert audit.section_patch(state, "Patch A", "Patch B") == "--- a/x\n+++ b/x\n+a  "
    assert (
        audit.normalized_patch(audit.section_patch(state, "Patch B", None))
        == "--- a/x\n+++ b/x\n+b"
    )


def test_source_file_classifier():
    assert audit.is_non_test_source("lib/queryHelpers.js")
    assert audit.is_non_test_source("src/pkg/core.py")
    assert not audit.is_non_test_source("packages/dev/vite/node-adapter-test.ts")
    assert not audit.is_non_test_source("src/app.spec.ts")
    assert not audit.is_non_test_source("tests/test_core.py")
    assert not audit.is_non_test_source("docs/api/dev.md")
    assert not audit.is_non_test_source("types/index.d.ts")


def test_vulnerable_side_matches_the_pre_fix_file():
    before = "func f(x int) int {\n    return x\n}\n"
    after = "func f(x int) int {\n    if x < 0 {\n        return 0\n    }\n    return x\n}\n"
    assert audit.vulnerable_side(before, after, before, after) == "A"
    assert audit.vulnerable_side(after, before, before, after) == "B"


def test_patch_reproduces_ignores_crlf_only_differences():
    before = "a\nb\nc\n"
    after = "a\nB\nc\n"
    patch = "@@ -1,3 +1,3 @@\n a\r\n-b\r\n+B\r\n c\r"
    assert audit.patch_reproduces(before, after, patch)
    assert not audit.patch_reproduces(before, "a\nX\nc\n", patch)


# --------------------------------------------------------------------------
# incident-root-cause
# --------------------------------------------------------------------------


def test_log_lines_parse_in_all_four_formats():
    year = 2026
    plain = audit.parse_log_line(
        "2026-07-23 22:14:06.391 INFO  [dvodriron] POST /v1/botreik 200 149ms", year
    )
    assert plain[1:] == ("dvodriron", "INFO", "POST /v1/botreik 200 149ms")
    sys_ = audit.parse_log_line("Jul 23 22:12:01.676 ip-10-5-11-100 gvainai[1444]: WARN slow", year)
    assert sys_[1:] == ("gvainai", "WARN", "slow")
    kv = audit.parse_log_line(
        'ts=2026-07-23T22:12:07.756Z level=error svc=zhidok msg="boom" host=ip-1', year
    )
    assert kv[1:] == ("zhidok", "ERROR", "boom")
    js = audit.parse_log_line(
        '{"ts":"2026-07-23T22:12:07.756Z","level":"warn","service":"qix","msg":"hi"}', year
    )
    assert js[1:] == ("qix", "WARN", "hi")
    assert plain[0] < js[0] or plain[0] > js[0]


def test_disclosed_clock_offsets_are_undone():
    rows = [
        (audit._iso("2026-07-23T22:00:00.000"), "aaa", "INFO", "hello"),
        (audit._iso("2026-07-23T22:00:10.000"), "bbb", "WARN",
         "ntp: local clock is 100.0s behind reference time; step refused, offset persists"),
        (audit._iso("2026-07-23T22:00:20.000"), "bbb", "ERROR", "later"),
    ]  # fmt: skip
    assert audit.corrected_times(rows) == [0.0, 110.0, 120.0]


def _incident_state(lines):
    head = (
        "Incident review: merged logs from 4 services, 2026-07-23 22:00:00 to 22:10:00 UTC.\n"
        "Lines are merged by the timestamp each host recorded.\n"
        "Services (address): apia (10.0.0.1), dbx (10.0.0.2), gwy (10.0.0.3), qul (10.0.0.4).\n\nLogs:\n"
    )
    return head + "\n".join(lines) + "\n"


def test_incident_tracer_follows_blame_to_the_root():
    def line(sec, svc, level, msg):
        m, s = divmod(sec, 60)
        return f"2026-07-23 22:{m:02d}:{s:02d}.000 {level:<5} [{svc}] {msg}"

    lines = [
        line(
            1, "qul", "WARN", "call to dbx took 900ms (budget 800ms)"
        ),  # background: seen in the quiet start
        line(5, "apia", "INFO", "GET /v1/items 200 12ms"),
        line(30, "gwy", "WARN", "client qul exceeded soft rate limit, allowing burst"),
        line(120, "dbx", "ERROR", "could not complete statement on orders: internal error"),
        line(
            125,
            "apia",
            "WARN",
            "retrying GET http://dbx:8080/v1/x (attempt 1/3): 503 Service Unavailable",
        ),
        line(130, "apia", "ERROR", "call to dbx failed after 3 attempts: 503 Service Unavailable"),
        line(140, "gwy", "ERROR", "GET /v1/items 502 40ms upstream=apia"),
        line(150, "gwy", "ERROR", "GET /v1/items 502 41ms upstream=apia"),
        line(160, "qul", "WARN", "call to dbx took 950ms (budget 800ms)"),
        line(170, "dbx", "ERROR", "could not complete statement on orders: internal error"),
        line(200, "gwy", "WARN", "client qul exceeded soft rate limit, allowing burst"),
    ]
    guess, _ = audit.incident_trace(_incident_state(lines))
    assert guess == "dbx"
