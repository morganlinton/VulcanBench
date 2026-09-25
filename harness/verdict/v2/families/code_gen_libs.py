"""Real library modules and their own test suites, for ``bug-function`` and ``mutant-kill``.

Each source unit is one pure-Python module from an installed package plus the
package's own test file for it (networkx, radon, jsonschema, referencing and
mando ship their tests). The package is copied once into a temp directory;
every run hard-links that copy into a fresh directory, overwrites the one
module with a mutant, and runs pytest on the test file with a JUnit report.
One run per mutant gives the outcome of every test in the file, which labels
both families: the planted bug's failing tests (``bug-function``, built in
``code_gen_bugs`` from its own mutants) and, per related test, killed or not
(``mutant-kill``).
"""

from __future__ import annotations

import ast
import difflib
import importlib.util
import random
import re
import shutil
import tempfile
import xml.etree.ElementTree as ET
from collections.abc import Iterator
from contextlib import contextmanager
from dataclasses import dataclass, field
from importlib import metadata
from pathlib import Path

from harness.verdict.v2.families.code_gen_exec import (
    Job,
    Runner,
    content_hash,
    runner_for,
    tree_hash,
)
from harness.verdict.v2.families.code_gen_mutate import Mutant, mutants
from harness.verdict.v2.families.code_gen_select import above_median, balance_noul
from harness.verdict.v2.items import Item, make_item, noul_question
from harness.verdict.v2.registry import BuildContext

PYTEST_TIMEOUT = 30.0
MUTANTS_PER_UNIT = 36


@dataclass(frozen=True)
class LibUnit:
    module: str  # path under site-packages, e.g. "networkx/algorithms/dag.py"
    tests: str  # the package's own test file for it

    @property
    def package(self) -> str:
        return self.module.split("/")[0]


UNITS: tuple[LibUnit, ...] = tuple(
    LibUnit(m, t)
    for m, t in [
        ("networkx/algorithms/dag.py", "networkx/algorithms/tests/test_dag.py"),
        ("networkx/algorithms/cycles.py", "networkx/algorithms/tests/test_cycles.py"),
        ("networkx/algorithms/simple_paths.py", "networkx/algorithms/tests/test_simple_paths.py"),
        (
            "networkx/algorithms/shortest_paths/unweighted.py",
            "networkx/algorithms/shortest_paths/tests/test_unweighted.py",
        ),
        (
            "networkx/algorithms/shortest_paths/generic.py",
            "networkx/algorithms/shortest_paths/tests/test_generic.py",
        ),
        ("networkx/algorithms/cluster.py", "networkx/algorithms/tests/test_cluster.py"),
        ("networkx/algorithms/clique.py", "networkx/algorithms/tests/test_clique.py"),
        ("networkx/algorithms/core.py", "networkx/algorithms/tests/test_core.py"),
        ("networkx/algorithms/euler.py", "networkx/algorithms/tests/test_euler.py"),
        ("networkx/algorithms/matching.py", "networkx/algorithms/tests/test_matching.py"),
        ("networkx/algorithms/chordal.py", "networkx/algorithms/tests/test_chordal.py"),
        ("networkx/algorithms/cuts.py", "networkx/algorithms/tests/test_cuts.py"),
        (
            "networkx/algorithms/link_prediction.py",
            "networkx/algorithms/tests/test_link_prediction.py",
        ),
        ("networkx/algorithms/triads.py", "networkx/algorithms/tests/test_triads.py"),
        ("networkx/algorithms/threshold.py", "networkx/algorithms/tests/test_threshold.py"),
        ("networkx/algorithms/planarity.py", "networkx/algorithms/tests/test_planarity.py"),
        ("networkx/algorithms/graphical.py", "networkx/algorithms/tests/test_graphical.py"),
        (
            "networkx/algorithms/operators/product.py",
            "networkx/algorithms/operators/tests/test_product.py",
        ),
        (
            "networkx/algorithms/community/label_propagation.py",
            "networkx/algorithms/community/tests/test_label_propagation.py",
        ),
        (
            "networkx/algorithms/flow/networksimplex.py",
            "networkx/algorithms/flow/tests/test_networksimplex.py",
        ),
        (
            "networkx/algorithms/isomorphism/vf2pp.py",
            "networkx/algorithms/isomorphism/tests/test_vf2pp.py",
        ),
        (
            "networkx/algorithms/bipartite/matching.py",
            "networkx/algorithms/bipartite/tests/test_matching.py",
        ),
        ("networkx/classes/function.py", "networkx/classes/tests/test_function.py"),
        ("networkx/classes/coreviews.py", "networkx/classes/tests/test_coreviews.py"),
        ("networkx/utils/misc.py", "networkx/utils/tests/test_misc.py"),
        ("networkx/utils/mapped_queue.py", "networkx/utils/tests/test_mapped_queue.py"),
        ("networkx/utils/decorators.py", "networkx/utils/tests/test_decorators.py"),
        ("networkx/generators/classic.py", "networkx/generators/tests/test_classic.py"),
        ("networkx/generators/trees.py", "networkx/generators/tests/test_trees.py"),
        ("networkx/generators/degree_seq.py", "networkx/generators/tests/test_degree_seq.py"),
        ("networkx/readwrite/gml.py", "networkx/readwrite/tests/test_gml.py"),
        ("networkx/readwrite/graph6.py", "networkx/readwrite/tests/test_graph6.py"),
        ("networkx/readwrite/gexf.py", "networkx/readwrite/tests/test_gexf.py"),
        ("networkx/convert.py", "networkx/tests/test_convert.py"),
        ("radon/visitors.py", "radon/tests/test_complexity_visitor.py"),
        ("radon/raw.py", "radon/tests/test_raw.py"),
        ("mando/utils.py", "mando/tests/test_utils.py"),
        ("mando/core.py", "mando/tests/test_core.py"),
        ("referencing/jsonschema.py", "referencing/tests/test_jsonschema.py"),
        ("referencing/exceptions.py", "referencing/tests/test_exceptions.py"),
        ("jsonschema/cli.py", "jsonschema/tests/test_cli.py"),
        ("jsonschema/validators.py", "jsonschema/tests/test_validators.py"),
    ]
)


@dataclass(frozen=True)
class FunctionInfo:
    name: str  # "func" or "Class.method"
    bare: str
    start: int  # first line including decorators
    body_start: int  # first statement after the docstring
    end: int
    docstring_lines: frozenset[int]
    statements: int

    def source(self, text: str) -> str:
        return "\n".join(text.splitlines()[self.start - 1 : self.end]) + "\n"


def _body_info(node: ast.FunctionDef | ast.AsyncFunctionDef) -> tuple[int, frozenset[int], int]:
    body = node.body
    doc_lines: frozenset[int] = frozenset()
    if body and isinstance(body[0], ast.Expr) and isinstance(body[0].value, ast.Constant):
        first = body[0]
        doc_lines = frozenset(range(first.lineno, (first.end_lineno or first.lineno) + 1))
        body = body[1:]
    start = body[0].lineno if body else node.lineno
    return start, doc_lines, len(body)


def module_functions(text: str) -> list[FunctionInfo]:
    """Top-level functions and class methods, in source order, unique names only."""
    tree = ast.parse(text)
    out: list[FunctionInfo] = []

    def add(node: ast.FunctionDef | ast.AsyncFunctionDef, prefix: str) -> None:
        body_start, doc_lines, count = _body_info(node)
        start = min([node.lineno, *[d.lineno for d in node.decorator_list]])
        out.append(
            FunctionInfo(
                name=f"{prefix}{node.name}",
                bare=node.name,
                start=start,
                body_start=body_start,
                end=node.end_lineno or node.lineno,
                docstring_lines=doc_lines,
                statements=count,
            )
        )

    for node in tree.body:
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            add(node, "")
        elif isinstance(node, ast.ClassDef):
            for sub in node.body:
                if isinstance(sub, (ast.FunctionDef, ast.AsyncFunctionDef)):
                    add(sub, f"{node.name}.")
    counts: dict[str, int] = {}
    for f in out:
        counts[f.name] = counts.get(f.name, 0) + 1
    return [f for f in out if counts[f.name] == 1]


@dataclass(frozen=True)
class TestInfo:
    test_id: str  # "test_x" or "TestClass::test_x"
    name: str
    source: str


def tests_in_file(text: str) -> dict[str, TestInfo]:
    """Test functions with the setup code a reader needs (class setup, fixtures)."""
    tree = ast.parse(text)
    lines = text.splitlines()

    def seg(node: ast.AST) -> str:
        start = min(
            [node.lineno, *[d.lineno for d in getattr(node, "decorator_list", [])]]  # type: ignore[attr-defined]
        )
        return "\n".join(lines[start - 1 : node.end_lineno])  # type: ignore[attr-defined]

    fixtures = {
        node.name: seg(node)
        for node in tree.body
        if isinstance(node, ast.FunctionDef)
        and any("fixture" in ast.unparse(d) for d in node.decorator_list)
    }

    def with_fixtures(node: ast.FunctionDef, body: str) -> str:
        used = [a.arg for a in node.args.args if a.arg in fixtures]
        return "\n\n".join([*[fixtures[u] for u in used], body])

    out: dict[str, TestInfo] = {}
    for node in tree.body:
        if isinstance(node, ast.FunctionDef) and node.name.startswith("test"):
            out[node.name] = TestInfo(node.name, node.name, with_fixtures(node, seg(node)))
        elif isinstance(node, ast.ClassDef) and node.name.startswith("Test"):
            setup = [
                seg(sub)
                for sub in node.body
                if isinstance(sub, ast.FunctionDef)
                and (sub.name.startswith("setup") or sub.name.startswith("_"))
                and sub.name not in {"__init__"}
            ]
            header = lines[node.lineno - 1]
            for sub in node.body:
                if isinstance(sub, ast.FunctionDef) and sub.name.startswith("test"):
                    body = "\n\n".join([*setup, seg(sub)])
                    source = header + "\n" + body
                    out[f"{node.name}::{sub.name}"] = TestInfo(
                        f"{node.name}::{sub.name}", sub.name, with_fixtures(sub, source)
                    )
    return out


@dataclass
class Outcome:
    """One pytest run: per test function, passed or not, and failure text."""

    timed_out: bool
    broken: bool  # no report (collection or import failure)
    passed: dict[str, bool] = field(default_factory=dict)
    failures: dict[str, str] = field(default_factory=dict)

    @property
    def failed(self) -> list[str]:
        return [t for t, ok in self.passed.items() if not ok]


def parse_junit(xml_text: str, tests_path: str) -> tuple[dict[str, bool], dict[str, str]]:
    """Collapse parametrised cases to their test function: passes only if all pass."""
    dotted = tests_path[:-3].replace("/", ".")
    passed: dict[str, bool] = {}
    failures: dict[str, str] = {}
    root = ET.fromstring(xml_text)
    for case in root.iter("testcase"):
        if case.find("skipped") is not None:
            continue
        classname = case.get("classname", "")
        rest = classname[len(dotted) :].lstrip(".") if classname.startswith(dotted) else ""
        name = case.get("name", "").split("[")[0]
        test_id = f"{rest}::{name}" if rest else name
        bad = case.find("failure")
        if bad is None:
            bad = case.find("error")
        ok = bad is None
        passed[test_id] = passed.get(test_id, True) and ok
        if bad is not None and test_id not in failures:
            failures[test_id] = (bad.text or bad.get("message") or "").strip()
    return passed, failures


class Workspace:
    """Copies of the unit packages and a runner that tests module variants."""

    def __init__(self, runner: Runner, root: Path, sources: dict[str, Path] | None = None) -> None:
        self.runner = runner
        self.root = root
        self.sources = sources or {}
        self.bases: dict[str, tuple[Path, str]] = {}

    def locate(self, package: str) -> Path:
        if package in self.sources:
            return self.sources[package]
        spec = importlib.util.find_spec(package)
        if spec is None or not spec.submodule_search_locations:
            raise FileNotFoundError(f"package {package} is not installed")
        return Path(next(iter(spec.submodule_search_locations)))

    def base(self, package: str) -> tuple[Path, str]:
        if package not in self.bases:
            src = self.locate(package)
            dest = self.root / package
            shutil.copytree(src, dest / package, ignore=shutil.ignore_patterns("__pycache__"))
            self.bases[package] = (
                dest,
                tree_hash(dest / package, (".py", ".txt", ".json", ".gml")),
            )
        return self.bases[package]

    def original(self, unit: LibUnit) -> str:
        base, _ = self.base(unit.package)
        return (base / unit.module).read_text()

    def job(self, unit: LibUnit, module_source: str) -> Job:
        base, key = self.base(unit.package)
        return Job(
            files={unit.module: module_source},
            argv=(
                "python-module",
                "pytest",
                unit.tests,
                "-q",
                "-p",
                "no:cacheprovider",
                "--tb=short",
                "--junitxml=report.xml",
                "-o",
                "junit_family=xunit1",
                "-o",
                "addopts=",
                "-W",
                "ignore",
            ),
            timeout=PYTEST_TIMEOUT,
            base=base,
            base_key=key,
            collect=("report.xml",),
        )

    def run(self, unit: LibUnit, sources: list[str]) -> list[Outcome]:
        results = self.runner.run_many([self.job(unit, s) for s in sources])
        out = []
        for result in results:
            report = result.collected.get("report.xml")
            if result.timed_out:
                out.append(Outcome(timed_out=True, broken=False))
                continue
            if not report:
                out.append(Outcome(timed_out=False, broken=True))
                continue
            try:
                passed, failures = parse_junit(report, unit.tests)
            except ET.ParseError:
                out.append(Outcome(timed_out=False, broken=True))
                continue
            out.append(Outcome(False, not passed, passed, failures))
        return out


@contextmanager
def workspace(runner: Runner, sources: dict[str, Path] | None = None) -> Iterator[Workspace]:
    with tempfile.TemporaryDirectory(prefix="verdict-v2-libs-") as tmp:
        yield Workspace(runner, Path(tmp), sources)


@dataclass
class MutantRecord:
    unit: LibUnit
    function: FunctionInfo
    mutant: Mutant
    outcome: Outcome


@dataclass
class UnitData:
    unit: LibUnit
    source: str
    functions: list[FunctionInfo]
    tests: dict[str, TestInfo]
    baseline: set[str]  # tests passing on the original
    records: list[MutantRecord]


def unit_mutants(
    source: str, functions: list[FunctionInfo], rng: random.Random, cap: int
) -> list[tuple[FunctionInfo, Mutant]]:
    """Up to ``cap`` mutants spread round-robin over the mutable functions."""
    pools: list[tuple[FunctionInfo, list[Mutant]]] = []
    docstrings = frozenset().union(*[f.docstring_lines for f in functions])
    every = mutants(source, allow_delete=True, skip_lines=docstrings)
    for fn in functions:
        if fn.statements < 2:
            continue
        found = [m for m in every if fn.body_start <= m.line <= fn.end]
        if found:
            rng.shuffle(found)
            pools.append((fn, found))
    rng.shuffle(pools)
    out: list[tuple[FunctionInfo, Mutant]] = []
    while len(out) < cap and any(p for _, p in pools):
        for fn, pool in pools:
            if pool and len(out) < cap:
                out.append((fn, pool.pop()))
    return out


def load_unit(
    ws: Workspace, unit: LibUnit, seed: int, cap: int = MUTANTS_PER_UNIT
) -> UnitData | None:
    base, _ = ws.base(unit.package)
    source = (base / unit.module).read_text()
    tests_text = (base / unit.tests).read_text()
    functions = module_functions(source)
    tests = tests_in_file(tests_text)
    baseline_run = ws.run(unit, [source])[0]
    if baseline_run.timed_out or baseline_run.broken:
        return None
    baseline = {t for t, ok in baseline_run.passed.items() if ok and t in tests}
    rng = random.Random(f"{seed}:{unit.module}")
    picked = unit_mutants(source, functions, rng, cap)
    outcomes = ws.run(unit, [m.source for _, m in picked])
    records = [
        MutantRecord(unit, fn, m, o)
        for (fn, m), o in zip(picked, outcomes, strict=True)
        if not o.timed_out and not o.broken
    ]
    return UnitData(unit, source, functions, tests, baseline, records)


def killed(record: MutantRecord, baseline: set[str]) -> list[str]:
    return [t for t in record.outcome.failed if t in baseline]


_FRAME = re.compile(r"^(?P<path>[\w./-]+\.py):(?P<line>\d+): (?P<rest>.*)$")
_OTHER_FRAME = re.compile(r"^\S.*:\d+(?::\d+)?: in \S+\s*$")


def clean_failure(text: str, tests_path: str, redact: list[str], limit: int = 6000) -> str:
    """Keep test-file frames and error lines; hide library frames, line numbers, names."""
    out: list[str] = []
    skipped = 0
    keep = True
    for line in text.splitlines():
        match = _FRAME.match(line)
        if match:
            keep = match.group("path") == tests_path
            if keep:
                if skipped:
                    out.append(f"    ... ({skipped} frame(s) inside library code omitted)")
                    skipped = 0
                out.append(f"{match.group('path')}:<line>: {match.group('rest')}")
            else:
                skipped += 1
            continue
        if _OTHER_FRAME.match(line):
            keep = False
            skipped += 1
            continue
        if line.startswith("E ") or line.startswith("E\t") or line == "E":
            if skipped:
                out.append(f"    ... ({skipped} frame(s) inside library code omitted)")
                skipped = 0
            keep = True
            out.append(line)
            continue
        if keep:
            out.append(line)
    if skipped:
        out.append(f"    ... ({skipped} frame(s) inside library code omitted)")
    cleaned = "\n".join(out)
    cleaned = re.sub(r"\bline \d+\b", "line <n>", cleaned)
    cleaned = re.sub(r"0x[0-9a-fA-F]{6,}", "0x...", cleaned)
    for name in sorted(set(redact), key=len, reverse=True):
        cleaned = re.sub(
            rf"(?<![A-Za-z0-9]){re.escape(name)}(?![A-Za-z0-9])", "<function>", cleaned
        )
    if len(cleaned) > limit:
        cleaned = cleaned[: limit // 2] + "\n    ... (output truncated)\n" + cleaned[-limit // 2 :]
    return cleaned


def plain_dashes(text: str) -> str:
    """Suite text never carries em or en dashes; a few library docstrings do.

    Only docstrings and comments of the quoted library code contain them, so
    swapping them for ASCII does not change what the code does.
    """
    return text.replace("\u2014", "--").replace("\u2013", "-")


def function_diff(unit: LibUnit, original: str, mutated: str) -> str:
    diff = difflib.unified_diff(
        original.splitlines(keepends=True),
        mutated.splitlines(keepends=True),
        fromfile=f"a/{unit.module}",
        tofile=f"b/{unit.module}",
        n=3,
    )
    return "".join(diff)


def package_version(package: str) -> str:
    try:
        return metadata.version(package)
    except metadata.PackageNotFoundError:
        return "unknown"


def unit_key(unit: LibUnit, source: str) -> str:
    return content_hash(unit.module, unit.tests, source)


# ----------------------------------------------------------------- builders

_LOADED: dict[tuple[int, str], list[UnitData]] = {}


def load_all(ws: Workspace, seed: int) -> list[UnitData]:
    """Every unit's mutants and outcomes; memoised so both families share one load."""
    key = (seed, str(ws.runner.cache_dir))
    if key not in _LOADED:
        _LOADED[key] = _load_all(ws, seed)
    return _LOADED[key]


def _load_all(ws: Workspace, seed: int) -> list[UnitData]:
    out = []
    for unit in UNITS:
        try:
            data = load_unit(ws, unit, seed)
        except (FileNotFoundError, SyntaxError):
            continue
        if data is not None:
            out.append(data)
    return out


def _identifiers(text: str) -> list[str]:
    return re.findall(r"[A-Za-z_][A-Za-z0-9_]*", text)


@dataclass(frozen=True)
class KillCandidate:
    data: UnitData
    rec: MutantRecord
    test: str
    label: bool
    diff: str
    test_source: str

    @property
    def line_in_test(self) -> bool:
        return self.rec.mutant.before.strip() in self.test_source

    @property
    def name_in_test(self) -> bool:
        return self.rec.function.bare in _identifiers(self.test_source)


def kill_pool(datas: list[UnitData], rng: random.Random) -> list[KillCandidate]:
    """(mutant, related test) pairs; a related test is one some mutant of the function kills."""
    pool: list[KillCandidate] = []
    for data in datas:
        related: dict[str, set[str]] = {}
        for rec in data.records:
            related.setdefault(rec.function.name, set()).update(killed(rec, data.baseline))
        for rec in data.records:
            tests = sorted(t for t in related.get(rec.function.name, set()) if t in data.tests)
            fails = set(killed(rec, data.baseline))
            pos = [t for t in tests if t in fails]
            neg = [t for t in tests if t not in fails]
            rng.shuffle(pos)
            rng.shuffle(neg)
            diff = function_diff(data.unit, data.source, rec.mutant.source)
            for test_id in pos[:2] + neg[:2]:
                test_source = data.tests[test_id].source
                if len(test_source) <= 6000:
                    pool.append(
                        KillCandidate(data, rec, test_id, test_id in fails, diff, test_source)
                    )
    return pool


def _yes(flag: bool) -> str:
    return "true" if flag else "false"


def build_mutant_kill(ctx: BuildContext) -> list[Item]:
    family = "mutant-kill"
    runner = runner_for(ctx, "libs")
    with workspace(runner) as ws:
        datas = load_all(ws, ctx.seed)
    rng = random.Random(f"{ctx.seed}:{family}")
    pool = kill_pool(datas, rng)
    if not pool:
        return []
    diff_cut = above_median([len(c.diff) for c in pool])
    test_cut = above_median([len(c.test_source) for c in pool])
    chosen = balance_noul(
        pool,
        lambda c: c.label,
        lambda c: (
            diff_cut(len(c.diff)),
            test_cut(len(c.test_source)),
            c.line_in_test,
            c.name_in_test,
        ),
        ctx.per_family,
        rng,
        group=lambda c: c.data.unit.module,
    )
    final_diff = above_median([len(c.diff) for c in chosen])
    final_test = above_median([len(c.test_source) for c in chosen])
    items = []
    for c in chosen:
        unit = c.data.unit
        version = package_version(unit.package)
        state = (
            f"Library: {unit.package} {version}. Function under change, from {unit.module}:\n"
            f"```python\n{c.rec.function.source(c.data.source)}```\n\n"
            "Mutant (a single edit to that function):\n"
            f"```diff\n{c.diff}```\n\n"
            f"Test {unit.tests}::{c.test} (it passes on the original code):\n"
            f"```python\n{c.test_source}\n```\n"
        )
        items.append(
            make_item(
                family=family,
                key=f"{ctx.seed}:{unit.module}:{c.rec.mutant.source}:{c.test}",
                source_unit=f"{family}:{unit.module}",
                state=plain_dashes(state),
                question=noul_question("Does this test fail when it is run against the mutant?"),
                answer=c.label,
                reference="execution",
                shortcuts={
                    "diff_above_median": _yes(final_diff(len(c.diff))),
                    "test_above_median": _yes(final_test(len(c.test_source))),
                    "line_in_test": _yes(c.line_in_test),
                    "name_in_test": _yes(c.name_in_test),
                },
                source={
                    "module": unit.module,
                    "tests": unit.tests,
                    "package_version": version,
                    "mutation": c.rec.mutant.kind,
                },
            )
        )
    return items
