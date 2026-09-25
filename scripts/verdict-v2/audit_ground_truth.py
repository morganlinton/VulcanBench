#!/usr/bin/env python3
"""Audit Verdict v2 ground truth on a fixed random sample (admission gate rule 5).

Draws a fixed-seed 5% sample per family (at least 10 items each) from the item
file and re-derives every sampled answer as independently of the family
builders as the stored data allows:

- general families and ``incident-root-cause``: the rendered state text is
  parsed and solved by small solvers written here (brute force over options,
  truth tables, small-model enumeration, table recomputation, a pairwise
  precedence engine, arithmetic from the stated numbers, a log tracer);
- ``code-output``, ``type-check-pair``, ``mutant-kill``, ``bug-function``:
  the code in the state is executed fresh in a sandbox (no network, scrubbed
  environment, temp directory), never read from the builder's cache;
- ``expected-value``: an implementation written here from each docstring,
  plus the builder's hidden reference re-run in the sandbox;
- ``patch-pair`` and ``failing-test``: the source run's ``summary.json`` in
  the archive, re-read and re-derived from the verifier fields;
- ``fix-file``, ``vuln-pair``, ``weakness-class``, ``semver-impact``: the raw
  mined cache records.

Writes ``verdict-v2-items/audit-ground-truth.json`` (private, like the item
file) and prints a per-family table. Item content is never printed. Makes no
network or model calls. Run it under ``nice`` with at most two workers while a
wall-clock-ranked sweep is running.
"""

from __future__ import annotations

import argparse
import ast
import calendar
import copy
import difflib
import gzip
import hashlib
import importlib.util
import itertools
import json
import math
import os
import random
import re
import shutil
import subprocess
import sys
import tempfile
import threading
import xml.etree.ElementTree as ET
from collections import Counter, defaultdict
from collections.abc import Callable, Iterable, Sequence
from concurrent.futures import ThreadPoolExecutor
from dataclasses import asdict, dataclass, field
from datetime import UTC, date, datetime, timedelta
from decimal import ROUND_HALF_UP, Decimal
from fractions import Fraction
from importlib import metadata
from pathlib import Path, PurePosixPath
from typing import Any

REPO = Path(__file__).resolve().parents[2]
if str(REPO) not in sys.path:
    sys.path.insert(0, str(REPO))

from harness.tasks import load_task, task_hash  # noqa: E402
from harness.verdict.v2.families import ops_gen  # noqa: E402
from harness.verdict.v2.families.code_gen_specs import SPECS  # noqa: E402
from harness.verdict.v2.families.mined import CWE_FAMILIES, label_semver  # noqa: E402
from harness.verdict.v2.registry import BuildContext  # noqa: E402

# Run archives and task trees live in the main checkout (read only here).
MAIN_CHECKOUT = Path.home() / "dev" / "VulcanBench"
ITEMS_DIR = REPO / "verdict-v2-items"
DEFAULT_ITEMS = ITEMS_DIR / "items.jsonl"
DEFAULT_OUT = ITEMS_DIR / "audit-ground-truth.json"
SAMPLE_SEED = 20260925
SAMPLE_SHARE = 0.05
MIN_PER_FAMILY = 10
MAX_WORKERS = 2
# The build that produced items.jsonl (build_items.py defaults, 300 per family).
BUILD_SEED = 20260924
BUILD_PER_FAMILY = 300

VERIFIED = "verified"
MISMATCH = "mismatch"
UNVERIFIABLE = "unverifiable"


# ---------------------------------------------------------------------------
# Results and sampling
# ---------------------------------------------------------------------------


@dataclass
class Result:
    item_id: str
    family: str
    status: str
    method: str
    reason: str = ""
    notes: list[str] = field(default_factory=list)


class AuditError(Exception):
    """The audit could not re-derive the answer (unverifiable, with the reason)."""


def per_family_quota(total: int, share: float = SAMPLE_SHARE, floor: int = MIN_PER_FAMILY) -> int:
    return min(total, max(floor, math.ceil(share * total)))


def sample_items(
    items: Sequence[dict[str, Any]],
    seed: int = SAMPLE_SEED,
    share: float = SAMPLE_SHARE,
    floor: int = MIN_PER_FAMILY,
) -> dict[str, list[dict[str, Any]]]:
    """A fixed-seed random sample per family, stable under item file reordering."""
    by_family: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for item in items:
        by_family[item["family"]].append(item)
    out: dict[str, list[dict[str, Any]]] = {}
    for family in sorted(by_family):
        pool = sorted(by_family[family], key=lambda it: it["item_id"])
        rng = random.Random(f"verdict-v2-audit:{seed}:{family}")
        out[family] = rng.sample(pool, per_family_quota(len(pool), share, floor))
    return out


def option_text(item: dict[str, Any], label: str) -> str:
    desc = item["question"].get("descriptions") or {}
    text = desc.get(label)
    return str(text if text is not None else label)


def labels(item: dict[str, Any]) -> list[str]:
    q = item["question"]
    if q["type"] == "noul":
        return ["true", "false"]
    if q["type"] == "choice":
        return [str(o) for o in q["options"]]
    return [str(level) for level in q["levels"]]


def expect_label(item: dict[str, Any], derived: str, method: str, notes: list[str]) -> Result:
    """Compare a derived answer label with the stored one."""
    stored = item["answer"]
    stored_label = ("true" if stored else "false") if isinstance(stored, bool) else str(stored)
    if derived == stored_label:
        return Result(item["item_id"], item["family"], VERIFIED, method, "", notes)
    return Result(
        item["item_id"],
        item["family"],
        MISMATCH,
        method,
        f"re-derived answer {derived!r}, stored {stored_label!r}",
        notes,
    )


def unique_match(item: dict[str, Any], text: str) -> str:
    """The one option whose description equals ``text`` (raises if none or several)."""
    hits = [lab for lab in labels(item) if option_text(item, lab) == text]
    if len(hits) != 1:
        raise AuditError(f"{len(hits)} options match the re-derived value")
    return hits[0]


# ---------------------------------------------------------------------------
# Sandboxed execution (fresh temp directory, no network, scrubbed environment)
# ---------------------------------------------------------------------------

PY_GUARD = """\
import runpy
import socket
import sys


def _blocked(*args, **kwargs):
    raise OSError("network disabled in the audit sandbox")


socket.socket.connect = _blocked
socket.socket.connect_ex = _blocked
socket.getaddrinfo = _blocked
socket.create_connection = _blocked
sys.path.insert(0, ".")
_mode, _target, *_rest = sys.argv[1:]
sys.argv = [_target, *_rest]
if _mode == "module":
    runpy.run_module(_target, run_name="__main__", alter_sys=True)
else:
    runpy.run_path(_target, run_name="__main__")
"""
GUARD_NAME = "_audit_guard.py"


@dataclass(frozen=True)
class RunOut:
    stdout: str
    stderr: str
    returncode: int
    timed_out: bool
    collected: dict[str, str]

    @property
    def ok(self) -> bool:
        return self.returncode == 0 and not self.timed_out


def sandbox_env(home: Path, hash_seed: str = "0") -> dict[str, str]:
    dead = "http://127.0.0.1:9"
    return {
        "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
        "HOME": str(home),
        "TMPDIR": str(home),
        "LANG": "C.UTF-8",
        "LC_ALL": "C.UTF-8",
        "PYTHONDONTWRITEBYTECODE": "1",
        "PYTHONHASHSEED": hash_seed,
        "PYTHONIOENCODING": "utf-8",
        "PYTHONNOUSERSITE": "1",
        "NO_COLOR": "1",
        "http_proxy": dead,
        "https_proxy": dead,
        "HTTP_PROXY": dead,
        "HTTPS_PROXY": dead,
        "ALL_PROXY": dead,
    }


def run_sandboxed(
    files: dict[str, str],
    argv: Sequence[str],
    *,
    timeout: float = 20.0,
    base: Path | None = None,
    collect: Sequence[str] = (),
    hash_seed: str = "0",
    python: str | None = None,
) -> RunOut:
    """Write ``files`` into a fresh directory (hard-linked over ``base``) and run ``argv``.

    ``argv[0]`` of ``"python"`` runs a script under the network guard,
    ``"python-module"`` runs ``-m argv[1]`` under the guard, anything else is
    an executable on PATH.
    """
    with tempfile.TemporaryDirectory(prefix="verdict-v2-audit-") as tmp:
        root = Path(tmp)
        work, home = root / "work", root / "home"
        home.mkdir()
        if base is not None:
            shutil.copytree(base, work, copy_function=os.link)
        else:
            work.mkdir()
        for rel, text in files.items():
            target = work / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            if target.exists():
                target.unlink()  # never write through a hard link
            target.write_text(text)
        head, *rest = argv
        interpreter = python or sys.executable
        if head in ("python", "python-module"):
            (work / GUARD_NAME).write_text(PY_GUARD)
            mode = "path" if head == "python" else "module"
            cmd = [interpreter, "-B", "-s", GUARD_NAME, mode, *rest]
        elif head == "python-raw":
            cmd = [interpreter, "-B", "-s", *rest]
        else:
            cmd = [head, *rest]
        try:
            proc = subprocess.run(
                cmd,
                cwd=work,
                env=sandbox_env(home, hash_seed),
                capture_output=True,
                timeout=timeout,
                stdin=subprocess.DEVNULL,
                check=False,
            )
            out = RunOut(
                proc.stdout.decode("utf-8", "replace"),
                proc.stderr.decode("utf-8", "replace"),
                proc.returncode,
                False,
                {},
            )
        except subprocess.TimeoutExpired:
            out = RunOut("", "", -9, True, {})
        collected = {}
        for rel in collect:
            path = work / rel
            if path.exists():
                collected[rel] = path.read_text(errors="replace")
        return RunOut(out.stdout, out.stderr, out.returncode, out.timed_out, collected)


def fenced_blocks(text: str) -> list[tuple[str, str]]:
    """(info string, body) of every fenced block, in order; bodies keep their newlines."""
    out = []
    for m in re.finditer(r"^```([\w+-]*)\n(.*?)^```", text, re.S | re.M):
        out.append((m.group(1), m.group(2)))
    return out


# ---------------------------------------------------------------------------
# constraint-pick: parse the rules and options, brute-force each option
# ---------------------------------------------------------------------------

_NAME = r"[A-Z][a-z]+"
# Per scenario: the wording of each positional relation, written from reading
# the rendered items. {A}, {B} stand for entity names.
CP_WORDING: dict[str, dict[str, str]] = {
    "couriers": {
        "before": "{A} has an earlier window than {B}",
        "imm": "{A}'s window comes immediately before {B}'s",
        "adj": "{A} and {B} have neighbouring windows",
        "nadj": "{A} and {B} do not have neighbouring windows",
        "gap": "there (?:is|are) exactly {K} other windows? between {A}'s window and {B}'s",
        "at": "{A} has window {S}",
        "not_at": "{A} does not have window {S}",
        "at_end": "{A} has window 1 or window {N}",
        "even": "{A} has an even-numbered window",
        "holder": "the courier with window {S}",
        "group_before": "has an earlier window than",
    },
    "traders": {
        "before": "{A}'s stall is west of {B}'s",
        "imm": "{A}'s stall is immediately west of {B}'s",
        "adj": "{A} and {B} rent stalls next to each other",
        "nadj": "{A} and {B} do not rent stalls next to each other",
        "gap": "there (?:is|are) exactly {K} other stalls? between {A}'s stall and {B}'s",
        "at": "{A} rents stall {S}",
        "not_at": "{A} does not rent stall {S}",
        "at_end": "{A} rents stall 1 or stall {N}",
        "even": "{A} rents an even-numbered stall",
        "holder": "the trader in stall {S}",
        "group_before": "has a stall west of the stall of",
    },
    "tenants": {
        "before": "{A} lives on a lower floor than {B}",
        "imm": "{A} lives on the floor directly below {B}'s",
        "adj": "{A} and {B} live on adjacent floors",
        "nadj": "{A} and {B} do not live on adjacent floors",
        "gap": "there (?:is|are) exactly {K} other floors? between {A}'s floor and {B}'s",
        "at": "{A} lives on floor {S}",
        "not_at": "{A} does not live on floor {S}",
        "at_end": "{A} lives on floor 1 or floor {N}",
        "even": "{A} lives on an even-numbered floor",
        "holder": "the tenant on floor {S}",
        "group_before": "lives on a lower floor than",
    },
    "runners": {
        "before": "{A} finishes ahead of {B}",
        "imm": "{A} finishes immediately ahead of {B}",
        "adj": "{A} and {B} finish in consecutive places",
        "nadj": "{A} and {B} do not finish in consecutive places",
        "gap": "there (?:is|are) exactly {K} other runners? finishing between {A} and {B}",
        "at": "{A} finishes in place {S}",
        "not_at": "{A} does not finish in place {S}",
        "at_end": "{A} finishes in place 1 or place {N}",
        "even": "{A} finishes in an even-numbered place",
        "holder": "the runner in place {S}",
        "group_before": "finishes ahead of",
    },
}


def _fill(template: str, **groups: str) -> str:
    """Turn a wording into a regex: {A} -> (?P<a>Name), {S} -> (?P<s>digits) and so on."""
    out = re.escape(template)
    # re.escape escapes braces and the regex bits we wrote into wordings; undo those.
    out = out.replace(r"\(\?:is\|are\)", "(?:is|are)").replace(r"s\?", "s?")
    for key, name in groups.items():
        pattern = rf"(?P<{name}>{_NAME})" if key in "AB" else rf"(?P<{name}>\d+)"
        out = out.replace(re.escape("{" + key + "}"), pattern)
    return out


@dataclass(frozen=True)
class CPWorld:
    frame: str
    names: list[str]
    n: int
    verb3: str
    verb: str
    noun: str
    kinds: list[str]


def cp_world(state: str) -> CPWorld:
    m = re.match(
        r"^(\w+) (couriers|traders|tenants|runners), (.+?), (?:each|finish) .*? numbered 1 \([^)]*\)"
        r" to (\d+) \(",
        state,
    )
    if not m:
        raise AuditError("constraint-pick: intro not recognised")
    names = re.split(r", | and ", m.group(3))
    n = int(m.group(4))
    if len(names) != n:
        raise AuditError("constraint-pick: entity count differs from the numbering")
    verb3 = verb = noun = ""
    kinds: list[str] = []
    b = re.search(r"^Each \w+ also (\w+) exactly one (\w+), of kind (.+)\.$", state, re.M)
    if b:
        verb3, noun = b.group(1), b.group(2)
        verb = verb3[:-3] + "y" if verb3.endswith("ies") else verb3[:-1]
        kinds = re.split(r", | or ", b.group(3))
    return CPWorld(m.group(2), names, n, verb3, verb, noun, kinds)


CPRule = tuple[str, dict[str, str]]


def cp_rule_patterns(world: CPWorld) -> list[tuple[str, re.Pattern[str]]]:
    w = CP_WORDING[world.frame]
    pats: list[tuple[str, str]] = [
        ("before", _fill(w["before"], A="a", B="b")),
        ("imm", _fill(w["imm"], A="a", B="b")),
        ("adj", _fill(w["adj"], A="a", B="b")),
        ("nadj", _fill(w["nadj"], A="a", B="b")),
        ("gap", _fill(w["gap"], A="a", B="b", K="k")),
        ("not_at", _fill(w["not_at"], A="a", S="s")),
        ("at_end", _fill(w["at_end"], A="a", N="n")),
        (
            "cond_even",
            "if " + _fill(w["even"], A="a") + ", then " + _fill(w["before"], A="b", B="c"),
        ),
        (
            "xor_end",
            "either "
            + _fill(w["at"], A="a", S="s")
            + " or "
            + _fill(w["at"], A="b", S="t")
            + ", but not both",
        ),
    ]
    if world.kinds:
        v3, v, noun = re.escape(world.verb3), re.escape(world.verb), re.escape(world.noun)
        kind = r"(?P<{}>\w+)"
        pats += [
            ("badge_is", rf"(?P<a>{_NAME}) {v3} the {kind.format('v')} {noun}"),
            (
                "badge_diff",
                rf"(?P<a>{_NAME}) and (?P<b>{_NAME}) do not {v} the same kind of {noun}",
            ),
            (
                "badge_order",
                rf"everyone who {v3} the {kind.format('v')} {noun} "
                + re.escape(w["group_before"])
                + rf" everyone who {v3} the {kind.format('w')} {noun}",
            ),
            (
                "holder_not",
                _fill(w["holder"], S="s") + rf" does not {v} the {kind.format('v')} {noun}",
            ),
            (
                "count_max",
                rf"at most (?P<k>\d+) of the \w+s {v} the {kind.format('v')} {noun}",
            ),
            (
                "cond_badge",
                rf"if (?P<a>{_NAME}) {v3} the {kind.format('v')} {noun}, then "
                + _fill(w["before"], A="b", B="c"),
            ),
        ]
    return [(k, re.compile(rf"^{p}\.$", re.I)) for k, p in pats]


def cp_parse(state: str) -> tuple[CPWorld, list[CPRule]]:
    world = cp_world(state)
    section = state.split("\nRules:\n", 1)
    if len(section) != 2:
        raise AuditError("constraint-pick: no rules section")
    lines = re.findall(r"^\d+\. (.*)$", section[1].split("\n\n", 1)[0], re.M)
    patterns = cp_rule_patterns(world)
    rules: list[CPRule] = []
    for line in lines:
        hits = [(k, m.groupdict()) for k, p in patterns if (m := p.match(line))]
        if len(hits) != 1:
            raise AuditError(f"constraint-pick: {len(hits)} readings of a rule")
        rules.append(hits[0])
    if not rules:
        raise AuditError("constraint-pick: no rules parsed")
    return world, rules


def cp_option(text: str, world: CPWorld) -> tuple[dict[str, int], dict[str, str]]:
    parts = text.split("; ")
    place: dict[str, int] = {}
    badge: dict[str, str] = {}
    for part in parts:
        m = re.fullmatch(rf"\w+ (\d+): ({_NAME})(?: \((\w+)\))?", part)
        if not m:
            raise AuditError("constraint-pick: option not recognised")
        place[m.group(2)] = int(m.group(1))
        if m.group(3):
            badge[m.group(2)] = m.group(3)
    if sorted(place) != sorted(world.names) or sorted(place.values()) != list(
        range(1, world.n + 1)
    ):
        raise AuditError("constraint-pick: option is not a full arrangement")
    if world.kinds and sorted(badge) != sorted(world.names):
        raise AuditError("constraint-pick: option misses a badge")
    return place, badge


def cp_holds(rule: CPRule, place: dict[str, int], badge: dict[str, str]) -> bool:  # noqa: PLR0911, PLR0912, one branch per rule kind
    kind, g = rule
    who = {s: e for e, s in place.items()}
    if kind == "before":
        return place[g["a"]] < place[g["b"]]
    if kind == "imm":
        return place[g["b"]] - place[g["a"]] == 1
    if kind in ("adj", "nadj"):
        return (abs(place[g["a"]] - place[g["b"]]) == 1) == (kind == "adj")
    if kind == "gap":
        return abs(place[g["a"]] - place[g["b"]]) - 1 == int(g["k"])
    if kind == "not_at":
        return place[g["a"]] != int(g["s"])
    if kind == "at_end":
        return place[g["a"]] in (1, int(g["n"]))
    if kind == "cond_even":
        return place[g["a"]] % 2 == 1 or place[g["b"]] < place[g["c"]]
    if kind == "xor_end":
        return (place[g["a"]] == int(g["s"])) != (place[g["b"]] == int(g["t"]))
    if kind == "badge_is":
        return badge[g["a"]] == g["v"]
    if kind == "badge_diff":
        return badge[g["a"]] != badge[g["b"]]
    if kind == "badge_order":
        early = [place[e] for e in place if badge[e] == g["v"]]
        late = [place[e] for e in place if badge[e] == g["w"]]
        return all(x < y for x in early for y in late)
    if kind == "holder_not":
        return badge[who[int(g["s"])]] != g["v"]
    if kind == "count_max":
        return sum(b == g["v"] for b in badge.values()) <= int(g["k"])
    if kind == "cond_badge":
        return badge[g["a"]] != g["v"] or place[g["b"]] < place[g["c"]]
    raise AuditError(f"constraint-pick: unknown rule kind {kind}")


def verify_constraint_pick(item: dict[str, Any], ctx: Context) -> Result:
    world, rules = cp_parse(item["state"])
    for _, g in rules:
        for key in ("a", "b", "c"):
            if key in g and g[key] not in world.names:
                raise AuditError("constraint-pick: a rule names an unknown entity")
        for key in ("v", "w"):
            if key in g and g[key] not in world.kinds:
                raise AuditError("constraint-pick: a rule names an unknown badge kind")
    valid = []
    for lab in labels(item):
        place, badge = cp_option(option_text(item, lab), world)
        if all(cp_holds(r, place, badge) for r in rules):
            valid.append(lab)
    if len(valid) != 1:
        return Result(
            item["item_id"],
            item["family"],
            MISMATCH,
            "brute force over the options",
            f"{len(valid)} options satisfy every rule",
        )
    return expect_label(item, valid[0], "brute force over the options", [])


# ---------------------------------------------------------------------------
# entailment: truth tables (propositional) and small-model enumeration (monadic)
# ---------------------------------------------------------------------------

Lit = tuple[str, bool]


def prop_literal(text: str) -> Lit:
    text = text.strip()
    m = re.fullmatch(r"[Tt]he (\w+) does not (\w+)", text)
    if m:
        return m.group(1), False
    m = re.fullmatch(r"[Tt]he (\w+) (\w+)s", text)
    if m:
        return m.group(1), True
    raise AuditError("entailment: literal not recognised")


PROP_FORMS: tuple[tuple[str, str], ...] = (
    ("if_or", r"If (.+), then (.+) or (.+) \(or both\)\."),
    ("if_and", r"If (.+) and (.+), then (.+)\."),
    ("if", r"If (.+), then (.+)\."),
    ("or", r"Either (.+) or (.+), or both\."),
    ("xor", r"Either (.+) or (.+), but not both\."),
    ("nand", r"It is not the case that both (.+) and (.+)\."),
    ("iff", r"(.+) if and only if (.+)\."),
    ("only_if", r"(.+) only if (.+)\."),
    ("unless", r"(.+) unless (.+)\."),
    ("fact", r"(.+)\."),
)

PropStmt = tuple[str, tuple[Lit, ...]]


def prop_parse(sentence: str) -> PropStmt:
    for style, pattern in PROP_FORMS:
        m = re.fullmatch(pattern, sentence.strip())
        if m:
            try:
                return style, tuple(prop_literal(g) for g in m.groups())
            except AuditError:
                continue
    raise AuditError("entailment: sentence not recognised")


def prop_value(stmt: PropStmt, val: dict[str, bool]) -> bool:  # noqa: PLR0911, one branch per wording
    style, lits = stmt
    v = [val[a] == pos for a, pos in lits]
    if style in ("if", "only_if"):
        return (not v[0]) or v[1]
    if style in ("or", "unless"):  # "P unless Q": if Q is false then P
        return v[0] or v[1]
    if style == "xor":
        return v[0] != v[1]
    if style == "nand":
        return not (v[0] and v[1])
    if style == "iff":
        return v[0] == v[1]
    if style == "if_and":
        return (not (v[0] and v[1])) or v[2]
    if style == "if_or":
        return (not v[0]) or v[1] or v[2]
    return v[0]


@dataclass(frozen=True)
class MonStmt:
    kind: str  # "all" or "some"
    conds: tuple[Lit, ...]
    concl: Lit | None


def mon_parse(sentence: str) -> MonStmt:
    s = sentence.strip()
    art = r"an? "
    if m := re.fullmatch(rf"Some (\w+) is (not )?{art}(\w+)\.", s):
        return MonStmt("some", ((m.group(1), True), (m.group(3), not m.group(2))), None)
    if m := re.fullmatch(rf"Something that is not {art}(\w+) is (not )?{art}(\w+)\.", s):
        return MonStmt("some", ((m.group(1), False), (m.group(3), not m.group(2))), None)
    if m := re.fullmatch(rf"(Every|No) (\w+) that is {art}(\w+) is {art}(\w+)\.", s):
        conds = ((m.group(2), True), (m.group(3), True))
        return MonStmt("all", conds, (m.group(4), m.group(1) == "Every"))
    if m := re.fullmatch(rf"(Every|No) (\w+) is {art}(\w+)\.", s):
        return MonStmt("all", ((m.group(2), True),), (m.group(3), m.group(1) == "Every"))
    if m := re.fullmatch(rf"(Everything|Nothing) that is not {art}(\w+) is {art}(\w+)\.", s):
        return MonStmt("all", ((m.group(2), False),), (m.group(3), m.group(1) == "Everything"))
    raise AuditError("entailment: monadic sentence not recognised")


def mon_holds(stmt: MonStmt, model: Sequence[dict[str, bool]]) -> bool:
    def meets(lits: Iterable[Lit], thing: dict[str, bool]) -> bool:
        return all(thing[p] == pos for p, pos in lits)

    if stmt.kind == "all":
        assert stmt.concl is not None
        return all(meets([stmt.concl], t) for t in model if meets(stmt.conds, t))
    return any(meets(stmt.conds, t) for t in model)


def entailment_parse(state: str) -> tuple[str, list[str], str]:
    m = re.search(r"\nPremises:\n(.*?)\n\nConclusion: (.*)$", state, re.S)
    if not m:
        raise AuditError("entailment: premises or conclusion missing")
    premises = re.findall(r"^\d+\. (.*)$", m.group(1), re.M)
    style = "mon" if state.startswith("The statements below are about the things") else "prop"
    return style, premises, m.group(2).strip()


def entailment_follows(state: str) -> tuple[bool, list[str]]:
    style, premise_texts, concl_text = entailment_parse(state)
    notes: list[str] = []
    if style == "prop":
        premises = [prop_parse(p) for p in premise_texts]
        concl = prop_parse(concl_text)
        atoms = sorted({a for _, lits in [*premises, concl] for a, _ in lits})
        models = []
        for values in itertools.product((False, True), repeat=len(atoms)):
            val = dict(zip(atoms, values, strict=True))
            if all(prop_value(p, val) for p in premises):
                models.append(val)
        if not models:
            notes.append("premises are inconsistent (vacuous entailment)")
        return all(prop_value(concl, v) for v in models), notes
    mpremises = [mon_parse(p) for p in premise_texts]
    mconcl = mon_parse(concl_text)
    preds = sorted(
        {p for s in [*mpremises, mconcl] for p, _ in [*s.conds, *([s.concl] if s.concl else [])]}
    )
    kinds = [
        dict(zip(preds, bits, strict=True))
        for bits in itertools.product((False, True), repeat=len(preds))
    ]
    witnesses = sum(s.kind == "some" for s in mpremises)
    # A countermodel needs a witness per existential premise plus one element.
    largest = witnesses + 1
    if len(kinds) > 64 or largest > 4:
        raise AuditError("entailment: model space too large to enumerate")
    satisfied = 0
    for size in range(1, largest + 1):
        for model in itertools.combinations(kinds, size):
            if all(mon_holds(p, model) for p in mpremises):
                satisfied += 1
                if not mon_holds(mconcl, model):
                    return False, notes
    if not satisfied:
        notes.append("premises have no model (vacuous entailment)")
    return True, notes


def verify_entailment(item: dict[str, Any], ctx: Context) -> Result:
    follows, notes = entailment_follows(item["state"])
    method = (
        "truth table" if not item["state"].startswith("The statements") else "model enumeration"
    )
    return expect_label(item, "true" if follows else "false", method, notes)


# ---------------------------------------------------------------------------
# word-problem: recompute from the numbers in the text
# ---------------------------------------------------------------------------


def _money(value: Fraction) -> str:
    if (value * 100).denominator != 1:
        raise AuditError("word-problem: money value is not a whole number of cents")
    return f"{float(value):.2f}"


def _whole(value: Fraction) -> str:
    if value.denominator != 1:
        raise AuditError("word-problem: count value is not whole")
    return str(value.numerator)


def word_problem_value(text: str) -> str:  # noqa: PLR0911, one branch per template
    F = Fraction
    n = r"(\d+)"
    if m := re.search(
        rf"buys {n} \w+ at {n} \w+ each and {n} \w+ at {n} \w+ each\. The shop takes {n}% off "
        rf"the whole order, and then adds a delivery charge of {n} ",
        text,
    ):
        q1, p1, q2, p2, d, fee = map(int, m.groups())
        return _money(F(q1 * p1 + q2 * p2) * F(100 - d, 100) + fee)
    if m := re.search(
        rf"travels {n} km at {n} km/h, stops for {n} minutes, and then travels {n} km at {n} km/h",
        text,
    ):
        d1, v1, stop, d2, v2 = map(int, m.groups())
        return _whole(F(60 * d1, v1) + stop + F(60 * d2, v2))
    if m := re.search(
        rf"makes {n} parts per hour and machine \w+ makes {n} parts per hour\. \w+ runs alone for "
        rf"{n} hours, and then both machines run together for {n} more hours\. Inspection "
        rf"rejects {n}% of all the parts made",
        text,
    ):
        a, b, h1, h2, p = map(int, m.groups())
        return _whole(F(a * h1 + (a + b) * h2) * F(100 - p, 100))
    if m := re.search(
        rf"straight path is {n} m long\. Posts are placed every {n} m along one side, with a post "
        rf"at both ends, and the same is done along the other side\. Each post costs {n} ",
        text,
    ):
        length, step, cost = map(int, m.groups())
        if length % step:
            raise AuditError("word-problem: posts do not divide the path")
        return _whole(F(2 * (length // step + 1) * cost))
    if m := re.search(
        rf"field measures {n} m by {n} m\. Fence posts are placed every {n} m around its whole "
        rf"edge, with a post at each corner\. Each post costs {n} ",
        text,
    ):
        width, height, step, cost = map(int, m.groups())
        if (2 * (width + height)) % step or width % step or height % step:
            raise AuditError("word-problem: posts do not divide the edge")
        return _whole(F(2 * (width + height) // step * cost))
    if m := re.search(
        rf"holds {n} litres of a solution that is {n}% \w+\. {n} litres of pure water are stirred "
        rf"in\. Then {n} litres of the mixture are drained off, and finally {n} litres of pure",
        text,
    ):
        x, c1, y, z, top = map(int, m.groups())
        chem, volume = F(x * c1, 100), F(x + y)
        return _money(chem - chem * F(z) / volume + top)
    if m := re.search(
        rf"saves {n} \w+ in week 1, and each week after that saves {n} \w+ more than the week "
        rf"before\. At the end of week {n}, \w+ spends {n} ",
        text,
    ):
        a, d, weeks, spend = map(int, m.groups())
        return _whole(F(sum(a + d * (w - 1) for w in range(1, weeks + 1)) - spend))
    if m := re.search(
        rf"costs {n} \w+\. Its price rises by {n}%, and later the new price falls by {n}%\. A tax "
        rf"of {n}% is then added",
        text,
    ):
        price, up, down, tax = map(int, m.groups())
        return _money(F(price) * F(100 + up, 100) * F(100 - down, 100) * F(100 + tax, 100))
    raise AuditError("word-problem: problem text not recognised")


def verify_word_problem(item: dict[str, Any], ctx: Context) -> Result:
    value = word_problem_value(item["state"])
    return expect_label(item, unique_match(item, value), "recomputed from the stated numbers", [])


# ---------------------------------------------------------------------------
# estimate-band: recompute the quantity, read the band edges from the levels
# ---------------------------------------------------------------------------


def estimate_value(state: str) -> float:  # noqa: PLR0911, one branch per template
    text, _, quantity = state.partition("\n\nQuantity: ")
    num = r"(\d+(?:\.\d+)?)"
    if m := re.search(
        rf"starts with {num} individuals\. Each cycle the colony grows by {num}% of its size at "
        rf"the start of that cycle\. After {num} cycles, {num} individuals are moved",
        text,
    ):
        p0, r, n, moved = (float(g) for g in m.groups())
        size = p0
        for _ in range(int(n)):
            size += size * r / 100
        return size - moved
    if m := re.search(
        rf"holds {num} litres when full and starts with {num} litres\. A pipe fills it at {num} "
        rf"litres per minute while a crack leaks {num} litres per minute\. After {num} minutes a "
        rf"second pipe adding {num} litres per minute is opened",
        text,
    ):
        cap, start, inflow, leak, t1, extra = (float(g) for g in m.groups())
        level_at_t1 = start + (inflow - leak) * t1
        if level_at_t1 >= cap:
            return (cap - start) / (inflow - leak)
        return t1 + (cap - level_at_t1) / (inflow + extra - leak)
    if m := re.search(
        rf"blended: {num} kg at {num}% \w+, {num} kg at {num}% \w+, and {num} kg at {num}% ", text
    ):
        v = [float(g) for g in m.groups()]
        masses, conc = v[0::2], v[1::2]
        return sum(a * b for a, b in zip(masses, conc, strict=True)) / sum(masses)
    if m := re.search(
        rf"loses half of its mass every {num} hours\. {num} mg is placed in a sealed chamber at "
        rf"time zero, and another {num} mg is added {num} hours later",
        text,
    ):
        h, a0, a1, t1 = (float(g) for g in m.groups())
        t2m = re.search(rf"chamber {num} hours after time zero", quantity)
        if not t2m:
            raise AuditError("estimate-band: decay time not found")
        t2 = float(t2m.group(1))
        later = a1 * 0.5 ** ((t2 - t1) / h) if t2 >= t1 else 0.0
        return float(a0 * 0.5 ** (t2 / h) + later)
    if m := re.search(
        rf"covers three legs of a route: {num} km at {num} km/h, {num} km at {num} km/h, then "
        rf"{num} km at {num} km/h",
        text,
    ):
        v = [float(g) for g in m.groups()]
        dist, speed = v[0::2], v[1::2]
        return sum(dist) / sum(d / s for d, s in zip(dist, speed, strict=True))
    if m := re.search(
        rf"starts with {num} \w+\. At the end of each month it first earns {num}% interest on its "
        rf"balance, and then a deposit of {num} \w+ is made\. This happens for {num} months",
        text,
    ):
        balance, r, deposit, months = (float(g) for g in m.groups())
        for _ in range(int(months)):
            balance = balance + balance * r / 100 + deposit
        return balance
    if m := re.search(
        rf"can finish a job in {num} hours, \w+ in {num} hours and \w+ in {num} hours\. \w+ and "
        rf"\w+ start together; after {num} hours \w+ joins them",
        text,
    ):
        a, b, c, t = (float(g) for g in m.groups())
        pair = 1 / a + 1 / b
        if t * pair >= 1:
            return 60 / pair
        return 60 * (t + (1 - t * pair) / (pair + 1 / c))
    raise AuditError("estimate-band: problem text not recognised")


def band_bounds(level: str) -> tuple[float | None, float | None]:
    nums = [float(x) for x in re.findall(r"\d+(?:\.\d+)?", level)]
    if level.startswith(("under ", "fewer than ")):
        return None, nums[0]
    if level.endswith(" or more"):
        return nums[0], None
    if len(nums) != 2 or " to " not in level:
        raise AuditError("band level not recognised")
    return nums[0], nums[1]


def verify_estimate_band(item: dict[str, Any], ctx: Context) -> Result:
    value = estimate_value(item["state"])
    levels = labels(item)
    hits = []
    notes = []
    for level in levels:
        lo, hi = band_bounds(level)
        if (lo is None or value >= lo) and (hi is None or value < hi):
            hits.append(level)
        for edge in (lo, hi):
            if edge is not None and abs(value - edge) <= 1e-6 * max(1.0, abs(edge)):
                notes.append("value sits on a band edge")
    if len(hits) != 1:
        raise AuditError(f"estimate-band: value falls in {len(hits)} bands")
    return expect_label(item, hits[0], "recomputed from the stated numbers", notes)


# ---------------------------------------------------------------------------
# Tables: parse the rendered table and recompute
# ---------------------------------------------------------------------------

COND = (
    r"(?P<col>\w+) is (?:between (?P<lo>\d+) and (?P<hi>\d+) inclusive|greater than (?P<gt>\d+)"
    r"|at most (?P<le>\d+)|not (?P<ne>\w+)|(?P<eq>\w+))"
)
COND_ANY = r"\w+ is (?:between \d+ and \d+ inclusive|greater than \d+|at most \d+|not \w+|\w+)"


def parse_table(state: str) -> list[dict[str, Any]]:
    head, sep, body = state.partition("\n\n")
    if not sep:
        raise AuditError("table: no table body")
    m = re.search(r"\((\d+) rows, (CSV|Markdown table)\)", head)
    lines = body.strip().splitlines()
    if lines[0].startswith("|"):
        rows = [[c.strip() for c in line.strip().strip("|").split("|")] for line in lines]
        rows = [r for r in rows if not all(set(c) <= {"-", ":"} for c in r)]
    else:
        rows = [line.split(",") for line in lines]
    header, data = rows[0], rows[1:]
    out = []
    for row in data:
        if len(row) != len(header):
            raise AuditError("table: ragged row")
        out.append(
            {
                h: int(v) if re.fullmatch(r"-?\d+", v) else v
                for h, v in zip(header, row, strict=True)
            }
        )
    if m and int(m.group(1)) != len(out):
        raise AuditError("table: row count differs from the header")
    return out


def cond_holds(cond: dict[str, str | None], row: dict[str, Any]) -> bool:
    col = cond["col"]
    if col not in row:
        raise AuditError(f"table: unknown column {col}")
    v = row[col]
    if cond["lo"] is not None:
        return bool(int(cond["lo"]) <= v <= int(cond["hi"] or 0))
    if cond["gt"] is not None:
        return bool(v > int(cond["gt"]))
    if cond["le"] is not None:
        return bool(v <= int(cond["le"]))
    if cond["ne"] is not None:
        return bool(v != cond["ne"])
    return bool(v == cond["eq"])


def parse_conds(text: str) -> list[dict[str, str | None]]:
    return [m.groupdict() for m in re.finditer(COND, text)]


def verify_table_lookup(item: dict[str, Any], ctx: Context) -> Result:
    rows = parse_table(item["state"])
    q = item["question"]["instructions"]
    m = re.fullmatch(
        r"Considering only rows where (?P<where>.+), which (?P<g>\w+) has the "
        r"(?P<what>largest total|smallest total|highest average|lowest average|most matching rows"
        r"|widest spread of)(?: (?P<num>\w+))?(?: \(largest value minus smallest value\))?\?",
        q,
    )
    if not m:
        raise AuditError("table-lookup: question not recognised")
    where = m.group("where")
    if not re.fullmatch(rf"{COND_ANY}(?: and {COND_ANY})*", where):
        raise AuditError("table-lookup: filter not recognised")
    conds = parse_conds(where)
    gcol, what, num = m.group("g"), m.group("what"), m.group("num")
    groups: dict[str, list[int]] = {g: [] for g in labels(item)}
    for row in rows:
        if all(cond_holds(c, row) for c in conds) and row.get(gcol) in groups:
            groups[row[gcol]].append(int(row[num]) if num else 1)
    notes = []
    scores: dict[str, float] = {}
    for g, vals in groups.items():
        if what.endswith("total"):
            scores[g] = float(sum(vals))
        elif what == "most matching rows":
            scores[g] = float(len(vals))
        elif not vals:
            notes.append(f"option {g} has no matching rows; left out")
        elif what.endswith("average"):
            scores[g] = sum(vals) / len(vals)
        else:
            scores[g] = float(max(vals) - min(vals))
    want_max = what in (
        "largest total",
        "highest average",
        "most matching rows",
        "widest spread of",
    )
    ordered = sorted(scores.items(), key=lambda kv: kv[1], reverse=want_max)
    if len(ordered) < 2:
        raise AuditError("table-lookup: fewer than two groups to compare")
    if ordered[0][1] == ordered[1][1]:
        return Result(
            item["item_id"],
            item["family"],
            MISMATCH,
            "table recomputed",
            "tie for the answer",
            notes,
        )
    return expect_label(item, ordered[0][0], "table recomputed", notes)


def count_structure(q: str) -> tuple[str, list[str]]:
    c = f"({COND_ANY})"
    forms = (
        ("or_and", rf"How many rows are there where {c}, and also either {c} or {c}\?"),
        ("or2", rf"How many rows are there where {c} or {c} \(or both\)\?"),
        ("and3", rf"How many rows are there where {c}, {c} and {c}\?"),
        ("and2", rf"How many rows are there where {c} and {c}\?"),
    )
    for shape, pattern in forms:
        m = re.fullmatch(pattern, q)
        if m:
            return shape, list(m.groups())
    raise AuditError("table-count-band: question not recognised")


def verify_table_count_band(item: dict[str, Any], ctx: Context) -> Result:
    rows = parse_table(item["state"])
    shape, parts = count_structure(item["question"]["instructions"])
    conds = [parse_conds(p)[0] for p in parts]

    def match(row: dict[str, Any]) -> bool:
        h = [cond_holds(c, row) for c in conds]
        if shape == "or_and":
            return h[0] and (h[1] or h[2])
        if shape == "or2":
            return h[0] or h[1]
        return all(h)

    count = sum(match(r) for r in rows)
    hits = []
    for level in labels(item):
        lo, hi = band_bounds(level)
        inside_hi = hi is None or (count < hi if level.startswith("fewer") else count <= hi)
        if (lo is None or count >= lo) and inside_hi:
            hits.append(level)
    if len(hits) != 1:
        raise AuditError(f"table-count-band: count falls in {len(hits)} bands")
    return expect_label(item, hits[0], "table recomputed", [])


# ---------------------------------------------------------------------------
# Policies: parse clauses and the case, apply the stated precedence
# ---------------------------------------------------------------------------

# Per policy frame: the case label and the wording of each condition kind.
POLICY_FRAMES: dict[str, dict[str, Any]] = {
    "Instrument loan policy": {
        "c1": ("Requester role", "the requester is"),
        "c2": ("Destination", "the destination is"),
        "n1": ("Instruments requested", r"more than (\d+) instruments are requested",
               r"at most (\d+) instruments are requested"),
        "n2": ("Loan length in days", r"the loan is for more than (\d+) days",
               r"the loan is for (\d+) days or fewer"),
        "flag": ("Sponsor on file", "a sponsor is on file", "no sponsor is on file"),
    },
    "Reading room access policy": {
        "c1": ("Applicant rank", "the applicant is"),
        "c2": ("Vault section", "the section is"),
        "n1": ("Items to view", r"more than (\d+) items are requested",
               r"at most (\d+) items are requested"),
        "n2": ("Prior visits", r"the applicant has made more than (\d+) prior visits",
               r"the applicant has made (\d+) or fewer prior visits"),
        "flag": ("Escort booked", "an escort is booked", "no escort is booked"),
    },
    "Expense claim policy": {
        "c1": ("Claimant grade", "the claimant is"),
        "c2": ("Expense type", "the expense type is"),
        "n1": ("Amount in marks", r"the amount is more than (\d+) marks",
               r"the amount is at most (\d+) marks"),
        "n2": ("Days since purchase", r"the claim is filed more than (\d+) days after purchase",
               r"the claim is filed within (\d+) days of purchase"),
        "flag": ("Receipt attached", "a receipt is attached", "no receipt is attached"),
    },
    "Berth policy": {
        "c1": ("Vessel class", "the vessel is"),
        "c2": ("Requested quay", "the quay requested is"),
        "n1": ("Length in metres", r"the vessel is longer than (\d+) metres",
               r"the vessel is (\d+) metres long or shorter"),
        "n2": ("Nights requested", r"more than (\d+) nights are requested",
               r"at most (\d+) nights are requested"),
        "flag": ("Hazard cargo declared", "hazard cargo is declared",
                 "no hazard cargo is declared"),
    },
}  # fmt: skip


@dataclass(frozen=True)
class PolicyClause:
    number: int
    conds: tuple[str, ...]
    unless: str | None
    approve: bool
    default: bool
    beats: frozenset[int]


def policy_atom(text: str, frame: dict[str, Any], case: dict[str, Any]) -> bool:
    text = text.strip()
    c1 = re.escape(frame["c1"][1])
    if m := re.fullmatch(rf"{c1} an? (\w+)(?: or an? (\w+))?", text):
        return case["c1"] in {m.group(1), m.group(2)}
    c2 = re.escape(frame["c2"][1])
    if m := re.fullmatch(rf"{c2} (not )?([A-Z]\w+)", text):
        return bool(case["c2"] == m.group(2)) != bool(m.group(1))
    for key in ("n1", "n2"):
        if m := re.fullmatch(frame[key][1], text):
            return bool(case[key] > int(m.group(1)))
        if m := re.fullmatch(frame[key][2], text):
            return bool(case[key] <= int(m.group(1)))
    if text == frame["flag"][1]:
        return bool(case["flag"])
    if text == frame["flag"][2]:
        return not case["flag"]
    raise AuditError("policy: condition not recognised")


POLICY_FORMS = (
    (r"A request is (approved|refused) if (.+?)(?:, unless (.+))?\.", (1, 2, 3)),
    (r"If (.+?), the request is (approved|refused)(?:, unless (.+))?\.", (2, 1, 3)),
    (r"(Approve|Refuse) any request where (.+?)(?:, unless (.+))?\.", (1, 2, 3)),
)


def policy_clause(number: int, text: str) -> PolicyClause:
    beats: frozenset[int] = frozenset()
    body = text
    pm = re.search(r" This clause takes precedence over clauses? ([\d, and]+)\.$", text)
    if pm:
        beats = frozenset(int(x) for x in re.findall(r"\d+", pm.group(1)))
        body = text[: pm.start()]
    if dm := re.fullmatch(r"If no other clause applies, the request is (approved|refused)\.", body):
        return PolicyClause(number, (), None, dm.group(1) == "approved", True, beats)
    for pattern, (iv, ic, iu) in POLICY_FORMS:
        m = re.fullmatch(pattern, body)
        if m:
            verdict = m.group(iv).lower() in ("approved", "approve")
            conds = tuple(m.group(ic).split(" and "))
            return PolicyClause(number, conds, m.group(iu), verdict, False, beats)
    raise AuditError("policy: clause not recognised")


def policy_parse(state: str) -> tuple[dict[str, Any], str, list[PolicyClause], dict[str, Any]]:
    title = state.splitlines()[0]
    frame = next((f for key, f in POLICY_FRAMES.items() if title.startswith(key)), None)
    if frame is None:
        raise AuditError("policy: unknown policy title")
    rm = re.search(r"the applicable clause with the (lowest|highest) number decides it", state)
    if not rm:
        raise AuditError("policy: numbering rule not found")
    shown = dict(re.findall(r"^- ([^:\n]+): (.*)$", state, re.M))
    case: dict[str, Any] = {}
    for key in ("c1", "c2", "n1", "n2", "flag"):
        label = frame[key][0]
        if label not in shown:
            raise AuditError(f"policy: case field {label!r} missing")
        raw = shown[label].strip()
        case[key] = int(raw) if key in ("n1", "n2") else (raw == "yes") if key == "flag" else raw
    clauses = []
    for number, text in re.findall(r"^Clause (\d+)\. (.*)$", state, re.M):
        clauses.append(policy_clause(int(number), text))
    if [c.number for c in clauses] != list(range(1, len(clauses) + 1)):
        raise AuditError("policy: clause numbering has gaps")
    return frame, rm.group(1), clauses, case


def policy_winner(state: str) -> PolicyClause:
    frame, which, clauses, case = policy_parse(state)

    def applies(c: PolicyClause) -> bool:
        if c.default:
            return False
        if not all(policy_atom(a, frame, case) for a in c.conds):
            return False
        return not (c.unless is not None and policy_atom(c.unless, frame, case))

    live = [c for c in clauses if applies(c)]
    if not live:
        defaults = [c for c in clauses if c.default]
        if len(defaults) != 1:
            raise AuditError("policy: no clause applies and there is no single default")
        return defaults[0]

    def beats(x: PolicyClause, y: PolicyClause) -> bool:
        if y.number in x.beats and x.number in y.beats:
            raise AuditError("policy: two clauses each take precedence over the other")
        if y.number in x.beats:
            return True
        if x.number in y.beats:
            return False
        return x.number < y.number if which == "lowest" else x.number > y.number

    winners = [x for x in live if all(beats(x, y) for y in live if y is not x)]
    if len(winners) != 1:
        raise AuditError("policy: the precedence rules name no single winner (cycle)")
    return winners[0]


def verify_policy_decision(item: dict[str, Any], ctx: Context) -> Result:
    winner = policy_winner(item["state"])
    return expect_label(item, "true" if winner.approve else "false", "precedence engine", [])


def verify_policy_clause(item: dict[str, Any], ctx: Context) -> Result:
    winner = policy_winner(item["state"])
    return expect_label(item, str(winner.number), "precedence engine", [])


# ---------------------------------------------------------------------------
# code-output: run the program fresh
# ---------------------------------------------------------------------------

ALT_NODE = sorted(Path.home().glob(".nvm/versions/node/v24*/bin/node"))


def code_output_program(state: str) -> tuple[str, str]:
    m = re.search(r"^Language: (Python 3 \(CPython\)|JavaScript \(Node\.js\))$", state, re.M)
    if not m:
        raise AuditError("code-output: language line not recognised")
    lang = "python" if m.group(1).startswith("Python") else "javascript"
    blocks = [body for info, body in fenced_blocks(state) if info == lang]
    if len(blocks) != 1:
        raise AuditError("code-output: expected exactly one program block")
    return lang, blocks[0]


def run_program(lang: str, source: str, hash_seed: str = "0", node: str = "node") -> RunOut:
    if lang == "python":
        return run_sandboxed(
            {"main.py": source}, ["python", "main.py"], timeout=10.0, hash_seed=hash_seed
        )
    return run_sandboxed({"main.js": source}, [node, "main.js"], timeout=10.0)


def verify_code_output(item: dict[str, Any], ctx: Context) -> Result:
    lang, source = code_output_program(item["state"])
    first = run_program(lang, source)
    method = (
        f"executed fresh ({'python ' + sys.version.split()[0] if lang == 'python' else 'node'})"
    )
    if not first.ok:
        return Result(
            item["item_id"],
            item["family"],
            MISMATCH,
            method,
            f"program exited {first.returncode}{' (timeout)' if first.timed_out else ''}",
        )
    truth = first.stdout.strip("\n")
    notes = []
    if lang == "python":
        again = run_program(lang, source, hash_seed="4242")
        if again.stdout.strip("\n") != truth:
            notes.append("output depends on PYTHONHASHSEED (builder ran with 0)")
    else:
        for node in ALT_NODE:
            other = run_program(lang, source, node=str(node))
            if other.ok and other.stdout.strip("\n") != truth:
                notes.append(f"output differs under {node.parts[-3]}")
    hits = [lab for lab in labels(item) if option_text(item, lab) == truth]
    if len(hits) != 1:
        return Result(
            item["item_id"],
            item["family"],
            MISMATCH,
            method,
            f"{len(hits)} options equal the program's output",
            notes,
        )
    return expect_label(item, hits[0], method, notes)


# ---------------------------------------------------------------------------
# type-check-pair: mypy on each snippet, alone, with the flags the state names
# ---------------------------------------------------------------------------

_MYPY_VERSION: list[str] = []
_MYPY_LOCK = threading.Lock()


def mypy_version() -> str:
    with _MYPY_LOCK:
        if not _MYPY_VERSION:
            proc = subprocess.run(
                [sys.executable, "-m", "mypy", "--version"],
                capture_output=True,
                text=True,
                check=False,
            )
            _MYPY_VERSION.append(" ".join(proc.stdout.split()[:2]))
        return _MYPY_VERSION[0]


def type_check_parse(state: str) -> tuple[list[str], str, dict[str, str]]:
    m = re.search(r"passes `mypy ([^`]*)` \((mypy [\d.]+), ([^)]*)\)", state)
    if not m:
        raise AuditError("type-check-pair: checker and flags not stated")
    flags = m.group(1).split() + m.group(3).split()
    snippets = dict(re.findall(r"^Snippet ([AB]):\n```python\n(.*?)^```", state, re.S | re.M))
    if sorted(snippets) != ["A", "B"]:
        raise AuditError("type-check-pair: two snippets not found")
    return flags, m.group(2), snippets


def mypy_passes(snippet: str, flags: Sequence[str]) -> tuple[bool, list[str]]:
    out = run_sandboxed(
        {"snippet.py": snippet, "mypy.ini": "[mypy]\n"},
        [
            "python-raw",
            "-m",
            "mypy",
            *flags,
            "--config-file",
            "mypy.ini",
            "--cache-dir",
            ".mypy_cache",
            "snippet.py",
        ],
        timeout=300.0,
    )
    errors = [ln for ln in out.stdout.splitlines() if ": error:" in ln]
    if out.timed_out or out.returncode not in (0, 1):
        raise AuditError(f"type-check-pair: mypy crashed (exit {out.returncode})")
    if (out.returncode == 0) != (not errors):
        raise AuditError("type-check-pair: mypy exit code and error lines disagree")
    return out.returncode == 0, errors


def verify_type_check_pair(item: dict[str, Any], ctx: Context) -> Result:
    flags, named, snippets = type_check_parse(item["state"])
    installed = mypy_version()
    if installed != named:
        raise AuditError(f"type-check-pair: state names {named}, installed is {installed}")
    passing = [lab for lab in ("A", "B") if mypy_passes(snippets[lab], flags)[0]]
    method = f"{installed} {' '.join(flags)}, each snippet alone"
    if len(passing) != 1:
        return Result(
            item["item_id"], item["family"], MISMATCH, method, f"{len(passing)} snippets pass"
        )
    return expect_label(item, passing[0], method, [])


# ---------------------------------------------------------------------------
# expected-value: implementations written here from each docstring
# ---------------------------------------------------------------------------


def _ev_snake_case(name: str) -> str:
    words = re.findall(r"[A-Z]+(?=[A-Z][a-z])|[A-Z]?[a-z0-9]+|[A-Z]+[0-9]*", name)
    return "_".join(w.lower() for w in words)


def _ev_binary_gap(n: int) -> int:
    best = run = 0
    seen_one = False
    while n:
        if n & 1:
            if seen_one:
                best = max(best, run)
            seen_one, run = True, 0
        else:
            run += 1
        n >>= 1
    return best


def _ev_roman(numeral: str) -> int:
    value = {"I": 1, "V": 5, "X": 10, "L": 50, "C": 100, "D": 500, "M": 1000}
    total = 0
    for i, ch in enumerate(numeral):
        v = value[ch]
        total += -v if i + 1 < len(numeral) and value[numeral[i + 1]] > v else v
    return total


def _ev_progressive_tax(income: int, brackets: list[tuple[int | None, float]]) -> float:
    tax, lower = 0.0, 0
    for upper, rate in brackets:
        top = income if upper is None else min(income, upper)
        if top > lower:
            tax += (top - lower) * rate
        if upper is None:
            break
        lower = upper
    return round(tax, 2)


def _ev_merge(intervals: list[list[int]]) -> list[list[int]]:
    out: list[list[int]] = []
    for s, e in sorted(intervals, key=lambda iv: (iv[0], iv[1])):
        if out and s <= out[-1][1]:
            out[-1][1] = max(out[-1][1], e)
        else:
            out.append([s, e])
    return out


def _ev_caesar(text: str, shift: int) -> str:
    out = []
    for ch in text:
        if "a" <= ch <= "z":
            out.append(chr((ord(ch) - 97 + shift) % 26 + 97))
        elif "A" <= ch <= "Z":
            out.append(chr((ord(ch) - 65 + shift) % 26 + 65))
        else:
            out.append(ch)
    return "".join(out)


def _ev_median(values: list[float]) -> float:
    s = sorted(values)
    n = len(s)
    return s[n // 2] if n % 2 else (s[n // 2 - 1] + s[n // 2]) / 2


def _ev_round_half_up(x: float, digits: int) -> float:
    return float(Decimal(repr(x)).quantize(Decimal(1).scaleb(-digits), rounding=ROUND_HALF_UP))


def _ev_weekdays(start: str, end: str) -> int:
    d, e = date.fromisoformat(start), date.fromisoformat(end)
    return sum((d + timedelta(days=i)).weekday() < 5 for i in range((e - d).days))


def _ev_leap(first: int, last: int) -> int:
    return sum(calendar.isleap(y) for y in range(first, last + 1))


def _ev_top_words(text: str, k: int) -> list[str]:
    counts = Counter(w.lower() for w in text.split(" ") if w)
    return [w for w, _ in sorted(counts.items(), key=lambda p: (-p[1], p[0]))][:k]


def _ev_page_summary(total: int, per_page: int) -> tuple[int, int]:
    if total == 0:
        return (0, 0)
    pages = (total + per_page - 1) // per_page
    return (pages, total - (pages - 1) * per_page)


def _ev_max_depth(text: str) -> int:
    depth = best = 0
    for ch in text:
        depth += (ch == "(") - (ch == ")")
        best = max(best, depth)
    return best


def _ev_full_years(born: str, on: str) -> int:
    b, d = date.fromisoformat(born), date.fromisoformat(on)
    return d.year - b.year - (1 if (d.month, d.day) < (b.month, b.day) else 0)


def _ev_histogram(values: list[int], width: int) -> list[int]:
    bins = [0] * (max(values) // width + 1)
    for v in values:
        bins[v // width] += 1
    return bins


def _ev_primes_below(n: int) -> int:
    return sum(1 for k in range(2, n) if all(k % d for d in range(2, k)))


def _ev_run_lengths(text: str) -> list[tuple[str, int]]:
    return [(ch, len(list(group))) for ch, group in itertools.groupby(text)]


EV_IMPLS: dict[str, Callable[..., Any]] = {
    "count_multiples": lambda lo, hi, k: sum(1 for n in range(lo, hi) if n % k == 0),
    "median": _ev_median,
    "round_half_up": _ev_round_half_up,
    "chunk": lambda items, size: [items[i : i + size] for i in range(0, len(items), size)],
    "rotate_right": lambda items, k: (
        (items[len(items) - k % len(items) :] + items[: len(items) - k % len(items)])
        if items
        else []
    ),
    "run_lengths": _ev_run_lengths,
    "moving_average": lambda values, window: [
        round(sum(values[i : i + window]) / window, 2) for i in range(len(values) - window + 1)
    ],
    "dedupe": lambda items: [x for i, x in enumerate(items) if x not in items[:i]],
    "weekdays_between": _ev_weekdays,
    "count_leap_years": _ev_leap,
    "insert_position": lambda values, x: sum(1 for v in values if v <= x),
    "merge_intervals": _ev_merge,
    "roman_to_int": _ev_roman,
    "digital_root": lambda n: 0 if n == 0 else 1 + (n - 1) % 9,
    "count_primes_below": _ev_primes_below,
    "progressive_tax": _ev_progressive_tax,
    "caesar": _ev_caesar,
    "top_words": _ev_top_words,
    "sum_multiples_3_or_5": lambda n: sum(k for k in range(1, n) if k % 3 == 0 or k % 5 == 0),
    "format_duration": lambda s: f"{s // 3600}:{s % 3600 // 60:02d}:{s % 60:02d}",
    "page_summary": _ev_page_summary,
    "max_depth": _ev_max_depth,
    "binary_gap": _ev_binary_gap,
    "weighted_mean": lambda values, weights: round(
        sum(v * w for v, w in zip(values, weights, strict=True)) / sum(weights), 3
    ),
    "full_years": _ev_full_years,
    "histogram": _ev_histogram,
    "snake_case": _ev_snake_case,
    "percent_change": lambda old, new: round((new - old) / old * 100, 1),
}


def expected_value_parse(state: str) -> tuple[str, list[Any], Any, str]:
    blocks = [body for info, body in fenced_blocks(state) if info == "python"]
    if len(blocks) != 1:
        raise AuditError("expected-value: one code block expected")
    asserts = [ln for ln in blocks[0].splitlines() if ln.startswith("assert ")]
    if len(asserts) != 1:
        raise AuditError("expected-value: one assertion expected")
    node = ast.parse(asserts[0]).body[0]
    if not (
        isinstance(node, ast.Assert)
        and isinstance(node.test, ast.Compare)
        and len(node.test.ops) == 1
        and isinstance(node.test.ops[0], ast.Eq)
        and isinstance(node.test.left, ast.Call)
        and isinstance(node.test.left.func, ast.Name)
    ):
        raise AuditError("expected-value: assertion shape not recognised")
    call = node.test.left
    assert isinstance(call.func, ast.Name)
    args = [ast.literal_eval(a) for a in call.args]
    expected_node = node.test.comparators[0]
    return call.func.id, args, ast.literal_eval(expected_node), ast.unparse(expected_node)


REFERENCE_RUNNER = """\
import copy, json, sys
source, name, args = json.load(open("job.json"))
namespace = {}
exec(source, namespace)
try:
    print(repr(namespace[name](*copy.deepcopy(args))))
except Exception as error:
    print("!" + type(error).__name__)
"""


def run_reference(name: str, args: list[Any]) -> str | None:
    """The builder's hidden reference implementation, re-run in the sandbox."""
    spec = next((s for s in SPECS if s.name == name), None)
    if spec is None:
        return None
    out = run_sandboxed(
        {"run.py": REFERENCE_RUNNER, "job.json": json.dumps([spec.reference, name, args])},
        ["python", "run.py"],
        timeout=20.0,
    )
    return out.stdout.strip() if out.ok else None


def verify_expected_value(item: dict[str, Any], ctx: Context) -> Result:
    name, args, expected, expected_text = expected_value_parse(item["state"])
    impl = EV_IMPLS.get(name)
    if impl is None:
        raise AuditError(f"expected-value: no independent implementation of {name}")
    mine = impl(*copy.deepcopy(args))
    equal = bool(mine == expected)
    exact = repr(mine) == repr(expected)
    notes = []
    # JSON turns tuples into lists; the references only iterate or index them.
    reference = run_reference(name, args)
    if reference is None:
        notes.append("builder reference could not be re-run")
    elif reference != repr(mine):
        notes.append(f"builder reference returns {reference}, independent implementation {mine!r}")
    method = "independent implementation from the docstring"
    if equal != exact:
        return Result(
            item["item_id"],
            item["family"],
            MISMATCH,
            method,
            f"assertion passes under == but the value's type differs ({expected_text} vs {mine!r})",
            notes,
        )
    return expect_label(item, "true" if exact else "false", method, notes)


# ---------------------------------------------------------------------------
# Library packages (mutant-kill, bug-function): fresh copies, pytest per test
# ---------------------------------------------------------------------------


def plain_dashes(text: str) -> str:
    return text.replace(chr(0x2014), "--").replace(chr(0x2013), "-")


class Packages:
    """One pristine copy per installed package; each run hard-links it into a fresh dir."""

    def __init__(self, root: Path) -> None:
        self.root = root
        self.lock = threading.Lock()
        self.bases: dict[str, Path] = {}

    def installed(self, package: str) -> Path:
        spec = importlib.util.find_spec(package)
        if spec is None or not spec.submodule_search_locations:
            raise AuditError(f"package {package} is not installed")
        return Path(next(iter(spec.submodule_search_locations)))

    def base(self, package: str) -> Path:
        with self.lock:
            if package not in self.bases:
                dest = self.root / package
                shutil.copytree(
                    self.installed(package),
                    dest / package,
                    ignore=shutil.ignore_patterns("__pycache__"),
                )
                self.bases[package] = dest
            return self.bases[package]

    def original(self, module: str) -> str:
        package = module.split("/", 1)[0]
        return (self.base(package) / module).read_text()


def package_version(package: str) -> str:
    try:
        return metadata.version(package)
    except metadata.PackageNotFoundError:
        return "missing"


def run_test(packages: Packages, module: str, module_source: str, node_id: str) -> str:
    """'pass', 'fail' or raises: one pytest node against a module variant."""
    package = module.split("/", 1)[0]
    out = run_sandboxed(
        {module: module_source},
        [
            "python-module",
            "pytest",
            node_id,
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
        ],
        timeout=120.0,
        base=packages.base(package),
        collect=("report.xml",),
    )
    if out.timed_out:
        raise AuditError("pytest timed out")
    report = out.collected.get("report.xml")
    if not report:
        raise AuditError(f"pytest wrote no report (exit {out.returncode})")
    cases = list(ET.fromstring(report).iter("testcase"))
    ran = [c for c in cases if c.find("skipped") is None]
    if not ran:
        raise AuditError("the named test was not collected or was skipped")
    bad = any(c.find("failure") is not None or c.find("error") is not None for c in ran)
    return "fail" if bad else "pass"


def apply_unified_diff(original: str, diff: str) -> str:
    """Apply a unified diff to ``original``; context must match (dashes normalised)."""
    src = original.splitlines(keepends=True)
    out: list[str] = []
    pos = 0
    lines = diff.splitlines(keepends=True)
    i = 0
    while i < len(lines) and not lines[i].startswith("@@"):
        i += 1
    if i == len(lines):
        raise AuditError("diff has no hunks")

    def same(a: str, b: str) -> bool:
        return plain_dashes(a.rstrip("\n")) == plain_dashes(b.rstrip("\n"))

    while i < len(lines):
        m = re.match(r"@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@", lines[i])
        if not m:
            raise AuditError("diff hunk header not recognised")
        start, count = int(m.group(1)), int(m.group(2) or 1)
        target = start - 1 if count else start
        if target < pos:
            raise AuditError("diff hunks overlap")
        out.extend(src[pos:target])
        pos = target
        i += 1
        while i < len(lines) and not lines[i].startswith("@@"):
            tag, body = lines[i][:1], lines[i][1:]
            if tag in (" ", "-"):
                if pos >= len(src) or not same(src[pos], body):
                    raise AuditError("diff context does not match the installed module")
                if tag == " ":
                    out.append(src[pos])
                pos += 1
            elif tag == "+":
                out.append(body if body.endswith("\n") else body + "\n")
            elif not lines[i].startswith("\\"):
                raise AuditError("diff line not recognised")
            i += 1
    out.extend(src[pos:])
    return "".join(out)


def verify_mutant_kill(item: dict[str, Any], ctx: Context) -> Result:
    src = item["source"]
    module, package = src["module"], src["module"].split("/", 1)[0]
    installed = package_version(package)
    if installed != src.get("package_version"):
        raise AuditError(
            f"{package} {installed} installed, item built on {src.get('package_version')}"
        )
    state = item["state"]
    diffs = [body for info, body in fenced_blocks(state) if info == "diff"]
    tm = re.search(r"^Test (\S+?)::(\S+) \(it passes on the original code\):$", state, re.M)
    if len(diffs) != 1 or not tm:
        raise AuditError("mutant-kill: diff or test id not found")
    if tm.group(1) != src["tests"]:
        raise AuditError("mutant-kill: test file differs from the source metadata")
    original = ctx.packages.original(module)
    mutant = apply_unified_diff(original, diffs[0])
    if mutant == original:
        raise AuditError("mutant-kill: the diff changes nothing")
    node = f"{tm.group(1)}::{tm.group(2)}"
    on_original = run_test(ctx.packages, module, original, node)
    notes: list[str] = []
    if on_original != "pass":
        return Result(
            item["item_id"],
            item["family"],
            MISMATCH,
            "pytest on a fresh copy",
            "the test fails on the original code",
        )
    on_mutant = run_test(ctx.packages, module, mutant, node)
    return expect_label(
        item,
        "true" if on_mutant == "fail" else "false",
        "mutant rebuilt from the diff, pytest on a fresh copy",
        notes,
    )


def module_functions(source: str) -> list[tuple[str, int, int]]:
    """(qualified name, first line with decorators, last line) of top-level defs and methods."""
    tree = ast.parse(source)
    out = []

    def span(node: ast.FunctionDef | ast.AsyncFunctionDef) -> tuple[int, int]:
        first = min([node.lineno, *[d.lineno for d in node.decorator_list]])
        return first, node.end_lineno or node.lineno

    for node in tree.body:
        if isinstance(node, ast.FunctionDef | ast.AsyncFunctionDef):
            out.append((node.name, *span(node)))
        elif isinstance(node, ast.ClassDef):
            for sub in node.body:
                if isinstance(sub, ast.FunctionDef | ast.AsyncFunctionDef):
                    out.append((f"{node.name}.{sub.name}", *span(sub)))
    return out


def changed_functions(original: str, shown: str) -> tuple[str, list[str]]:
    """Rebuild the mutant from the shown module; name the functions its edit touches."""
    a = original.splitlines(keepends=True)
    b = shown.splitlines(keepends=True)
    norm_a = [plain_dashes(x) for x in a]
    matcher = difflib.SequenceMatcher(None, norm_a, b, autojunk=False)
    mutant: list[str] = []
    touched: set[int] = set()
    for tag, i1, i2, j1, j2 in matcher.get_opcodes():
        if tag == "equal":
            mutant.extend(a[i1:i2])
            continue
        mutant.extend(b[j1:j2])
        touched.update(range(i1 + 1, i2 + 1) if i2 > i1 else [i1, i1 + 1])
    functions = module_functions(original)
    names = set()
    for line in touched:
        owner = [name for name, lo, hi in functions if lo <= line <= hi]
        names.add(owner[0] if owner else "<module level>")
    return "".join(mutant), sorted(names)


def verify_bug_function(item: dict[str, Any], ctx: Context) -> Result:
    src = item["source"]
    module, package = src["module"], src["module"].split("/", 1)[0]
    installed = package_version(package)
    if installed != src.get("package_version"):
        raise AuditError(
            f"{package} {installed} installed, item built on {src.get('package_version')}"
        )
    state = item["state"]
    marker = "\nModule source, as it is now (with the bug):\n```python\n"
    if marker not in state or not state.endswith("```\n"):
        raise AuditError("bug-function: module source block not found")
    shown = state.split(marker, 1)[1][: -len("```\n")]
    fm = re.search(r"^Failing test: (\S+)$", state, re.M)
    if not fm:
        raise AuditError("bug-function: failing test not named")
    original = ctx.packages.original(module)
    mutant, names = changed_functions(original, shown)
    if mutant == original:
        raise AuditError("bug-function: the shown module equals the installed one")
    method = "edit located by diff against the installed module, pytest on a fresh copy"
    if len(names) != 1:
        return Result(
            item["item_id"],
            item["family"],
            MISMATCH,
            method,
            f"the edit touches {len(names)} functions",
        )
    notes: list[str] = []
    on_original = run_test(ctx.packages, module, original, fm.group(1))
    on_mutant = run_test(ctx.packages, module, mutant, fm.group(1))
    if on_original != "pass" or on_mutant != "fail":
        return Result(
            item["item_id"],
            item["family"],
            MISMATCH,
            method,
            f"named test {on_original}es on the original and {on_mutant}s on the mutant",
            notes,
        )
    return expect_label(item, names[0], method, notes)


# ---------------------------------------------------------------------------
# patch-pair and failing-test: the source run's summary.json in the archive
# ---------------------------------------------------------------------------

PRUNE_DIRS = frozenset(
    {
        "workspace",
        "regrade_workspace",
        "node_modules",
        ".git",
        "target",
        "grok-session",
        "zcode-session",
        "devin-session",
        "muse-session-logs",
    }
)


class RunArchive:
    """Every run directory (one holding summary.json) under the main checkout's runs*."""

    def __init__(self, root: Path) -> None:
        self.root = root
        self.lock = threading.Lock()
        self._dirs: dict[str, list[Path]] | None = None
        self._by_id: dict[str, list[Path]] | None = None

    def dirs(self) -> dict[str, list[Path]]:
        with self.lock:
            if self._dirs is None:
                found: dict[str, list[Path]] = defaultdict(list)
                for top in sorted(p for p in self.root.glob("runs*") if p.is_dir()):
                    for dirpath, dirnames, filenames in os.walk(top):
                        if "summary.json" in filenames:
                            dirnames.clear()
                            found[Path(dirpath).name].append(Path(dirpath))
                            continue
                        dirnames[:] = sorted(d for d in dirnames if d not in PRUNE_DIRS)
                self._dirs = dict(found)
            return self._dirs

    def by_summary_id(self) -> dict[str, list[Path]]:
        """Run directories keyed by the run id inside summary.json (read once, lazily)."""
        with self.lock:
            if self._by_id is None:
                index: dict[str, list[Path]] = defaultdict(list)
                for paths in (self._dirs or {}).values():
                    for run_dir in paths:
                        try:
                            summary = json.loads((run_dir / "summary.json").read_text())
                        except (OSError, ValueError):
                            continue
                        if isinstance(summary, dict) and summary.get("run_id"):
                            index[str(summary["run_id"])].append(run_dir)
                self._by_id = dict(index)
            return self._by_id

    def runs(self, run_id: str) -> list[tuple[Path, dict[str, Any]]]:
        out = []
        candidates = self.dirs().get(run_id, [])
        if not candidates:
            # Some summaries carry a run id that differs from their directory name
            # (a secret scrubber redacted part of it), so fall back to the field.
            candidates = self.by_summary_id().get(run_id, [])
        for run_dir in candidates:
            try:
                summary = json.loads((run_dir / "summary.json").read_text())
            except (OSError, ValueError):
                continue
            if isinstance(summary, dict) and str(summary.get("run_id") or run_dir.name) == run_id:
                out.append((run_dir, summary))
        return out

    def task_runs(self, task_id: str) -> list[tuple[Path, dict[str, Any]]]:
        """Every run of one task (run directories are named <task id>-<hash>)."""
        out = []
        for name, paths in self.dirs().items():
            if name.startswith(task_id + "-") and re.fullmatch(
                r"[0-9a-f]{8}", name[len(task_id) + 1 :]
            ):
                for run_dir in paths:
                    try:
                        summary = json.loads((run_dir / "summary.json").read_text())
                    except (OSError, ValueError):
                        continue
                    if isinstance(summary, dict) and summary.get("task_id") == task_id:
                        out.append((run_dir, summary))
        return out


def normalized_patch(text: str) -> str:
    return "\n".join(
        line.rstrip() for line in text.splitlines() if not line.startswith("index ")
    ).strip()


def patch_digest(text: str) -> str:
    return hashlib.sha256(normalized_patch(text).encode()).hexdigest()


def excluded_dir(path: Path) -> bool:
    return any(part in str(path).lower() for part in ("contaminated", "discarded"))


@dataclass(frozen=True)
class RunVerdict:
    run_dir: Path
    summary: dict[str, Any]
    patch: str
    targets: dict[str, bool]
    passes: bool
    notes: tuple[str, ...]


def read_verdict(run_dir: Path, summary: dict[str, Any]) -> RunVerdict:
    verifier = summary.get("verifier") or {}
    f2p = verifier.get("fail_to_pass")
    if not summary.get("finished") or not isinstance(f2p, dict) or not f2p:
        raise AuditError("run is unfinished or has no per-test verifier results")
    targets = {str(k): bool(v) for k, v in f2p.items()}
    p2p = verifier.get("pass_to_pass") or {}
    ok_field = verifier.get("pass_to_pass_ok")
    all_p2p = all(bool(v) for v in p2p.values()) if isinstance(p2p, dict) else True
    notes = []
    if ok_field is None:
        p2p_ok = all_p2p
        notes.append("pass_to_pass_ok missing; used the per-test pass_to_pass results")
    else:
        p2p_ok = bool(ok_field)
        if bool(ok_field) != all_p2p:
            notes.append("pass_to_pass_ok disagrees with the per-test pass_to_pass results")
    if (summary.get("integrity_audit") or {}).get("contaminated"):
        notes.append("run is marked contaminated")
    if excluded_dir(run_dir):
        notes.append("run sits in an excluded archive directory")
    patch_path = run_dir / "final.patch"
    patch = patch_path.read_text(errors="replace") if patch_path.is_file() else ""
    return RunVerdict(
        run_dir, summary, patch, targets, p2p_ok and all(targets.values()), tuple(notes)
    )


def locate_run(ctx: Context, meta: dict[str, Any]) -> RunVerdict:
    hits = []
    for run_dir, summary in ctx.archive.runs(str(meta["run_id"])):
        patch_path = run_dir / "final.patch"
        text = patch_path.read_text(errors="replace") if patch_path.is_file() else ""
        if patch_digest(text) == meta.get("patch_sha256"):
            hits.append(read_verdict(run_dir, summary))
    if not hits:
        raise AuditError(f"run {meta['run_id']} with the recorded patch not found in the archive")
    # Copies of one run in several archive roots must agree.
    if len({(tuple(sorted(h.targets.items())), h.passes) for h in hits}) != 1:
        raise AuditError("copies of the source run disagree")
    return hits[0]


def task_version_note(ctx: Context, verdict: RunVerdict) -> list[str]:
    summary = verdict.summary
    recorded = summary.get("task_hash")
    tasks_dir = summary.get("suite_id") or summary.get("suite")
    task_id = str(summary.get("task_id"))
    if not recorded:
        return ["run records no task hash"]
    for root in sorted((ctx.main / "tasks").glob(f"*/{task_id}")):
        try:
            if task_hash(load_task(task_id, root.parent)) == recorded:
                return []
        except (OSError, ValueError):
            continue
    return [f"no task directory under tasks/ hashes to the run's recorded task_hash ({tasks_dir})"]


def flaky_note(ctx: Context, verdict: RunVerdict) -> list[str]:
    """Other runs of the same task with an identical patch but a different outcome."""
    task_id = str(verdict.summary.get("task_id"))
    digest = patch_digest(verdict.patch)
    others = []
    for run_dir, summary in ctx.archive.task_runs(task_id):
        if run_dir == verdict.run_dir or excluded_dir(run_dir):
            continue
        patch_path = run_dir / "final.patch"
        if (
            not patch_path.is_file()
            or patch_digest(patch_path.read_text(errors="replace")) != digest
        ):
            continue
        try:
            other = read_verdict(run_dir, summary)
        except AuditError:
            continue
        others.append(other)
    changed = [o for o in others if o.targets != verdict.targets or o.passes != verdict.passes]
    if changed:
        return [
            f"{len(changed)} other run(s) of the identical patch got a different verifier result"
        ]
    return []


def section_patch(state: str, heading: str, next_heading: str | None) -> str:
    start = state.find(f"## {heading}\n\n```diff\n")
    if start < 0:
        raise AuditError(f"patch section {heading!r} not found")
    body = state[start + len(f"## {heading}\n\n```diff\n") :]
    end = body.find(f"\n```\n\n## {next_heading}\n") if next_heading else body.rfind("\n```\n")
    if end < 0:
        raise AuditError(f"end of patch section {heading!r} not found")
    return body[:end]


def verify_patch_pair(item: dict[str, Any], ctx: Context) -> Result:
    src = item["source"]
    runs = {"passing": locate_run(ctx, src["passing"]), "failing": locate_run(ctx, src["failing"])}
    shown = {
        "A": normalized_patch(section_patch(item["state"], "Patch A", "Patch B")),
        "B": normalized_patch(section_patch(item["state"], "Patch B", None)),
    }
    notes: list[str] = []
    passing_labels = []
    for label, text in shown.items():
        owners = [k for k, v in runs.items() if normalized_patch(v.patch) == text]
        if len(owners) != 1:
            raise AuditError(f"patch {label} matches {len(owners)} of the source runs")
        if runs[owners[0]].passes:
            passing_labels.append(label)
    for v in runs.values():
        notes += list(v.notes) + task_version_note(ctx, v) + flaky_note(ctx, v)
    method = "summary.json verifier fields re-read from the archive"
    if len(passing_labels) != 1:
        return Result(
            item["item_id"],
            item["family"],
            MISMATCH,
            method,
            f"{len(passing_labels)} shown patches pass",
            notes,
        )
    return expect_label(item, passing_labels[0], method, sorted(set(notes)))


def verify_failing_test(item: dict[str, Any], ctx: Context) -> Result:
    src = item["source"]
    verdict = locate_run(ctx, src)
    shown = normalized_patch(section_patch(item["state"], "Candidate patch", "Hidden target tests"))
    if shown != normalized_patch(verdict.patch):
        raise AuditError("the shown patch differs from the run's final.patch")
    options = labels(item)
    missing = [o for o in options if o not in verdict.targets]
    method = "summary.json fail_to_pass re-read from the archive"
    if missing:
        return Result(
            item["item_id"],
            item["family"],
            MISMATCH,
            method,
            f"{len(missing)} listed tests have no verifier result",
        )
    failed = [o for o in options if not verdict.targets[o]]
    notes = list(verdict.notes) + task_version_note(ctx, verdict) + flaky_note(ctx, verdict)
    subset = len(options) < len(verdict.targets)
    says_subset = (
        "exactly one of the listed hidden target tests fails" in item["question"]["instructions"]
    )
    if subset != says_subset:
        notes.append("instructions do not match whether the listed tests are a subset")
    if len(failed) != 1:
        return Result(
            item["item_id"],
            item["family"],
            MISMATCH,
            method,
            f"{len(failed)} listed tests failed",
            notes,
        )
    return expect_label(item, failed[0], method, notes)


# ---------------------------------------------------------------------------
# Mined families: the raw cache records
# ---------------------------------------------------------------------------

CUTOFF = "2026-06-01"


class Mined:
    def __init__(self, root: Path) -> None:
        self.root = root
        self.lock = threading.Lock()
        self._cache: dict[str, list[dict[str, Any]]] = {}

    def records(self, name: str) -> list[dict[str, Any]]:
        with self.lock:
            if name not in self._cache:
                rows = []
                path = self.root / name
                if path.exists():
                    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
                        try:
                            value = json.loads(line)
                        except json.JSONDecodeError:
                            continue
                        if isinstance(value, dict):
                            rows.append(value)
                self._cache[name] = rows
            return self._cache[name]

    def tree(self, repo: str, sha: str) -> set[str] | None:
        path = self.root / "trees" / f"{repo.replace('/', '__')}__{sha}.json.gz"
        if not path.exists():
            return None
        with gzip.open(path, "rt", encoding="utf-8") as fh:
            data = json.load(fh)
        return {str(p) for p, _ in data.get("files", [])}


CODE_EXT = {".py", ".go", ".rs", ".ts", ".tsx", ".mts", ".cts", ".js", ".jsx", ".mjs", ".cjs"}
NOT_SOURCE_PARTS = {
    "test", "tests", "testing", "__tests__", "__test__", "spec", "specs", "testdata",
    "test-data", "test_data", "fixtures", "__fixtures__", "__mocks__", "e2e", "examples",
    "example", "docs", "doc", "benchmarks", "benchmark", "benches", "bench", "scripts",
    "vendor", "node_modules", "third_party", "third-party", "dist", "build", "target",
    ".github", "playground", "demo", "demos", "samples", "sample",
}  # fmt: skip
TEST_FILE = re.compile(
    r"(^test_.*\.py$|_test\.py$|^conftest\.py$|_test\.go$|_tests?\.rs$|^tests?\.rs$"
    r"|[-_.](test|spec|bench)\.[cm]?[jt]sx?$|\.d\.[cm]?ts$|\.min\.js$|\.pb\.go$|_pb2\.py$)"
)


def is_non_test_source(path: str) -> bool:
    p = PurePosixPath(path)
    if p.suffix not in CODE_EXT:
        return False
    if any(part.lower() in NOT_SOURCE_PARTS for part in p.parts[:-1]):
        return False
    return not TEST_FILE.search(p.name.lower())


def verify_fix_file(item: dict[str, Any], ctx: Context) -> Result:
    src = item["source"]
    rows = [
        r
        for r in ctx.mined.records("fix_file.jsonl")
        if r.get("repo") == src["repo"] and int(r.get("number", -1)) == int(src["pr"])
    ]
    if not rows:
        raise AuditError("fix-file: no mined record for this pull request")
    record = rows[0]
    notes = []
    if len({json.dumps([r.get("fixed_file"), sorted(r.get("other_files", []))]) for r in rows}) > 1:
        notes.append("the cache holds conflicting records for this pull request")
    if str(record.get("merged_at", ""))[:10] < CUTOFF:
        notes.append("merged before the cutoff")
    changed = [str(record["fixed_file"]), *[str(p) for p in record.get("other_files", [])]]
    sources = [p for p in changed if is_non_test_source(p)]
    method = "changed files of the mined pull request record"
    if len(sources) != 1:
        return Result(
            item["item_id"],
            item["family"],
            MISMATCH,
            method,
            f"the pull request changes {len(sources)} non-test source files by this audit's classifier",
            notes,
        )
    tree = ctx.mined.tree(str(record["repo"]), str(record["base_sha"]))
    if tree is None:
        notes.append("base tree not cached")
    elif sources[0] not in tree:
        notes.append("fixed file is not in the base commit's tree (new file)")
    if sources[0] not in item["state"].splitlines():
        notes.append("fixed file is not in the listed paths")
    return expect_label(item, unique_match(item, sources[0]), method, notes)


def advisory_records(ctx: Context, src: dict[str, Any]) -> list[dict[str, Any]]:
    ids = set(src.get("advisories") or [])
    rows = [
        r
        for r in ctx.mined.records("advisories.jsonl")
        if r.get("status") == "ok"
        and r.get("id") in ids
        and r.get("fix_commit") == src.get("fix_commit")
    ]
    if not rows:
        raise AuditError("no mined advisory record with this id and fix commit")
    return rows


def line_set(code: str) -> set[str]:
    return {ln.strip() for ln in code.splitlines() if ln.strip()}


def squash(code: str) -> str:
    return re.sub(r"\s+", "", code)


def vulnerable_side(a: str, b: str, before: str, after: str) -> str | None:
    """Which shown version matches the pre-fix file: by lines unique to each version."""
    la, lb = line_set(a), line_set(b)
    fb, fa = line_set(before), line_set(after)
    only_before, only_after = fb - fa, fa - fb
    score_a = len((la - lb) & only_before) - len((la - lb) & only_after)
    score_b = len((lb - la) & only_before) - len((lb - la) & only_after)
    if score_a > score_b:
        return "A"
    if score_b > score_a:
        return "B"
    # Differences only inside lines that occur in both files: fall back to similarity.
    ra = difflib.SequenceMatcher(None, squash(a), squash(before), autojunk=False).ratio()
    rb = difflib.SequenceMatcher(None, squash(b), squash(before), autojunk=False).ratio()
    ra2 = difflib.SequenceMatcher(None, squash(a), squash(after), autojunk=False).ratio()
    rb2 = difflib.SequenceMatcher(None, squash(b), squash(after), autojunk=False).ratio()
    if (ra - ra2) > (rb - rb2):
        return "A"
    if (rb - rb2) > (ra - ra2):
        return "B"
    return None


def verify_vuln_pair(item: dict[str, Any], ctx: Context) -> Result:
    src = item["source"]
    rows = advisory_records(ctx, src)
    entry = next(
        (f for r in rows for f in r.get("files", []) if f.get("path") == src.get("path")), None
    )
    if entry is None or not entry.get("before") or not entry.get("after"):
        raise AuditError("the advisory record has no before/after text for this file")
    state = item["state"]
    versions = dict(re.findall(r"^Version ([AB]):\n```\w*\n(.*?)^```", state, re.S | re.M))
    if sorted(versions) != ["A", "B"]:
        raise AuditError("vuln-pair: two versions not found")
    notes = []
    if str(src.get("commit_date") or "")[:10] < CUTOFF:
        notes.append("fix commit predates the cutoff")
    side = vulnerable_side(versions["A"], versions["B"], str(entry["before"]), str(entry["after"]))
    if side is None:
        raise AuditError("vuln-pair: neither version matches the pre-fix file better")
    return expect_label(
        item, side, "shown versions matched to the fix commit's before/after files", notes
    )


def verify_weakness_class(item: dict[str, Any], ctx: Context) -> Result:
    src = item["source"]
    rows = advisory_records(ctx, src)
    cwes = sorted({str(c) for r in rows for c in r.get("cwes", [])})
    notes = []
    if cwes != sorted(str(c) for c in src.get("cwes", [])):
        notes.append("source metadata CWEs differ from the raw advisory records")
    listing = {
        str(r.get("ghsa_id")): r
        for r in ctx.mined.records("advisory_listing.jsonl")
        if r.get("ghsa_id")
    }
    listed = sorted(
        {str(c) for a in src.get("advisories", []) for c in listing.get(a, {}).get("cwes", [])}
    )
    if listed and listed != cwes:
        notes.append("the advisory listing gives different CWEs")
    families = set()
    for cwe in cwes:
        m = re.search(r"(\d+)", cwe)
        if m:
            families |= {fam for fam, ids in CWE_FAMILIES.items() if int(m.group(1)) in ids}
    method = "advisory CWEs collapsed by the published CWE_FAMILIES mapping"
    if len(families) != 1:
        return Result(
            item["item_id"],
            item["family"],
            MISMATCH,
            method,
            f"CWEs map to {len(families)} families",
            notes,
        )
    return expect_label(item, families.pop(), method, notes)


def diff_lines(patch: str) -> tuple[Counter[str], Counter[str]]:
    added: Counter[str] = Counter()
    removed: Counter[str] = Counter()
    for line in patch.splitlines():
        if line.startswith("+") and not line.startswith("+++"):
            added[line[1:].rstrip()] += 1
        elif line.startswith("-") and not line.startswith("---"):
            removed[line[1:].rstrip()] += 1
    return added, removed


def patch_reproduces(before: str, after: str, patch: str) -> bool:
    """True when applying the shown hunks to ``before`` gives exactly ``after``.

    Line endings are compared loosely: GitHub patches keep CRLF where the cached
    file contents were stored with LF.
    """
    before, after, patch = (
        "\n".join(line.rstrip("\r") for line in x.split("\n")) for x in (before, after, patch)
    )
    try:
        got = apply_unified_diff(before, patch) if patch.strip() else before
    except AuditError:
        return False
    return got.rstrip("\n") == after.rstrip("\n")


def public_names(source: str | None) -> set[str] | None:
    """Module-level public def/class names and assigned constants (a deliberately small differ)."""
    if source is None:
        return set()
    try:
        tree = ast.parse(source)
    except (SyntaxError, ValueError):
        return None
    names = set()
    for node in tree.body:
        if isinstance(node, ast.FunctionDef | ast.AsyncFunctionDef | ast.ClassDef):
            names.add(node.name)
        elif isinstance(node, ast.Assign):
            names |= {t.id for t in node.targets if isinstance(t, ast.Name)}
        elif isinstance(node, ast.AnnAssign) and isinstance(node.target, ast.Name):
            names.add(node.target.id)
    return {n for n in names if not n.startswith("_")}


def importable(source: str | None) -> set[str]:
    if not source:
        return set()
    try:
        tree = ast.parse(source)
    except (SyntaxError, ValueError):
        return set()
    out = set()
    for node in ast.walk(tree):
        if isinstance(node, ast.ImportFrom | ast.Import):
            out |= {a.asname or a.name.split(".")[0] for a in node.names}
        elif isinstance(node, ast.Constant) and isinstance(node.value, str):
            out.add(node.value)
    return out


def shown_diff_problems(record: dict[str, Any], state: str) -> list[str]:
    """Files whose shown patch is missing or does not produce the labelled after-contents."""
    problems = []
    contents = record.get("contents", {})
    for entry in record.get("files", []):
        path = str(entry.get("path"))
        patch = entry.get("patch")
        if patch is None or patch not in state:
            problems.append(f"{path}: patch not shown verbatim")
            continue
        previous = str(entry.get("previous_path") or path)
        old_side, new_side = contents.get(previous), contents.get(path)
        if old_side is None and new_side is None:
            continue  # not a public module: shown but not labelled, by convention
        if previous != path and (old_side is None or new_side is None):
            continue  # a rename across the public boundary: only one side is labelled
        before = (old_side or {}).get("before") or ""
        after = (new_side or {}).get("after") or ""
        if not patch_reproduces(before, after, patch):
            problems.append(f"{path}: the shown patch does not turn the labelled before into after")
    return problems


def name_floor(contents: dict[str, Any]) -> str:
    """A small independent differ: public top-level names removed (major) or added (minor)."""
    floor = "patch"
    for path, got in contents.items():
        parts = PurePosixPath(path).parts
        if any(p.startswith("_") and p != "__init__.py" for p in parts) or "internal" in parts:
            continue
        old, new = public_names(got.get("before")), public_names(got.get("after"))
        if old is None or new is None:
            continue
        if {n for n in old - new if n not in importable(got.get("after"))}:
            return "major"
        if new - old:
            floor = "minor"
    return floor


SEMVER_RANK = {"patch": 0, "minor": 1, "major": 2}


def verify_semver_impact(item: dict[str, Any], ctx: Context) -> Result:
    src = item["source"]
    rows = [
        r
        for r in ctx.mined.records("semver.jsonl")
        if r.get("repo") == src["repo"] and int(r.get("number", -1)) == int(src["pr"])
    ]
    if not rows:
        raise AuditError("semver-impact: no mined record for this pull request")
    record = rows[0]
    notes = []
    commits = (record.get("before_sha"), record.get("after_sha"))
    if commits != (src.get("before_sha"), src.get("after_sha")):
        notes.append("record commits differ from the source metadata")
    # 1. The diff shown must be the diff the label was computed from.
    problems = shown_diff_problems(record, item["state"])
    # 2. Re-run the differ that defines the label.
    labelled = label_semver(record)
    if labelled is None:
        raise AuditError("semver-impact: the differ cannot label the cached sources")
    level, reasons = labelled
    if reasons[:20] != list(src.get("reasons", [])):
        notes.append("differ reasons differ from those recorded")
    # 3. The independent name differ must not find more than the label says.
    floor = name_floor(record.get("contents", {}))
    if SEMVER_RANK[floor] > SEMVER_RANK[level]:
        notes.append(f"independent name differ says at least {floor}")
    method = "verdict-v2 python api differ re-run on the cached sources, diff checked"
    if problems:
        return Result(
            item["item_id"],
            item["family"],
            MISMATCH,
            method,
            "the diff shown is not the change the label was computed from",
            notes + problems[:5],
        )
    return expect_label(item, level, method, notes)


# ---------------------------------------------------------------------------
# incident-root-cause: parse the logs, undo disclosed clock offsets, trace blame
# ---------------------------------------------------------------------------

MONTHS = {
    m: i + 1
    for i, m in enumerate(
        ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]
    )
}
PLAIN_LINE = re.compile(
    r"^(\d{4})-(\d\d)-(\d\d) (\d\d):(\d\d):(\d\d)\.(\d{3}) (DEBUG|INFO|WARN|ERROR) +\[([a-z]+)\] (.*)$"
)
SYSLOG_LINE = re.compile(
    r"^([A-Z][a-z]{2}) +(\d{1,2}) (\d\d):(\d\d):(\d\d)\.(\d{3}) \S+ ([a-z]+)\[\d+\]: "
    r"(DEBUG|INFO|WARN|ERROR) (.*)$"
)
KV_LINE = re.compile(r'^ts=(\S+)Z level=([a-z]+) svc=([a-z]+) msg="([^"]*)" host=\S+$')
NTP_OFFSET = re.compile(r"local clock is (?:now )?([\d.]+)s (behind|ahead of) reference time")
ACCESS_LINE = re.compile(r"^(?:GET|POST|PUT) (/v1/[a-z]+) 200 \d+ms$")

LogRow = tuple[datetime, str, str, str]


def _iso(text: str) -> datetime:
    return datetime.strptime(text, "%Y-%m-%dT%H:%M:%S.%f").replace(tzinfo=UTC)


def parse_log_line(line: str, year: int) -> LogRow:
    if line.startswith("{"):
        rec = json.loads(line)
        return _iso(rec["ts"].rstrip("Z")), rec["service"], rec["level"].upper(), rec["msg"]
    if m := PLAIN_LINE.match(line):
        y, mo, d, h, mi, s, ms = (int(g) for g in m.groups()[:7])
        return datetime(y, mo, d, h, mi, s, ms * 1000, tzinfo=UTC), m[9], m[8], m[10]
    if m := KV_LINE.match(line):
        return _iso(m[1]), m[3], m[2].upper(), m[4]
    if m := SYSLOG_LINE.match(line):
        h, mi, s, ms = (int(g) for g in m.groups()[2:6])
        stamp = datetime(year, MONTHS[m[1]], int(m[2]), h, mi, s, ms * 1000, tzinfo=UTC)
        return stamp, m[7], m[8], m[9]
    raise AuditError("incident: log line format not recognised")


def parse_incident(state: str) -> tuple[list[str], dict[str, str], list[LogRow]]:
    head, sep, logs = state.partition("\nLogs:\n")
    ym = re.search(r"services, (\d{4})-", head)
    lm = re.search(r"Services \(address\): (.*)\.\n", head)
    if not sep or not ym or not lm:
        raise AuditError("incident: header not recognised")
    pairs = re.findall(r"([a-z]+) \((\d+\.\d+\.\d+\.\d+)\)", lm.group(1))
    rows = [parse_log_line(line, int(ym.group(1))) for line in logs.splitlines() if line.strip()]
    return [n for n, _ in pairs], {ip: n for n, ip in pairs}, rows


def corrected_times(rows: Sequence[LogRow]) -> list[float]:
    """Seconds after the first line, undoing every clock offset an ntp line discloses."""
    base = rows[0][0]
    shown = [(ts - base).total_seconds() for ts, *_ in rows]
    shifts: dict[str, tuple[float, float]] = {}
    for t, (_, service, _, msg) in zip(shown, rows, strict=True):
        if m := NTP_OFFSET.search(msg):
            off = float(m[1]) * (-1 if m[2] == "behind" else 1)
            start = t if "clock stepped" in msg else -math.inf
            shifts[service] = (start, off)
    out = []
    for t, (_, service, _, _) in zip(shown, rows, strict=True):
        start, off = shifts.get(service, (0.0, 0.0))
        out.append(t - off if t >= start else t)
    return out


def route_owners(rows: Sequence[LogRow]) -> dict[str, str]:
    owners: dict[str, set[str]] = defaultdict(set)
    for _, service, _, msg in rows:
        if m := ACCESS_LINE.match(msg):
            owners[m[1]].add(service)
    return {route: next(iter(s)) for route, s in owners.items() if len(s) == 1}


def referenced(
    msg: str, known: set[str], by_ip: dict[str, str], routes: dict[str, str], skip: str
) -> list[str]:
    named = [tok for tok in re.findall(r"[a-z]+", msg) if tok in known]
    named += [by_ip[ip] for ip in re.findall(r"\b(\d+\.\d+\.\d+\.\d+):\d+", msg) if ip in by_ip]
    named += [routes[r] for r in re.findall(r"(/v1/[a-z]+)", msg) if r in routes]
    return [n for n in named if n != skip]


# Warnings a reader can see are not failures of the service that logs them:
# a client's own misuse, a soft limit that still allows traffic, housekeeping,
# and clock disclosures other than a step (a step is itself an anomaly).
BENIGN = re.compile(
    r"deprecated endpoint|exceeded soft rate limit, allowing burst|^rate limit at \d+% for client key"
    r"|^deprecated config key|^log shipper slow|^container running as root"
    r"|^ntp: (?!clock stepped)"
)
# Lines that name another service without blaming it: a service's own
# connection pool, its resolver failing a lookup, its credentials being
# refused, its own deadline or internal error while proxying a route.
NOT_BLAME = re.compile(
    r"connection pool|from pool|connections to \S+ checked out|\(pool size \d+\)|no such host"
    r"|slow lookup for|401 Unauthorized|deadline of \d+ms exceeded|^unhandled error|^no route for"
)
# Mild warnings that name a service but show neither blame nor failure.
MILD = re.compile(r"slow lookup for|connection pool \S+: \d+% in use")
# A fault that announces its own recovery was transient, not the incident.
RECOVERY = re.compile(
    r"recovered|resumed|rolled back to previous version|resynchronized|lookups healthy"
    r"|log rotation freed|autovacuum finished|batch job finished"
)
RECOVERY_WINDOW_S = 120.0
# Nothing in these simulations goes wrong in the first minute and a half, so a
# message shape a service already logged then is background, not the incident.
QUIET_START_S = 80.0


def _shape(msg: str) -> str:
    return re.sub(r"\d+", "#", re.sub(r"\b[0-9a-f]{7,8}\b", "#", msg))


def incident_trace(state: str) -> tuple[str, list[str]]:
    """The root by reading the logs alone.

    Trouble is a WARN or ERROR line that is not benign and whose shape the
    service did not already log in the quiet opening minute. A complaint is a
    trouble line naming one other service (by name, address or route); it is
    credible when that service shows trouble of its own. Each service's first
    credible complaint is its blame edge; following the edges from every
    troubled service ends at a service that blames nobody (in a cycle, the
    member whose trouble started first), and the end that the most trouble
    lines lead to is the root.
    """
    services, by_ip, rows = parse_incident(state)
    known = set(services)
    raw = corrected_times(rows)
    zero = min(raw)
    times = [t - zero for t in raw]
    routes = route_owners(rows)
    order = sorted(range(len(rows)), key=lambda i: times[i])
    background = {(rows[i][1], _shape(rows[i][3])) for i in order if times[i] < QUIET_START_S}
    recovered = [(rows[i][1], times[i]) for i in order if RECOVERY.search(rows[i][3])]

    def transient(i: int) -> bool:
        return any(
            svc == rows[i][1] and 0 <= t - times[i] <= RECOVERY_WINDOW_S for svc, t in recovered
        )

    trouble = [
        i
        for i in order
        if rows[i][2] in ("WARN", "ERROR")
        and not BENIGN.search(rows[i][3])
        and (rows[i][1], _shape(rows[i][3])) not in background
        and not transient(i)
    ]
    onset: dict[str, float] = {}
    count: Counter[str] = Counter()
    for i in trouble:
        onset.setdefault(rows[i][1], times[i])
        count[rows[i][1]] += 1
    pointed: dict[str, Counter[str]] = defaultdict(Counter)
    own: dict[str, float] = {}  # first trouble that is about the service itself
    for i in trouble:
        svc, msg = rows[i][1], rows[i][3]
        refs = set(referenced(msg, known, by_ip, routes, svc))
        if MILD.search(msg):
            continue
        if not refs or NOT_BLAME.search(msg):
            own.setdefault(svc, times[i])
        elif len(refs) == 1 and next(iter(refs)) in onset:
            pointed[svc][next(iter(refs))] += 1
    # A victim keeps naming what it is failing on; stray slow-call or decoy
    # lines name something else once or twice.
    blame = {svc: c.most_common(1)[0][0] for svc, c in pointed.items()}
    votes: Counter[str] = Counter()
    for svc in onset:
        path = [svc]
        while path[-1] in blame and blame[path[-1]] not in path:
            path.append(blame[path[-1]])
        end = path[-1]
        if end in blame:  # a cycle: the member whose own trouble began first
            cycle = path[path.index(blame[end]) :]
            end = min(cycle, key=lambda s: (own.get(s, math.inf), onset[s]))
        votes[end] += count[svc]
    if not votes:
        raise AuditError("incident: no trouble lines to trace")
    best = max(votes.values())
    top = sorted((s for s, v in votes.items() if v == best), key=lambda s: onset[s])
    notes = [] if len(top) == 1 else ["tracer tie broken by earliest trouble"]
    return top[0], notes


class Regenerated:
    """The incident simulations, rebuilt from the recorded build seed (for a second check)."""

    def __init__(self) -> None:
        self.lock = threading.Lock()
        self._drafts: dict[str, Any] | None = None

    def draft(self, item_id: str) -> Any:
        with self.lock:
            if self._drafts is None:
                ctx = BuildContext(seed=BUILD_SEED, repo=REPO, per_family=BUILD_PER_FAMILY)
                self._drafts = {item.item_id: draft for item, draft in ops_gen.generate(ctx)}
            return self._drafts.get(item_id)


def blame_chain_problem(
    answer: str, lead: float, first_symptom: dict[str, float], first_complaint: dict[str, str]
) -> str | None:
    """Every victim's chain of first complaints must run back in time to ``answer``."""
    for svc in first_complaint:
        node, hops = svc, 0
        while node in first_complaint:
            blamed = first_complaint[node]
            start = lead if blamed == answer else first_symptom.get(blamed, math.inf)
            if start >= first_symptom[node]:
                return "a victim blames a service that failed after it"
            node, hops = blamed, hops + 1
            if hops > len(first_complaint) + 1:
                return "the blame chain loops"
        if node != answer:
            return "a blame chain ends away from the stored answer"
    return None


def simulation_trace(item: dict[str, Any], draft: Any) -> str | None:  # noqa: PLR0911, one return per failed check
    """Follow first complaints over the rendered text, using the simulation's line kinds.

    Returns None when the rendered logs support the stored root, else the reason.
    """
    spec = draft.spec
    records = spec["records"]
    services, by_ip, rows = parse_incident(item["state"])
    if draft.state != item["state"] or len(rows) != len(records):
        return "regenerated item differs from the stored one"
    zero = (rows[0][0] - spec["base"]).total_seconds()
    times = [t + zero for t in corrected_times(rows)]
    for row, rec, t in zip(rows, records, times, strict=True):
        if (row[1], row[2], row[3]) != (rec["service"], rec["level"], rec["msg"]):
            return "a rendered line differs from its simulation record"
        if abs(t * 1000 - rec["t_ms"]) > 1.5:
            return "a disclosed clock offset does not restore a line's true time"
    firsts = [i for i, r in enumerate(records) if r["cat"] == "root_first"]
    if len(firsts) != 1 or rows[firsts[0]][1] != item["answer"]:
        return "the first root anomaly is not on the stored answer"
    lead = times[firsts[0]]
    symptoms = sorted(
        (i for i, r in enumerate(records) if r["cat"] == "symptom"), key=lambda i: times[i]
    )
    if not symptoms or times[symptoms[0]] <= lead:
        return "a symptom precedes the root's first anomaly"
    routes = route_owners(rows)
    first_symptom: dict[str, float] = {}
    first_complaint: dict[str, str] = {}
    for i in symptoms:
        svc = rows[i][1]
        first_symptom.setdefault(svc, times[i])
        named = set(referenced(rows[i][3], set(services), by_ip, routes, svc))
        if named and svc not in first_complaint:
            if len(named) != 1:
                return "a victim's first complaint names several services"
            first_complaint[svc] = next(iter(named))
    return blame_chain_problem(item["answer"], lead, first_symptom, first_complaint)


def verify_incident(item: dict[str, Any], ctx: Context) -> Result:
    guess, notes = incident_trace(item["state"])
    draft = ctx.regenerated.draft(item["item_id"])
    if draft is None:
        notes.append("simulation could not be regenerated from the build seed")
        return expect_label(item, guess, "log tracer (text only)", notes)
    problem = simulation_trace(item, draft)
    if problem is not None:
        return Result(
            item["item_id"],
            item["family"],
            MISMATCH,
            "simulation re-run and rendered-log trace",
            problem,
            notes,
        )
    if guess == item["answer"]:
        return Result(
            item["item_id"],
            item["family"],
            VERIFIED,
            "log tracer (text only), confirmed by the regenerated simulation's trace",
            "",
            notes,
        )
    notes.append(f"text-only tracer picked another service ({guess})")
    return Result(
        item["item_id"],
        item["family"],
        VERIFIED,
        "regenerated simulation's trace over the rendered logs (text-only tracer disagreed)",
        "",
        notes,
    )


# ---------------------------------------------------------------------------
# Runner
# ---------------------------------------------------------------------------


@dataclass
class Context:
    main: Path
    packages: Packages
    archive: RunArchive
    mined: Mined
    regenerated: Regenerated


VERIFIERS: dict[str, Callable[[dict[str, Any], Context], Result]] = {
    "code-output": verify_code_output,
    "type-check-pair": verify_type_check_pair,
    "patch-pair": verify_patch_pair,
    "failing-test": verify_failing_test,
    "fix-file": verify_fix_file,
    "bug-function": verify_bug_function,
    "vuln-pair": verify_vuln_pair,
    "weakness-class": verify_weakness_class,
    "mutant-kill": verify_mutant_kill,
    "expected-value": verify_expected_value,
    "incident-root-cause": verify_incident,
    "semver-impact": verify_semver_impact,
    "constraint-pick": verify_constraint_pick,
    "entailment": verify_entailment,
    "word-problem": verify_word_problem,
    "estimate-band": verify_estimate_band,
    "table-lookup": verify_table_lookup,
    "table-count-band": verify_table_count_band,
    "policy-decision": verify_policy_decision,
    "policy-clause": verify_policy_clause,
}


def verify(item: dict[str, Any], ctx: Context) -> Result:
    fn = VERIFIERS.get(item["family"])
    if fn is None:
        return Result(
            item["item_id"], item["family"], UNVERIFIABLE, "none", "no verifier for this family"
        )
    try:
        return fn(item, ctx)
    except AuditError as error:
        return Result(item["item_id"], item["family"], UNVERIFIABLE, "", str(error))
    except Exception as error:  # an audit bug must not hide the other results
        return Result(
            item["item_id"],
            item["family"],
            UNVERIFIABLE,
            "",
            f"audit error: {type(error).__name__}: {error}",
        )


def summarize(
    sample: dict[str, list[dict[str, Any]]], totals: Counter[str], results: list[Result]
) -> dict[str, Any]:
    by_family: dict[str, list[Result]] = defaultdict(list)
    for r in results:
        by_family[r.family].append(r)
    out = {}
    for family in sorted(sample):
        rs = by_family[family]
        counts = Counter(r.status for r in rs)
        out[family] = {
            "items": totals[family],
            "sampled": len(rs),
            "verified": counts[VERIFIED],
            "mismatched": counts[MISMATCH],
            "unverifiable": counts[UNVERIFIABLE],
            "unverifiable_reasons": dict(Counter(r.reason for r in rs if r.status == UNVERIFIABLE)),
            "mismatch_reasons": dict(Counter(r.reason for r in rs if r.status == MISMATCH)),
            "methods": dict(Counter(r.method for r in rs if r.status != UNVERIFIABLE)),
            "notes": dict(Counter(n for r in rs for n in r.notes)),
        }
    return out


def print_table(summary: dict[str, Any]) -> None:
    head = (
        f"{'family':20} {'items':>5} {'sampled':>7} {'verified':>8} {'mismatch':>8} {'unverif.':>8}"
    )
    print(head)
    print("-" * len(head))
    tot: Counter[str] = Counter()
    for family, s in summary.items():
        print(
            f"{family:20} {s['items']:5} {s['sampled']:7} {s['verified']:8} {s['mismatched']:8} "
            f"{s['unverifiable']:8}"
        )
        for key in ("items", "sampled", "verified", "mismatched", "unverifiable"):
            tot[key] += s[key]
    print("-" * len(head))
    print(
        f"{'total':20} {tot['items']:5} {tot['sampled']:7} {tot['verified']:8} {tot['mismatched']:8} "
        f"{tot['unverifiable']:8}"
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--items", type=Path, default=DEFAULT_ITEMS)
    parser.add_argument("-o", "--out", type=Path, default=DEFAULT_OUT)
    parser.add_argument("--seed", type=int, default=SAMPLE_SEED)
    parser.add_argument("--share", type=float, default=SAMPLE_SHARE)
    parser.add_argument("--min-per-family", type=int, default=MIN_PER_FAMILY)
    parser.add_argument("--workers", type=int, default=MAX_WORKERS)
    parser.add_argument("--family", action="append", help="audit only these (repeatable)")
    parser.add_argument("--data-root", type=Path, default=MAIN_CHECKOUT)
    args = parser.parse_args()
    workers = max(1, min(args.workers, MAX_WORKERS))

    raw = args.items.read_bytes()
    items = [json.loads(line) for line in raw.decode().splitlines() if line.strip()]
    totals = Counter(it["family"] for it in items)
    sample = sample_items(items, args.seed, args.share, args.min_per_family)
    if args.family:
        sample = {f: v for f, v in sample.items() if f in args.family}
    todo = [it for family in sorted(sample) for it in sample[family]]
    started = datetime.now(UTC)
    with tempfile.TemporaryDirectory(prefix="verdict-v2-audit-pkgs-") as tmp:
        ctx = Context(
            main=args.data_root,
            packages=Packages(Path(tmp)),
            archive=RunArchive(args.data_root),
            mined=Mined(REPO / "verdict-v2-items" / "mined"),
            regenerated=Regenerated(),
        )
        with ThreadPoolExecutor(max_workers=workers) as pool:
            results = list(pool.map(lambda it: verify(it, ctx), todo))
    summary = summarize(sample, totals, results)
    report = {
        "suite": "verdict-v2",
        "rule": "admission gate rule 5: ground truth re-verified on a 5% random sample by re-execution",
        "generated_at": started.isoformat(timespec="seconds"),
        "items_file": str(args.items),
        "items_sha256": hashlib.sha256(raw).hexdigest(),
        "sample_seed": args.seed,
        "sample_share": args.share,
        "min_per_family": args.min_per_family,
        "build_seed_assumed": BUILD_SEED,
        "families": summary,
        "results": [asdict(r) for r in results],
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print_table(summary)
    bad = [r for r in results if r.status != VERIFIED]
    for r in bad:
        print(f"{r.status:12} {r.family:20} {r.item_id}  {r.reason}")
    print(f"wrote {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
