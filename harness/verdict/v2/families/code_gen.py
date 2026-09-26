"""Builders for the five generated Software families.

``code-output``, ``type-check-pair``, ``bug-function``, ``mutant-kill`` and
``expected-value``. Every answer comes from running code (or mypy) on the
exact text the model sees; nothing is labelled by a model. Builds are
seeded from ``ctx.seed`` plus the family id and cache every execution under
``<repo>/verdict-v2-items/cache``, so a rebuild with the same seed is fast
and produces identical items.

The heavy lifting lives in sibling modules: ``code_gen_exec`` (sandboxed,
cached runs), ``code_gen_mutate`` (single-edit mutants), ``code_gen_select``
(shortcut balancing), ``code_gen_programs`` (code-output templates),
``code_gen_types`` (type-check templates), ``code_gen_libs`` (real library
modules and their test suites) and ``code_gen_specs`` (expected-value specs).
"""

from __future__ import annotations

import math
import random
import re
from collections import Counter
from collections.abc import Callable
from dataclasses import dataclass
from itertools import chain

from harness.verdict.v2.families.code_gen_bugs import build_bug_function
from harness.verdict.v2.families.code_gen_exec import Job, RunResult, runner_for
from harness.verdict.v2.families.code_gen_libs import build_mutant_kill
from harness.verdict.v2.families.code_gen_mutate import js_mutants, mutants
from harness.verdict.v2.families.code_gen_programs import TEMPLATES, Program
from harness.verdict.v2.families.code_gen_select import (
    ChoiceBalancer,
    ChoiceConfig,
    labelled,
    longest,
    medoid,
    most_common_shape,
    nearest_center,
    outlier,
    rng_for,
)
from harness.verdict.v2.families.code_gen_specs import build_expected_value
from harness.verdict.v2.families.code_gen_types import build_type_check_pair
from harness.verdict.v2.items import Item, choice_question, make_item
from harness.verdict.v2.registry import BuildContext

FAMILY_CODE_OUTPUT = "code-output"
MAX_MUTANTS_PER_PROGRAM = 24
CONFIGS_PER_ITEM = 40
MAX_ANCHORS = 5
CHILDREN_PER_ANCHOR = 8


# ------------------------------------------------------------ code-output

LANG_LABEL = {"python": "Python 3 (CPython)", "javascript": "JavaScript (Node.js)"}
FENCE = {"python": "python", "javascript": "javascript"}


def pad_program(program: Program, min_lines: int = 10) -> str:
    """Wrap very short programs in a main function so every program has 10+ lines."""
    source = program.source
    if len(source.splitlines()) >= min_lines:
        return source
    if program.lang == "python":
        body = "".join(f"    {line}\n" if line.strip() else "\n" for line in source.splitlines())
        return f'def main():\n{body}\n\nif __name__ == "__main__":\n    main()\n'
    body = "".join(f"  {line}\n" if line.strip() else "\n" for line in source.splitlines())
    return f"function main() {{\n{body}}}\n\nmain();\n"


def _job(lang: str, source: str) -> Job:
    if lang == "python":
        return Job(files={"main.py": source}, argv=("python", "main.py"))
    return Job(files={"main.js": source}, argv=("node", "main.js"))


_ADDRESS = re.compile(r" at 0x[0-9a-fA-F]+")


def _usable(result_stdout: str, truth: str) -> bool:
    """A wrong output fit to show: deterministic, same line count, similar length."""
    text = result_stdout.strip("\n")
    if not text.strip() or text == truth or _ADDRESS.search(text):
        return False
    if len(text.splitlines()) != len(truth.splitlines()):
        return False
    return len(text) <= 2 * len(truth) + 20


def code_output_shortcuts(texts: dict[str, str]) -> dict[str, str]:
    return {
        "longest": longest(texts),
        "common_shape": most_common_shape(texts),
        "nearest_median": nearest_center(texts, "median"),
        "nearest_mean": nearest_center(texts, "mean"),
        "medoid": medoid(texts),
        "outlier": outlier(texts),
    }


@dataclass(frozen=True)
class _Plan:
    """One program and every variant to run: traps, single edits, and edits of edits.

    ``anchors`` are the traps plus a few single-edit mutants; ``children[k]``
    are single edits of ``anchors[k]``. Their outputs let an option set
    cluster around a wrong answer, so the truth is not simply the option the
    other three each differ from in one place.
    """

    name: str
    program: Program
    source: str
    traps: list[str]
    muts: list[str]
    anchors: list[str]
    children: list[list[str]]


def _edits(lang: str, source: str) -> list[str]:
    if lang == "python":
        return [m.source for m in mutants(source, skip_data=True)]
    return js_mutants(source)


def _plans(
    ctx: BuildContext,
    pending: list[str],
    quota: Counter[str],
    attempt: Counter[str],
    seen: set[str],
) -> list[_Plan]:
    plans = []
    options = ctx.options or {}
    cap = int(options.get("code_output_mutants", MAX_MUTANTS_PER_PROGRAM))
    child_cap = int(options.get("code_output_children", CHILDREN_PER_ANCHOR))
    for name in pending:
        for _ in range(quota[name] + 2):
            attempt[name] += 1
            rng = rng_for(ctx.seed, FAMILY_CODE_OUTPUT, name, attempt[name])
            program = TEMPLATES[name](rng)
            source = pad_program(program)
            if source in seen:
                continue
            seen.add(source)
            traps = [pad_program(Program(name, program.lang, t, ())) for t in program.traps]
            muts = _edits(program.lang, source)
            rng.shuffle(muts)
            muts = muts[:cap]
            anchors = traps[:MAX_ANCHORS] + muts[: max(0, MAX_ANCHORS - len(traps))]
            children = []
            for anchor in anchors:
                kids = [k for k in _edits(program.lang, anchor) if k != source]
                rng.shuffle(kids)
                children.append(kids[:child_cap])
            plans.append(_Plan(name, program, source, traps, muts, anchors, children))
    return plans


def _outputs(runs: list[RunResult], truth: str, exclude: list[str]) -> list[str]:
    texts = dict.fromkeys(r.stdout.strip("\n") for r in runs if r.ok and _usable(r.stdout, truth))
    return [t for t in texts if t not in exclude]


@dataclass(frozen=True)
class _Pools:
    """Wrong outputs of one program: traps, single edits, and clusters of edits."""

    traps: list[str]
    singles: list[str]  # traps first, then single-edit mutants
    clusters: dict[str, list[str]]  # wrong output -> outputs of edits of it


def _distractor_sets(rng: random.Random, pools: _Pools) -> list[tuple[str, list[str]]]:
    """Candidate distractor triples of three shapes.

    ``star``: three single edits of the program (the truth sits at the centre).
    ``cluster``: one wrong output and two edits of it (a wrong answer sits at
    the centre). ``chain``: a wrong output, one edit of it, and another
    single edit. The balancer picks among them so that no shape, and no
    shortcut that reads the shape, predicts the answer.
    """
    trap_first = pools.traps[:1]
    others = [t for t in pools.singles if t not in trap_first]
    anchors = [a for a, kids in pools.clusters.items() if len(kids) >= 1]
    wide = [a for a, kids in pools.clusters.items() if len(kids) >= 2]
    out: list[tuple[str, list[str]]] = []
    for k in range(CONFIGS_PER_ITEM):
        kind = ("star", "cluster", "chain")[k % 3]
        if kind == "cluster" and wide:
            anchor = rng.choice(wide)
            out.append((kind, [anchor, *rng.sample(pools.clusters[anchor], 2)]))
        elif kind == "chain" and anchors:
            anchor = rng.choice(anchors)
            kid = rng.choice(pools.clusters[anchor])
            rest = [t for t in pools.singles if t not in (anchor, kid)]
            if rest:
                out.append((kind, [anchor, kid, rng.choice(rest)]))
        elif len(trap_first) + len(others) >= 3:
            need = 3 - len(trap_first)
            out.append(("star", trap_first + rng.sample(others, need)))
    return out


def _code_output_item(
    ctx: BuildContext,
    plan: _Plan,
    truth: str,
    pools: _Pools,
    balancer: ChoiceBalancer,
) -> Item | None:
    rng = rng_for(ctx.seed, FAMILY_CODE_OUTPUT, plan.name, "options", plan.source)
    configs = []
    for kind, distractors in _distractor_sets(rng, pools):
        if len(set(distractors)) != 3:
            continue
        texts, correct = labelled(rng, truth, distractors)
        configs.append(
            ChoiceConfig(texts, correct, code_output_shortcuts(texts), payload=(kind, distractors))
        )
    if not configs:
        return None
    chosen = balancer.choose(configs)
    kind, distractors = chosen.payload
    lang = plan.program.lang
    state = (
        f"Language: {LANG_LABEL[lang]}\n"
        "The program below runs once with no input and no arguments.\n\n"
        f"```{FENCE[lang]}\n{plan.source}```\n"
    )
    return make_item(
        family=FAMILY_CODE_OUTPUT,
        key=f"{ctx.seed}:{plan.source}",
        source_unit=f"{FAMILY_CODE_OUTPUT}:{plan.name}",
        state=state,
        question=choice_question(
            "Which option is exactly what this program prints to standard output?",
            dict(chosen.texts.items()),
        ),
        answer=chosen.correct,
        reference="execution",
        shortcuts=chosen.shortcuts,
        source={
            "template": plan.name,
            "lang": lang,
            "trap_distractors": sum(t in pools.traps for t in distractors),
            "structure": kind,
            "lines": len(plan.source.splitlines()),
        },
    )


def _pools(
    truth: str,
    trap_runs: list[RunResult],
    mut_runs: list[RunResult],
    anchor_runs: list[RunResult],
    child_runs: list[list[RunResult]],
) -> _Pools:
    trap_texts = _outputs(trap_runs, truth, [])
    mut_texts = _outputs(mut_runs, truth, trap_texts)
    clusters: dict[str, list[str]] = {}
    for anchor_run, runs in zip(anchor_runs, child_runs, strict=True):
        anchor = anchor_run.stdout.strip("\n")
        if not (anchor_run.ok and _usable(anchor_run.stdout, truth)) or anchor in clusters:
            continue
        kids = _outputs(runs, truth, [anchor])
        if kids:
            clusters[anchor] = kids
    return _Pools(trap_texts, trap_texts + mut_texts, clusters)


def build_code_output(ctx: BuildContext) -> list[Item]:
    runner = runner_for(ctx, FAMILY_CODE_OUTPUT)
    names = sorted(TEMPLATES)
    per_template = math.ceil(ctx.per_family / len(names))
    quota = Counter(dict.fromkeys(names, per_template))
    # Trim quotas so the total is exactly the target, spreading the cut.
    cut_rng = rng_for(ctx.seed, FAMILY_CODE_OUTPUT, "quota")
    for name in cut_rng.sample(names, per_template * len(names) - ctx.per_family):
        quota[name] -= 1
    balancer = ChoiceBalancer()
    items: list[Item] = []
    seen: set[str] = set()
    attempt: Counter[str] = Counter()
    exhausted: set[str] = set()
    for _round in range(8):
        pending = [n for n in names if quota[n] > 0]
        if not pending:
            break
        before = Counter(quota)
        plans = _plans(ctx, pending, quota, attempt, seen)
        jobs = [
            _job(p.program.lang, src)
            for p in plans
            for src in [p.source, *p.traps, *p.muts, *p.anchors, *chain(*p.children)]
        ]
        results = iter(runner.run_many(jobs))
        for plan in plans:
            truth_run = next(results)
            trap_runs = [next(results) for _ in plan.traps]
            mut_runs = [next(results) for _ in plan.muts]
            anchor_runs = [next(results) for _ in plan.anchors]
            child_runs = [[next(results) for _ in kids] for kids in plan.children]
            truth = truth_run.stdout.strip("\n")
            if (
                quota[plan.name] <= 0
                or not truth_run.ok
                or not truth.strip()
                or len(truth) > 1500
                or _ADDRESS.search(truth)
            ):
                continue
            item = _code_output_item(
                ctx,
                plan,
                truth,
                _pools(truth, trap_runs, mut_runs, anchor_runs, child_runs),
                balancer,
            )
            if item is not None:
                items.append(item)
                quota[plan.name] -= 1
        # A template that yielded nothing new this round is out of variety:
        # hand its remaining quota to the other templates.
        exhausted.update(n for n in pending if quota[n] == before[n])
        spare = sum(quota[n] for n in exhausted)
        for n in exhausted:
            quota[n] = 0
        live = [n for n in names if n not in exhausted]
        for i in range(spare if live else 0):
            quota[live[i % len(live)]] += 1
    return items


BUILDERS: dict[str, Callable[[BuildContext], list[Item]]] = {
    "code-output": build_code_output,
    "type-check-pair": build_type_check_pair,
    "bug-function": build_bug_function,
    "mutant-kill": build_mutant_kill,
    "expected-value": build_expected_value,
}
