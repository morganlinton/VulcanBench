"""``bug-function``: find the function holding a planted bug, among the functions the test reaches.

Construction rules (hardened 2026-09-25 after the pilot, where Jev scored
skill 96):

- **Candidates are the failing test's call path.** The options are the
  module functions statically reachable from the names the test uses (its
  entry points, their callees, and so on), not a random sample of the
  module. Reading the call graph therefore narrows nothing: every option is
  one the test could have reached.
- **Idiomatic edits only.** Boundary flips, operator swaps, off-by-one
  constants, dropped ``not``, swapped arguments of a call, one local
  variable used in place of another, and the like. Edits that read as
  planted (a statement replaced by ``pass``, a condition wrapped in
  ``not (...)``, odd method swaps) are not used.
- **A wrong result, not a crash.** The failure shown is the test's own
  assertion (a wrong value, or an expected exception that never came),
  with no frames inside the library. A crash names what broke (an unbound
  local, a missing attribute, the library's own error message), which a
  text search of the module maps to one function.
- **More than networkx.** Units from radon, jsonschema, referencing, mando
  and opt_einsum join the networkx modules, and items alternate between
  networkx and the other packages so the memorised library does not
  dominate.

Mutant-kill keeps its own mutants (``code_gen_libs.load_all``); this module
draws a separate set, so changing it never moves that family.
"""

from __future__ import annotations

import ast
import random
import re
from collections import deque
from dataclasses import dataclass
from itertools import pairwise

from harness.verdict.v2.families.code_gen_exec import runner_for
from harness.verdict.v2.families.code_gen_libs import (
    MUTANTS_PER_UNIT,
    UNITS,
    FunctionInfo,
    LibUnit,
    MutantRecord,
    UnitData,
    Workspace,
    clean_failure,
    killed,
    module_functions,
    package_version,
    plain_dashes,
    tests_in_file,
    unit_mutants,
    workspace,
)
from harness.verdict.v2.families.code_gen_mutate import Mutant, mutants
from harness.verdict.v2.families.code_gen_select import ChoiceBalancer, ChoiceConfig
from harness.verdict.v2.items import Item, choice_question, make_item
from harness.verdict.v2.registry import BuildContext

FAMILY = "bug-function"
BUG_MUTANTS_PER_UNIT = 60
WRONG_NAME_PER_FUNCTION = 2
MIN_CANDIDATES = 5
MAX_OPTIONS = 20
STATE_BUDGET = 100_000
BUG_WINDOW = 6
TESTS_PER_MUTANT = 3
CONFIGS_PER_VIEW = 4

EXTRA_UNITS: tuple[LibUnit, ...] = tuple(
    LibUnit(m, t)
    for m, t in [
        ("opt_einsum/parser.py", "opt_einsum/tests/test_parser.py"),
        ("opt_einsum/paths.py", "opt_einsum/tests/test_paths.py"),
        ("opt_einsum/blas.py", "opt_einsum/tests/test_blas.py"),
        ("opt_einsum/sharing.py", "opt_einsum/tests/test_sharing.py"),
        ("opt_einsum/contract.py", "opt_einsum/tests/test_edge_cases.py"),
        ("radon/metrics.py", "radon/tests/test_other_metrics.py"),
        ("radon/complexity.py", "radon/tests/test_complexity_utils.py"),
        ("radon/cli/tools.py", "radon/tests/test_cli_tools.py"),
        ("radon/cli/harvest.py", "radon/tests/test_cli_harvest.py"),
        ("jsonschema/_utils.py", "jsonschema/tests/test_utils.py"),
        ("jsonschema/_format.py", "jsonschema/tests/test_format.py"),
        ("jsonschema/_types.py", "jsonschema/tests/test_types.py"),
        ("referencing/_core.py", "referencing/tests/test_core.py"),
        ("referencing/retrieval.py", "referencing/tests/test_retrieval.py"),
        ("mando/napoleon/docstring.py", "mando/tests/test_google.py"),
    ]
)
BUG_UNITS: tuple[LibUnit, ...] = UNITS + EXTRA_UNITS

# ------------------------------------------------------------------ mutants

# Kinds from ``code_gen_mutate.mutants`` whose edits read like ordinary code.
IDIOMATIC_KINDS = frozenset(
    {
        "binop",
        "augassign",
        "compare",
        "boolop",
        "constant",
        "unary",
        "slice",
        "slice-copy",
        "range",
        "drop-copy",
        "break",
        "continue",
        "call-swap",
        "method-swap",
    }
)
NATURAL_CALL_SWAPS = frozenset({"min", "max", "any", "all"})
NATURAL_METHOD_SWAPS = frozenset(
    {
        "append",
        "extend",
        "startswith",
        "endswith",
        "lstrip",
        "rstrip",
        "find",
        "rfind",
        "keys",
        "values",
        "issubset",
        "issuperset",
        "union",
        "intersection",
        "upper",
        "lower",
        "split",
        "rsplit",
        "remove",
        "discard",
        "popleft",
        "appendleft",
    }
)
_WORD = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")


def _natural(m: Mutant) -> bool:
    """The edited line reads like code someone could have written."""
    if m.kind not in IDIOMATIC_KINDS:
        return False
    if m.kind in {"call-swap", "method-swap"}:
        new = set(_WORD.findall(m.after)) - set(_WORD.findall(m.before))
        allowed = NATURAL_CALL_SWAPS if m.kind == "call-swap" else NATURAL_METHOD_SWAPS
        return bool(new) and new <= allowed
    return "not (" not in m.after or "not (" in m.before


def _pos(node: ast.AST) -> tuple[int, int, int, int]:
    return (
        node.lineno,  # type: ignore[attr-defined]
        node.col_offset,  # type: ignore[attr-defined]
        node.end_lineno or node.lineno,  # type: ignore[attr-defined]
        node.end_col_offset or 0,  # type: ignore[attr-defined]
    )


class _Text:
    """Line/UTF-8-column positions to string offsets, for span edits."""

    def __init__(self, text: str) -> None:
        self.text = text
        self.lines = text.splitlines(keepends=True)
        self.starts = [0]
        for line in self.lines:
            self.starts.append(self.starts[-1] + len(line))

    def offset(self, line: int, col: int) -> int:
        raw = self.lines[line - 1].encode()[:col].decode(errors="replace")
        return self.starts[line - 1] + len(raw)

    def seg(self, node: ast.AST) -> str:
        line, col, end_line, end_col = _pos(node)
        return self.text[self.offset(line, col) : self.offset(end_line, end_col)]

    def replace(self, spans: list[tuple[ast.AST, str]]) -> str:
        out = self.text
        for node, new in sorted(spans, key=lambda s: _pos(s[0]), reverse=True):
            line, col, end_line, end_col = _pos(node)
            out = out[: self.offset(line, col)] + new + out[self.offset(end_line, end_col) :]
        return out


_SIMPLE = (ast.Name, ast.Attribute, ast.Constant, ast.Subscript)


def _local_names(fn: ast.FunctionDef | ast.AsyncFunctionDef) -> set[str]:
    names = {a.arg for a in [*fn.args.posonlyargs, *fn.args.args, *fn.args.kwonlyargs]}
    for node in ast.walk(fn):
        if isinstance(node, ast.Name) and isinstance(node.ctx, ast.Store):
            names.add(node.id)
    names.discard("self")
    names.discard("cls")
    return names


_Spans = list[tuple[ast.AST, str]]
MAX_EXTRA_PER_FUNCTION = 40


def _enclosing(tree: ast.Module, region: tuple[int, int]) -> ast.FunctionDef | None:
    for n in ast.walk(tree):
        if isinstance(n, ast.FunctionDef):
            end = n.end_lineno or n.lineno
            if n.lineno <= region[0] <= end and end >= region[1]:
                return n
    return None


def _sites(
    fn: ast.FunctionDef, text: _Text, region: tuple[int, int]
) -> list[tuple[str, int, _Spans]]:
    locals_ = sorted(_local_names(fn))
    edits: list[tuple[str, int, _Spans]] = []
    for node in ast.walk(fn):
        line = getattr(node, "lineno", 0)
        if not region[0] <= line <= region[1]:
            continue
        if isinstance(node, ast.Call):
            for a, b in pairwise(node.args):
                sa, sb = text.seg(a), text.seg(b)
                if isinstance(a, _SIMPLE) and isinstance(b, _SIMPLE) and sa != sb:
                    edits.append(("swap-args", line, [(a, sb), (b, sa)]))
        elif isinstance(node, ast.Name) and isinstance(node.ctx, ast.Load) and node.id in locals_:
            short = len(node.id) == 1
            same_kind = [n for n in locals_ if n != node.id and (len(n) == 1) == short]
            edits.extend(("wrong-name", line, [(node, other)]) for other in same_kind[:2])
        elif isinstance(node, ast.If) and isinstance(node.test, (ast.Name, ast.Attribute)):
            edits.append(("negate", line, [(node.test, f"not {text.seg(node.test)}")]))
    step = max(1, len(edits) // MAX_EXTRA_PER_FUNCTION)
    return edits[::step][:MAX_EXTRA_PER_FUNCTION]


def extra_mutants(source: str, region: tuple[int, int]) -> list[Mutant]:
    """Idiomatic edits ``mutants`` does not make, within one function.

    ``swap-args``: two adjacent simple positional arguments of a call trade
    places. ``wrong-name``: one read of a local variable uses another local
    of the same function (the classic u-for-v slip). ``negate``: ``if x:``
    becomes ``if not x:`` for a simple condition.
    """
    tree = ast.parse(source)
    text = _Text(source)
    fn = _enclosing(tree, region)
    if fn is None:
        return []
    out: list[Mutant] = []
    seen = {source}
    base_dump = ast.dump(fn)
    for kind, line, spans in _sites(fn, text, region):
        new = text.replace(spans)
        if new in seen or "\n" in "".join(t for _, t in spans):
            continue
        seen.add(new)
        try:
            new_fn = _enclosing(ast.parse(new), (fn.lineno, fn.lineno))
        except SyntaxError:
            continue
        if new_fn is None or ast.dump(new_fn) == base_dump:
            continue
        before = text.lines[line - 1].rstrip("\n")
        after_lines = new.splitlines()
        after = after_lines[line - 1] if line - 1 < len(after_lines) else ""
        out.append(Mutant(kind, line, before, after, new))
    return out


def bug_mutants(
    source: str, functions: list[FunctionInfo], rng: random.Random, cap: int
) -> list[tuple[FunctionInfo, Mutant]]:
    """Up to ``cap`` idiomatic mutants, spread round-robin over the functions."""
    docstrings = frozenset().union(*[f.docstring_lines for f in functions])
    every = [m for m in mutants(source, skip_lines=docstrings) if _natural(m)]
    pools: list[tuple[FunctionInfo, list[Mutant]]] = []
    for fn in functions:
        if fn.statements < 2:
            continue
        found = [m for m in every if fn.body_start <= m.line <= fn.end]
        extra = [
            m
            for m in extra_mutants(source, (fn.body_start, fn.end))
            if m.line not in fn.docstring_lines
        ]
        rng.shuffle(extra)
        # A wrong variable mostly crashes (and crashes are not shown), so a
        # function takes only a few of them.
        wrong = [m for m in extra if m.kind == "wrong-name"][:WRONG_NAME_PER_FUNCTION]
        found += [m for m in extra if m.kind != "wrong-name"] + wrong
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


def load_bug_unit(
    ws: Workspace, unit: LibUnit, seed: int, cap: int = BUG_MUTANTS_PER_UNIT
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
    # The idiomatic subset of mutant-kill's mutants comes first: their runs
    # are already cached. Fresh idiomatic mutants fill the rest.
    shared = [
        (fn, m)
        for fn, m in unit_mutants(
            source, functions, random.Random(f"{seed}:{unit.module}"), MUTANTS_PER_UNIT
        )
        if _natural(m)
    ]
    rng = random.Random(f"{seed}:{FAMILY}:{unit.module}")
    fresh = bug_mutants(source, functions, rng, cap)
    taken = {m.source for _, m in shared}
    picked = (shared + [(fn, m) for fn, m in fresh if m.source not in taken])[:cap]
    outcomes = ws.run(unit, [m.source for _, m in picked])
    records = [
        MutantRecord(unit, fn, m, o)
        for (fn, m), o in zip(picked, outcomes, strict=True)
        if not o.timed_out and not o.broken
    ]
    return UnitData(unit, source, functions, tests, baseline, records)


def load_bug_units(ws: Workspace, seed: int, units: tuple[LibUnit, ...]) -> list[UnitData]:
    out = []
    for unit in units:
        try:
            data = load_bug_unit(ws, unit, seed)
        except (FileNotFoundError, SyntaxError):
            continue
        if data is not None:
            out.append(data)
    return out


# --------------------------------------------------------------- call graph


@dataclass(frozen=True)
class CallGraph:
    """Which module functions each function names, by bare name or class."""

    functions: dict[str, FunctionInfo]
    edges: dict[str, set[str]]
    by_bare: dict[str, set[str]]
    by_class: dict[str, set[str]]

    def targets(self, identifiers: list[str]) -> list[str]:
        """Functions an identifier list names, in order of first mention."""
        out: dict[str, None] = {}
        for ident in identifiers:
            for name in sorted(self.by_bare.get(ident, ())):
                out.setdefault(name)
            for name in sorted(self.by_class.get(ident, ())):
                out.setdefault(name)
        return list(out)

    def distances(self, test_source: str) -> dict[str, int]:
        """BFS distance from the test: 0 for functions it names, 1 for their callees..."""
        idents = _WORD.findall(test_source)
        direct = {n for ident in idents for n in self.by_bare.get(ident, ())}
        via_class = {n for ident in idents for n in self.by_class.get(ident, ())} - direct
        dist = dict.fromkeys(sorted(direct), 0)
        for name in sorted(via_class):
            dist.setdefault(name, 1)
        queue = deque(sorted(dist, key=lambda n: dist[n]))
        while queue:
            name = queue.popleft()
            for nxt in sorted(self.edges.get(name, ())):
                if nxt not in dist:
                    dist[nxt] = dist[name] + 1
                    queue.append(nxt)
        return dist


def _function_nodes(tree: ast.Module, known: dict[str, FunctionInfo]) -> dict[str, ast.AST]:
    nodes: dict[str, ast.AST] = {}
    for node in tree.body:
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and node.name in known:
            nodes[node.name] = node
        elif isinstance(node, ast.ClassDef):
            for sub in node.body:
                name = f"{node.name}.{getattr(sub, 'name', '')}"
                if isinstance(sub, (ast.FunctionDef, ast.AsyncFunctionDef)) and name in known:
                    nodes[name] = sub
    return nodes


def _identifiers_in(node: ast.AST) -> list[str]:
    out: list[str] = []
    for sub in ast.walk(node):
        if isinstance(sub, ast.Name):
            out.append(sub.id)
        elif isinstance(sub, ast.Attribute):
            out.append(sub.attr)
    return out


def call_graph(source: str, functions: list[FunctionInfo]) -> CallGraph:
    known = {f.name: f for f in functions}
    by_bare: dict[str, set[str]] = {}
    by_class: dict[str, set[str]] = {}
    for f in functions:
        by_bare.setdefault(f.bare, set()).add(f.name)
        # Instances reach their dunder methods without naming them.
        if "." in f.name and f.bare.startswith("__") and f.bare.endswith("__"):
            by_class.setdefault(f.name.split(".")[0], set()).add(f.name)
    edges: dict[str, set[str]] = {}
    for name, node in _function_nodes(ast.parse(source), known).items():
        targets: set[str] = set()
        for ident in _identifiers_in(node):
            targets |= by_bare.get(ident, set()) | by_class.get(ident, set())
        targets.discard(name)
        edges[name] = targets
    return CallGraph(known, edges, by_bare, by_class)


# ----------------------------------------------------------------- building


def _tokens(text: str) -> set[str]:
    spaced = re.sub(r"([a-z])([A-Z])", r"\1 \2", text)
    return {t.lower() for t in re.split(r"[^A-Za-z0-9]+|_", spaced) if len(t) >= 3}


def _first_max(options: list[str], key: dict[str, float]) -> str:
    best = options[0]
    for option in options[1:]:
        if key[option] > key[best]:
            best = option
    return best


def bug_shortcuts(
    options: list[str],
    graph: CallGraph,
    dist: dict[str, int],
    visible: str,
    test_source: str,
) -> dict[str, str]:
    """Surface-only guesses; every one must stay near chance."""
    seen = _tokens(visible)
    fns = graph.functions
    overlap = {o: float(len(_tokens(fns[o].bare) & seen)) for o in options}
    length = {o: float(fns[o].end - fns[o].start) for o in options}
    deep = {o: float(dist.get(o, -1)) for o in options}
    shallow = {o: -float(dist.get(o, 99)) for o in options}
    named = graph.targets(_WORD.findall(test_source))
    first_called = next((n for n in named if n in options), options[0])
    callees = sorted(graph.edges.get(first_called, ()))
    direct_callee = next((n for n in callees if n in options and n != first_called), options[0])
    fan_in = {o: float(sum(o in e for e in graph.edges.values())) for o in options}
    return {
        "overlap": _first_max(options, overlap),
        "longest": _first_max(options, length),
        "first_called": first_called,
        "direct_callee": direct_callee,
        "deepest": _first_max(options, deep),
        "shallowest_helper": _first_max(
            options, {o: shallow[o] if deep[o] > 0 else -100.0 for o in options}
        ),
        "most_called": _first_max(options, fan_in),
    }


_ASSERTION = re.compile(r"^E\s+(assert\b|AssertionError\b|Failed: DID NOT RAISE)")


def behavioural(failure: str) -> bool:
    """A wrong result caught by the test's own assertion, not a crash inside the library.

    A crash names what broke (an unbound local, a missing attribute, the
    library's own error message), and a grep of the module for that name
    finds the function. A wrong value or a missing exception does not.
    """
    if "inside library code omitted" in failure:
        return False
    first = next((line for line in failure.splitlines() if line.startswith("E")), "")
    return bool(_ASSERTION.match(first))


@dataclass(frozen=True)
class BugView:
    """One way to present a planted bug: which failing test, which options."""

    data: UnitData
    rec: MutantRecord
    test_id: str
    failure: str
    distance: int


def _bug_configs(
    data: UnitData, graph: CallGraph, rec: MutantRecord, rng: random.Random
) -> list[ChoiceConfig]:
    answer = rec.function.name
    redact = [f.bare for f in data.functions]
    shown = {
        t: clean_failure(rec.outcome.failures[t], data.unit.tests, redact)
        for t in killed(rec, data.baseline)
        if rec.outcome.failures.get(t)
    }
    failing = [t for t, text in shown.items() if behavioural(text)]
    mutable = {f.name for f in data.functions if f.statements >= 2}
    configs = []
    for test_id in failing[:TESTS_PER_MUTANT]:
        test = data.tests[test_id]
        dist = graph.distances(test.source)
        if answer not in dist:
            continue  # the static call path misses it; listing it would give it away
        path = sorted(n for n in dist if n in mutable and n != answer)
        if len(path) + 1 < MIN_CANDIDATES:
            continue
        failure = shown[test_id]
        visible = f"{test_id}\n{failure}\n{test.source}"
        for _ in range(CONFIGS_PER_VIEW):
            options = [answer, *rng.sample(path, min(len(path), MAX_OPTIONS - 1))]
            rng.shuffle(options)
            configs.append(
                ChoiceConfig(
                    dict(zip(options, options, strict=True)),
                    answer,
                    bug_shortcuts(options, graph, dist, visible, test.source),
                    payload=BugView(data, rec, test_id, failure, dist[answer]),
                )
            )
    return configs


def _group(module: str) -> str:
    return "networkx" if module.startswith("networkx/") else "other"


def build_bug_function(ctx: BuildContext) -> list[Item]:
    runner = runner_for(ctx, "libs")
    units = tuple((ctx.options or {}).get("bug_units", BUG_UNITS))
    with workspace(runner, (ctx.options or {}).get("bug_sources")) as ws:
        datas = load_bug_units(ws, ctx.seed, units)
    rng = random.Random(f"{ctx.seed}:{FAMILY}")
    graphs = {d.unit.module: call_graph(d.source, d.functions) for d in datas}
    candidates: dict[str, list[MutantRecord]] = {}
    by_module = {d.unit.module: d for d in datas}
    for data in datas:
        recs = [
            r
            for r in data.records
            if any(r.outcome.failures.get(t) for t in killed(r, data.baseline))
        ]
        rng.shuffle(recs)
        rank: dict[str, int] = {}
        keyed = []
        for i, rec in enumerate(recs):
            keyed.append((rank.get(rec.function.name, 0), i, rec))
            rank[rec.function.name] = rank.get(rec.function.name, 0) + 1
        if keyed:
            candidates[data.unit.module] = [r for _, _, r in sorted(keyed, key=lambda x: x[:2])]
    balancer = ChoiceBalancer(position_weight=0.05)
    items: list[Item] = []
    groups = sorted({_group(m) for m in candidates})
    # Alternate networkx and other packages; within a group, round-robin by module.
    turn = 0
    while len(items) < ctx.per_family and any(candidates[m] for m in candidates):
        live = [g for g in groups if any(candidates[m] for m in candidates if _group(m) == g)]
        group = live[turn % len(live)]
        turn += 1
        modules = sorted(m for m in candidates if _group(m) == group and candidates[m])
        module = min(modules, key=lambda m: (sum(i.source_unit.endswith(m) for i in items), m))
        data = by_module[module]
        window = candidates[module][:BUG_WINDOW]
        configs = [c for rec in window for c in _bug_configs(data, graphs[module], rec, rng)]
        if not configs:
            for rec in window:
                candidates[module].remove(rec)
            continue
        chosen = balancer.choose(configs)
        view = chosen.payload
        assert isinstance(view, BugView)
        candidates[module].remove(view.rec)
        item = _bug_item(ctx, view, chosen)
        if item is not None:
            items.append(item)
    return items


def _bug_item(ctx: BuildContext, view: BugView, chosen: ChoiceConfig) -> Item | None:
    data, rec, test_id, failure = view.data, view.rec, view.test_id, view.failure
    unit = data.unit
    answer = rec.function.name
    test = data.tests[test_id]
    failing = killed(rec, data.baseline)
    others_failing = [t for t in failing if t != test_id]
    listed = ", ".join(others_failing[:8]) + (" and more" if len(others_failing) > 8 else "")
    options = list(chosen.texts)
    version = package_version(unit.package)
    header = (
        f"Module: {unit.module} ({unit.package} {version}).\n"
        "One function in this module has a planted bug: a single small edit. The package's "
        "own test suite for the module now fails.\n\n"
        f"Failing test: {unit.tests}::{test_id}\n"
        f"Other tests failing in the same run ({len(others_failing)}): {listed or 'none'}\n\n"
        f"Test source:\n```python\n{test.source}\n```\n\n"
        "Failure output (line numbers, frames inside library code, and names of this module's "
        "functions are redacted):\n"
        f"```\n{failure}\n```\n\n"
        "Candidate functions (the module functions this test can reach, directly or through "
        f"calls): {', '.join(options)}\n"
    )
    module_block = (
        f"\nModule source, as it is now (with the bug):\n```python\n{rec.mutant.source}```\n"
    )
    if len(header) + len(module_block) > STATE_BUDGET:
        return None
    question = choice_question("Which function contains the planted bug?", dict.fromkeys(options))
    return make_item(
        family=FAMILY,
        key=f"{ctx.seed}:{unit.module}:{rec.mutant.source}:{test_id}",
        source_unit=f"{FAMILY}:{unit.module}",
        state=plain_dashes(header + module_block),
        question=question,
        answer=answer,
        reference="execution",
        shortcuts=chosen.shortcuts,
        source={
            "module": unit.module,
            "tests": unit.tests,
            "package_version": version,
            "mutation": rec.mutant.kind,
            "failing_tests": len(failing),
            "call_distance": view.distance,
            "module_source_included": True,
        },
    )
