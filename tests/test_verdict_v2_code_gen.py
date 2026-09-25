"""Fast tests for the generated Software families of Verdict v2."""

import ast
import json
import random
import re
import subprocess
import sys
import textwrap
from pathlib import Path

import pytest

from harness.verdict.v2.families import code_gen
from harness.verdict.v2.families.code_gen_exec import Job, Runner
from harness.verdict.v2.families.code_gen_libs import (
    LibUnit,
    clean_failure,
    killed,
    load_unit,
    module_functions,
    parse_junit,
    tests_in_file,
    workspace,
)
from harness.verdict.v2.families.code_gen_mutate import js_mutants, mutants
from harness.verdict.v2.families.code_gen_programs import TEMPLATES, Program
from harness.verdict.v2.families.code_gen_select import (
    ChoiceBalancer,
    ChoiceConfig,
    balance_noul,
    longest,
    most_common_shape,
    nearest_center,
)
from harness.verdict.v2.families.code_gen_specs import SPECS, is_round
from harness.verdict.v2.families.code_gen_types import TEMPLATES as TYPE_TEMPLATES
from harness.verdict.v2.families.code_gen_types import mypy_check, mypy_version, render
from harness.verdict.v2.items import answer_label, labels_for, validate
from harness.verdict.v2.registry import BuildContext, builder_for

SAMPLE = textwrap.dedent(
    """\
    def f(xs, limit=3):
        total = 0
        for i, x in enumerate(xs):
            if x % 2 == 0 and not i:
                continue
            total += x // limit
        return total, xs[1:-1], max(xs)


    print(f([3, 4, 7, 10]))
    """
)


def _ctx(tmp_path: Path, n: int, seed: int = 7) -> BuildContext:
    return BuildContext(
        seed=seed, repo=tmp_path, per_family=n, options={"workers": 4, "code_output_mutants": 10}
    )


def _dash_free(text: str) -> bool:
    return chr(0x2014) not in text and chr(0x2013) not in text


# ------------------------------------------------------------------ mutation


def test_python_mutants_parse_differ_and_touch_one_line():
    found = mutants(SAMPLE)
    assert len(found) > 15
    kinds = {m.kind for m in found}
    assert {"binop", "compare", "constant", "boolop", "slice", "call-swap"} <= kinds
    original = ast.dump(ast.parse(SAMPLE))
    for m in found:
        assert ast.dump(ast.parse(m.source)) != original
        changed = [
            i
            for i, (a, b) in enumerate(zip(SAMPLE.splitlines(), m.source.splitlines(), strict=True))
            if a != b
        ]
        assert len(changed) == 1
    assert len({m.source for m in found}) == len(found)


def test_mutant_region_skip_data_and_deletes():
    in_region = mutants(SAMPLE, region=(2, 4))
    assert in_region and all(2 <= m.line <= 4 for m in in_region)
    data_free = mutants(SAMPLE, skip_data=True)
    assert all(m.line != 10 for m in data_free)
    with_delete = mutants(SAMPLE, allow_delete=True)
    assert any(m.kind == "delete" and m.after.strip() == "pass" for m in with_delete)


def test_js_mutants_leave_strings_alone():
    src = 'const s = "a < b";\nfor (let i = 0; i < 3; i++) { console.log(s, i + 1); }\n'
    found = js_mutants(src)
    assert found
    assert all('"a < b"' in m for m in found)
    assert any("var i" in m for m in found)


# ----------------------------------------------------------------- execution


def test_runner_labels_by_execution_and_caches(tmp_path):
    runner = Runner(tmp_path / "cache", workers=2)
    jobs = [
        Job(files={"main.py": "print(sum(range(5)))\n"}, argv=("python", "main.py")),
        Job(files={"main.py": "raise SystemExit(3)\n"}, argv=("python", "main.py")),
    ]
    first = runner.run_many(jobs)
    assert first[0].ok and first[0].stdout == "10\n"
    assert first[1].returncode == 3
    again = Runner(tmp_path / "cache", workers=2)
    assert again.run_many(jobs) == first
    assert again.hits == 2 and again.misses == 0


def test_runner_timeout_and_network_guard(tmp_path):
    runner = Runner(tmp_path / "cache", workers=2)
    loop, net = runner.run_many(
        [
            Job(
                files={"main.py": "while True:\n    pass\n"}, argv=("python", "main.py"), timeout=1
            ),
            Job(
                files={"main.py": "import socket\nsocket.create_connection(('example.com', 80))\n"},
                argv=("python", "main.py"),
            ),
        ]
    )
    assert loop.timed_out and not loop.ok
    assert not net.ok and "network disabled" in net.stderr


# ----------------------------------------------------------------- balancing


def test_shortcut_helpers():
    texts = {"A": "[1, 2]", "B": "[10, 20, 30]", "C": "[3, 4]", "D": "x"}
    assert longest(texts) == "B"
    assert most_common_shape(texts) == "A"
    assert nearest_center(texts, "median") in {"A", "C"}


def test_choice_balancer_drives_a_biased_shortcut_to_chance():
    rng = random.Random(0)
    balancer = ChoiceBalancer()
    hits = 0
    n = 400
    for _ in range(n):
        configs = []
        for _ in range(8):
            order = ["A", "B", "C", "D"]
            correct = rng.choice(order)
            # A shortcut that is right 70% of the time when chosen at random.
            guess = (
                correct if rng.random() < 0.7 else rng.choice([o for o in order if o != correct])
            )
            configs.append(ChoiceConfig(dict.fromkeys(order, ""), correct, {"s": guess}))
        chosen = balancer.choose(configs)
        hits += chosen.shortcuts["s"] == chosen.correct
    assert abs(hits / n - 0.25) < 0.03


def test_balance_noul_is_exactly_half_and_shortcut_is_chance():
    rng = random.Random(1)
    cands = []
    for i in range(600):
        label = rng.random() < 0.5
        feature = rng.random() < (0.8 if label else 0.2)  # strongly predictive
        cands.append((i, label, feature, f"u{i % 25}"))
    chosen = balance_noul(cands, lambda c: c[1], lambda c: (c[2],), 200, rng, group=lambda c: c[3])
    assert len(chosen) == 200
    assert sum(c[1] for c in chosen) == 100
    assert sum(c[1] == c[2] for c in chosen) == 100
    per_unit = [sum(c[3] == f"u{u}" for c in chosen) for u in range(25)]
    assert max(per_unit) <= 13


# ----------------------------------------------------------------- templates


def test_every_program_template_runs_and_has_ten_to_forty_lines():
    for name, template in TEMPLATES.items():
        program = template(random.Random(name))
        source = code_gen.pad_program(program)
        assert 10 <= len(source.splitlines()) <= 40, name
        assert _dash_free(source)
        cmd = [sys.executable, "-c", source] if program.lang == "python" else ["node", "-e", source]
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=10, check=False)
        assert proc.returncode == 0 and proc.stdout.strip(), (name, proc.stderr[-300:])


def test_pad_program_wraps_short_programs():
    short = Program("t", "python", "x = 1\nprint(x)\n", ())
    padded = code_gen.pad_program(short)
    assert len(padded.splitlines()) >= 7 and "def main():" in padded
    js = code_gen.pad_program(Program("t", "javascript", "console.log(1);\n", ()))
    assert js.startswith("function main() {") and js.rstrip().endswith("main();")


def test_type_templates_base_passes_mypy(tmp_path):
    version = mypy_version()
    values = {
        "Cls": "Order",
        "obj": "order",
        "Cls2": "Parcel",
        "obj2": "parcel",
        "fld": "weight",
        "fld2": "score",
        "key": "region",
    }
    bases = [render(t, (), values) for t in TYPE_TEMPLATES.values()]
    assert all(b is not None for b in bases)
    results = mypy_check([b for b in bases if b], tmp_path / "mypy", version)
    assert all(not errs for errs in results.values()), results
    for t in TYPE_TEMPLATES.values():
        assert 20 <= len((render(t, (), values) or "").splitlines()) <= 60, t.name


def test_specs_are_distinct_and_round_detection():
    assert len({s.name for s in SPECS}) == len(SPECS) >= 20
    assert is_round("40") and is_round("[5, 10]") and not is_round("41") and not is_round("'x'")


# -------------------------------------------------------------- small builds


def _check_items(items, family, n):
    assert len(items) == n
    for item in items:
        validate(item)
        assert item.family == family
        assert _dash_free(item.state)
        assert _dash_free(json.dumps(item.question))
        assert set(item.shortcuts.values()) <= set(labels_for(item.question))


def test_code_output_small_build_is_verified_and_deterministic(tmp_path):
    ctx = _ctx(tmp_path, 8)
    items = code_gen.build_code_output(ctx)
    _check_items(items, "code-output", 8)
    for item in items:
        program = re.search(r"```(?:python|javascript)\n(.*)```", item.state, re.S)
        assert program
        lang = item.source["lang"]
        cmd = (
            [sys.executable, "-c", program.group(1)]
            if lang == "python"
            else ["node", "-e", program.group(1)]
        )
        out = subprocess.run(cmd, capture_output=True, text=True, timeout=10, check=True).stdout
        assert item.question["descriptions"][item.answer] == out.strip("\n")
        assert len(set(item.question["descriptions"].values())) == 4
    again = code_gen.build_code_output(ctx)
    assert [i.to_json() for i in again] == [i.to_json() for i in items]


def test_expected_value_build_is_balanced_and_labels_hold(tmp_path):
    items = code_gen.BUILDERS["expected-value"](_ctx(tmp_path, 60))
    _check_items(items, "expected-value", 60)
    assert sum(i.answer for i in items) == 30
    for name in ("round_value", "assertion_above_median"):
        hits = sum(i.shortcuts[name] == answer_label(i.question, i.answer) for i in items)
        assert abs(hits / len(items) - 0.5) <= 0.15
    specs = {s.name: s for s in SPECS}
    for item in items[:15]:
        line = item.state.split("\n\n\n")[1].split("\n")[0]
        name = item.source["spec"]
        namespace: dict[str, object] = {}
        exec(specs[name].reference, namespace)
        call, expected = line[len("assert ") :].split(" == ", 1)
        actual = eval(call, namespace)
        assert (actual == eval(expected)) is item.answer


def test_type_check_pair_small_build_verified_by_mypy(tmp_path):
    items = code_gen.BUILDERS["type-check-pair"](_ctx(tmp_path, 6))
    _check_items(items, "type-check-pair", 6)
    version = mypy_version()
    for item in items:
        snippets = re.findall(r"```python\n(.*?)```", item.state, re.S)
        assert len(snippets) == 2
        results = mypy_check(snippets, tmp_path / "recheck", version)
        passing = ["A", "B"][[not results[s] for s in snippets].index(True)]
        assert [not results[s] for s in snippets].count(True) == 1
        assert passing == item.answer
    assert len({i.source_unit for i in items}) == 6


# ------------------------------------------------------------ library units

TOY_MODULE = textwrap.dedent(
    """\
    def clamp(x, lo, hi):
        if x < lo:
            return lo
        if x > hi:
            return hi
        return x


    def mean(values):
        total = 0
        for v in values:
            total += v
        return total / len(values)


    def span(values):
        lo = min(values)
        hi = max(values)
        return hi - lo
    """
)
TOY_TESTS = textwrap.dedent(
    """\
    from toypkg.stats import clamp, mean, span


    def test_clamp():
        assert clamp(5, 0, 3) == 3
        assert clamp(-1, 0, 3) == 0


    def test_mean():
        assert mean([1, 2, 3, 6]) == 3


    class TestSpan:
        def test_span(self):
            assert span([4, 1, 9]) == 8
    """
)


def test_library_pipeline_on_a_toy_package(tmp_path):
    pkg = tmp_path / "src" / "toypkg"
    (pkg / "tests").mkdir(parents=True)
    (pkg / "__init__.py").write_text("")
    (pkg / "tests" / "__init__.py").write_text("")
    (pkg / "stats.py").write_text(TOY_MODULE)
    (pkg / "tests" / "test_stats.py").write_text(TOY_TESTS)
    unit = LibUnit("toypkg/stats.py", "toypkg/tests/test_stats.py")
    assert [f.name for f in module_functions(TOY_MODULE)] == ["clamp", "mean", "span"]
    assert set(tests_in_file(TOY_TESTS)) == {"test_clamp", "test_mean", "TestSpan::test_span"}
    runner = Runner(tmp_path / "cache", workers=4)
    with workspace(runner, {"toypkg": pkg}) as ws:
        data = load_unit(ws, unit, seed=3, cap=9)
    assert data is not None
    assert data.baseline == {"test_clamp", "test_mean", "TestSpan::test_span"}
    assert data.records
    kills = [r for r in data.records if killed(r, data.baseline)]
    assert kills, "some mutant must be caught"
    for rec in kills:
        failing = killed(rec, data.baseline)
        expected = {"clamp": "test_clamp", "mean": "test_mean", "span": "TestSpan::test_span"}
        assert failing == [expected[rec.function.name]]
    rec = kills[0]
    test_id = killed(rec, data.baseline)[0]
    text = clean_failure(rec.outcome.failures[test_id], unit.tests, ["clamp", "mean", "span"])
    assert ":<line>:" in text
    assert not re.search(r"\b(clamp|mean|span)\(", text)


def test_parse_junit_collapses_parametrised_cases():
    xml = textwrap.dedent(
        """\
        <testsuites><testsuite>
          <testcase classname="pkg.tests.test_x" name="test_a[1]"/>
          <testcase classname="pkg.tests.test_x" name="test_a[2]"><failure message="boom">E boom</failure></testcase>
          <testcase classname="pkg.tests.test_x.TestK" name="test_b"/>
          <testcase classname="pkg.tests.test_x" name="test_c"><skipped/></testcase>
        </testsuite></testsuites>
        """
    )
    passed, failures = parse_junit(xml, "pkg/tests/test_x.py")
    assert passed == {"test_a": False, "TestK::test_b": True}
    assert failures["test_a"] == "E boom"


def test_clean_failure_hides_library_frames_and_line_numbers():
    raw = textwrap.dedent(
        """\
        pkg/tests/test_x.py:12: in test_a
            assert pkg.walk(g) == [1]
        pkg/core.py:88: in walk
            return helper(g)
        <class 'pkg.argmap'> compilation 3:4: in argmap_walk_1
            ???
        E   ValueError: bad at 0x10a3f2c40 in walk
        """
    )
    text = clean_failure(raw, "pkg/tests/test_x.py", ["walk", "helper"])
    assert "pkg/tests/test_x.py:<line>: in test_a" in text
    assert "pkg/core.py" not in text and "argmap" not in text and "helper" not in text
    assert "walk" not in text
    assert "0x..." in text
    assert "frame(s) inside library code omitted" in text


def test_registry_finds_all_five_builders():
    for family in (
        "code-output",
        "type-check-pair",
        "bug-function",
        "mutant-kill",
        "expected-value",
    ):
        assert builder_for(family) is not None


@pytest.mark.parametrize("family", ["code-output", "expected-value"])
def test_builds_do_not_depend_on_worker_count(tmp_path, family):
    a = code_gen.BUILDERS[family](
        BuildContext(
            seed=5,
            repo=tmp_path / "a",
            per_family=4,
            options={"workers": 1, "code_output_mutants": 8},
        )
    )
    b = code_gen.BUILDERS[family](
        BuildContext(
            seed=5,
            repo=tmp_path / "b",
            per_family=4,
            options={"workers": 4, "code_output_mutants": 8},
        )
    )
    assert [i.to_json() for i in a] == [i.to_json() for i in b]
