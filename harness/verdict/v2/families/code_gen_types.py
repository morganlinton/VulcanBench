"""Typed Python snippet templates for ``type-check-pair``.

Each template is a realistic 20 to 60 line module with named single edits
(guards removed, annotations narrowed, casts or ignores added or dropped).
A variant is the base with some set of edits applied; two variants that
differ by exactly one edit form a candidate pair, and mypy decides which (if
exactly one) passes ``--strict``. Because edits both break and repair code,
the passing snippet is sometimes the longer one, sometimes the one with the
``cast`` or ``# type: ignore``, sometimes the one with more ``Optional``.
"""

from __future__ import annotations

import json
import random
import re
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from string import Template

from harness.verdict.v2.families.code_gen_exec import cache_root, content_hash
from harness.verdict.v2.families.code_gen_select import ChoiceBalancer, ChoiceConfig
from harness.verdict.v2.items import Item, choice_question, make_item
from harness.verdict.v2.registry import BuildContext

NOUNS = [
    ("Order", "order"),
    ("Invoice", "invoice"),
    ("Sensor", "sensor"),
    ("Ticket", "ticket"),
    ("Parcel", "parcel"),
    ("Booking", "booking"),
    ("Account", "account"),
    ("Shipment", "shipment"),
    ("Reading", "reading"),
    ("Task", "task"),
    ("Member", "member"),
    ("Asset", "asset"),
]
FIELDS = ["weight", "amount", "score", "quantity", "priority", "level", "count", "size"]
KEYS = ["region", "owner", "status", "channel", "team", "zone"]


@dataclass(frozen=True)
class TypeTemplate:
    name: str
    source: str
    edits: dict[str, tuple[str, str]]
    variants: tuple[tuple[str, ...], ...]


def names(rng: random.Random) -> dict[str, str]:
    (cls, obj), (cls2, obj2) = rng.sample(NOUNS, 2)
    fld, fld2 = rng.sample(FIELDS, 2)
    return {
        "Cls": cls,
        "obj": obj,
        "Cls2": cls2,
        "obj2": obj2,
        "fld": fld,
        "fld2": fld2,
        "key": rng.choice(KEYS),
    }


def _t(
    name: str, source: str, edits: dict[str, tuple[str, str]], variants: list[tuple[str, ...]]
) -> TypeTemplate:
    return TypeTemplate(name, source, edits, tuple(variants))


RAW: list[TypeTemplate] = [
    _t(
        "optional-guard",
        """\
from dataclasses import dataclass, field


@dataclass
class $Cls:
    name: str
    $fld: int | None = None
    tags: list[str] = field(default_factory=list)


def total_$fld(items: list[$Cls]) -> int:
    total = 0
    for item in items:
        if item.$fld is None:
            continue
        total += item.$fld
    return total


def describe(item: $Cls) -> str:
    label = item.name.upper()
    if item.tags:
        label += " [" + ", ".join(sorted(item.tags)) + "]"
    return label


def heaviest(items: list[$Cls]) -> $Cls | None:
    best: $Cls | None = None
    for item in items:
        if item.$fld is None:
            continue
        if best is None or (best.$fld or 0) < item.$fld:
            best = item
    return best
""",
        {
            "drop-guard": (
                "        if item.$fld is None:\n            continue\n        total",
                "        total",
            ),
            "ignore": (
                "        total += item.$fld\n",
                "        total += item.$fld  # type: ignore[operator]\n",
            ),
            "drop-or": ("(best.$fld or 0) < item.$fld", "best.$fld < item.$fld"),
            "narrow-return": (
                "def heaviest(items: list[$Cls]) -> $Cls | None:",
                "def heaviest(items: list[$Cls]) -> $Cls:",
            ),
            "cast-or": ("(best.$fld or 0) < item.$fld", "cast(int, best.$fld) < item.$fld"),
            "fld-int": ("    $fld: int | None = None\n", "    $fld: int = 0\n"),
            "label-int": (
                '        label += " [" + ", ".join(sorted(item.tags)) + "]"',
                "        label += len(item.tags)",
            ),
            "sorted-none": ('", ".join(sorted(item.tags))', '", ".join(sorted(item.tags) or None)'),
        },
        [
            (),
            ("drop-guard",),
            ("ignore",),
            ("drop-guard", "ignore"),
            ("drop-or",),
            ("narrow-return",),
            ("cast-or",),
            ("drop-or", "cast-or"),
        ],
    ),
    _t(
        "dict-get",
        """\
from collections.abc import Iterable


def tally(${key}s: Iterable[str]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for $key in ${key}s:
        counts[$key] = counts.get($key, 0) + 1
    return counts


def top(counts: dict[str, int], limit: int = 3) -> list[tuple[str, int]]:
    ranked = sorted(counts.items(), key=lambda pair: (-pair[1], pair[0]))
    return ranked[:limit]


def share(counts: dict[str, int], $key: str) -> float:
    total = sum(counts.values())
    if total == 0:
        return 0.0
    return counts.get($key, 0) / total


def merge(left: dict[str, int], right: dict[str, int]) -> dict[str, int]:
    merged = dict(left)
    for name, value in right.items():
        merged[name] = merged.get(name, 0) + value
    return merged
""",
        {
            "drop-default": (
                "counts[$key] = counts.get($key, 0) + 1",
                "counts[$key] = counts.get($key) + 1",
            ),
            "ignore-default": (
                "counts[$key] = counts.get($key, 0) + 1",
                "counts[$key] = counts.get($key) + 1  # type: ignore[operator]",
            ),
            "share-optional": (
                "return counts.get($key, 0) / total",
                "return counts.get($key) / total",
            ),
            "merge-optional": (
                "merged[name] = merged.get(name, 0) + value",
                "merged[name] = (merged.get(name) or 0) + value",
            ),
            "merge-ignore": (
                "merged[name] = merged.get(name, 0) + value",
                "merged[name] = merged.get(name, 0) + value  # type: ignore[operator]",
            ),
            "top-str": (
                "def top(counts: dict[str, int], limit: int = 3) -> list[tuple[str, int]]:",
                "def top(counts: dict[str, int], limit: int = 3) -> list[str]:",
            ),
            "share-int": (
                "def share(counts: dict[str, int], $key: str) -> float:",
                "def share(counts: dict[str, int], $key: str) -> int:",
            ),
            "counter-any": (
                "    counts: dict[str, int] = {}\n",
                "    counts: dict[str, float] = {}\n",
            ),
        },
        [
            (),
            ("drop-default",),
            ("ignore-default",),
            ("share-optional",),
            ("merge-optional",),
            ("merge-ignore",),
        ],
    ),
    _t(
        "invariance",
        """\
from collections.abc import Sequence


def mean(values: Sequence[float]) -> float:
    if not values:
        raise ValueError("no values")
    return sum(values) / len(values)


def spread(values: Sequence[float]) -> float:
    return max(values) - min(values)


def normalise(values: list[float]) -> list[float]:
    low, high = min(values), max(values)
    width = (high - low) or 1.0
    return [(v - low) / width for v in values]


def report(${fld}s: list[int]) -> str:
    avg = mean(${fld}s)
    rng = spread(${fld}s)
    return f"mean={avg:.2f} spread={rng:.2f}"


def rescale(${fld}s: list[float]) -> list[float]:
    return normalise(${fld}s)
""",
        {
            "list-param": (
                "def mean(values: Sequence[float]) -> float:",
                "def mean(values: list[float]) -> float:",
            ),
            "spread-list": (
                "def spread(values: Sequence[float]) -> float:",
                "def spread(values: list[float]) -> float:",
            ),
            "rescale-int": (
                "def rescale(${fld}s: list[float]) -> list[float]:",
                "def rescale(${fld}s: list[int]) -> list[float]:",
            ),
            "rescale-seq": (
                "def normalise(values: list[float]) -> list[float]:",
                "def normalise(values: Sequence[float]) -> list[float]:",
            ),
            "mean-int": (
                "def mean(values: Sequence[float]) -> float:",
                "def mean(values: Sequence[int]) -> float:",
            ),
            "report-float": (
                "def report(${fld}s: list[int]) -> str:",
                "def report(${fld}s: list[float]) -> str:",
            ),
            "width-int": (
                "    width = (high - low) or 1.0\n",
                "    width: int = (high - low) or 1\n",
            ),
        },
        [
            (),
            ("list-param",),
            ("spread-list",),
            ("rescale-int",),
            ("rescale-seq",),
            ("rescale-int", "rescale-seq"),
        ],
    ),
    _t(
        "protocol",
        """\
from typing import Protocol


class Renderer(Protocol):
    def render(self, width: int) -> str: ...

    @property
    def title(self) -> str: ...


class ${Cls}Card:
    def __init__(self, title: str, $fld: int) -> None:
        self._title = title
        self.$fld = $fld

    @property
    def title(self) -> str:
        return self._title

    def render(self, width: int) -> str:
        line = f"{self.title}: {self.$fld}"
        return line[:width].ljust(width)


def render_all(items: list[Renderer], width: int = 20) -> list[str]:
    return [item.render(width) for item in items]


cards: list[Renderer] = [${Cls}Card("north", 3), ${Cls}Card("south", 5)]
print("\\n".join(render_all(cards)))
""",
        {
            "param-str": (
                "    def render(self, width: int) -> str:\n        line",
                "    def render(self, width: str) -> str:\n        line",
            ),
            "return-optional": (
                "    def render(self, width: int) -> str:\n        line",
                "    def render(self, width: int) -> str | None:\n        line",
            ),
            "extra-default": (
                "    def render(self, width: int) -> str:\n        line",
                '    def render(self, width: int, fill: str = " ") -> str:\n        line',
            ),
            "attr-title": (
                "    @property\n    def title(self) -> str:\n        return self._title",
                '    title = "card"',
            ),
            "rename": (
                "    def render(self, width: int) -> str:\n        line",
                "    def draw(self, width: int) -> str:\n        line",
            ),
            "return-int": (
                "        return line[:width].ljust(width)",
                "        return len(line[:width])",
            ),
            "list-cards": ("cards: list[Renderer] = [", "cards: list[${Cls}Card] = ["),
            "title-method": (
                "    @property\n    def title(self) -> str:\n        return self._title",
                "    def title(self) -> str:\n        return self._title",
            ),
        },
        [
            (),
            ("param-str",),
            ("return-optional",),
            ("extra-default",),
            ("attr-title",),
            ("rename",),
        ],
    ),
    _t(
        "typeddict",
        """\
from typing import NotRequired, TypedDict


class ${Cls}Row(TypedDict):
    id: int
    $key: str
    $fld: float
    note: NotRequired[str]


def parse(line: str) -> ${Cls}Row:
    ident, $key, $fld = line.split(",")
    row: ${Cls}Row = {"id": int(ident), "$key": $key.strip(), "$fld": float($fld)}
    return row


def label(row: ${Cls}Row) -> str:
    note = row.get("note", "")
    return f"{row['id']}:{row['$key']}:{row['$fld']:.1f} {note}".strip()


def heavy(rows: list[${Cls}Row], cutoff: float) -> list[int]:
    return [row["id"] for row in rows if row["$fld"] > cutoff]
""",
        {
            "missing-key": (
                'row: ${Cls}Row = {"id": int(ident), "$key": $key.strip(), "$fld": float($fld)}',
                'row: ${Cls}Row = {"id": int(ident), "$key": $key.strip()}',
            ),
            "wrong-type": ('"id": int(ident),', '"id": ident,'),
            "extra-key": ('"$fld": float($fld)}', '"$fld": float($fld), "extra": 1}'),
            "with-note": ('"$fld": float($fld)}', '"$fld": float($fld), "note": ""}'),
            "bad-get": ('note = row.get("note", "")', 'note = row["notes"]'),
            "label-missing": ("row['$key']", "row['${key}_name']"),
            "heavy-str": (
                '    return [row["id"] for row in rows if row["$fld"] > cutoff]',
                '    return [row["$key"] for row in rows if row["$fld"] > cutoff]',
            ),
            "total-false": (
                "class ${Cls}Row(TypedDict):",
                "class ${Cls}Row(TypedDict, total=False):",
            ),
        },
        [(), ("missing-key",), ("wrong-type",), ("extra-key",), ("with-note",), ("bad-get",)],
    ),
    _t(
        "overload",
        """\
from typing import overload


@overload
def parse_$fld(raw: str) -> int: ...
@overload
def parse_$fld(raw: bytes) -> str: ...
def parse_$fld(raw: str | bytes) -> int | str:
    if isinstance(raw, bytes):
        return raw.decode("ascii").strip()
    return int(raw.strip() or "0")


def total(raws: list[str]) -> int:
    return sum(parse_$fld(raw) for raw in raws)


def names(raws: list[bytes]) -> list[str]:
    return [parse_$fld(raw).upper() for raw in raws]


print(total(["3", " 4 ", ""]), names([b" ab ", b"cd"]))
""",
        {
            "narrow-impl": (
                "def parse_$fld(raw: str | bytes) -> int | str:",
                "def parse_$fld(raw: str) -> int | str:",
            ),
            "bytes-total": (
                "def total(raws: list[str]) -> int:",
                "def total(raws: list[bytes]) -> int:",
            ),
            "names-str": (
                "def names(raws: list[bytes]) -> list[str]:",
                "def names(raws: list[str]) -> list[str]:",
            ),
            "widen-return": (
                "def parse_$fld(raw: bytes) -> str: ...",
                "def parse_$fld(raw: bytes) -> str | None: ...",
            ),
            "ignore-total": (
                "    return sum(parse_$fld(raw) for raw in raws)",
                "    return sum(parse_$fld(raw) for raw in raws)  # type: ignore[misc]",
            ),
            "names-int": (
                "    return [parse_$fld(raw).upper() for raw in raws]",
                "    return [parse_$fld(raw) for raw in raws]",
            ),
            "total-float": (
                "def total(raws: list[str]) -> int:",
                "def total(raws: list[str]) -> float:",
            ),
            "impl-object": (
                "def parse_$fld(raw: str | bytes) -> int | str:",
                "def parse_$fld(raw: object) -> int | str:",
            ),
        },
        [
            (),
            ("narrow-impl",),
            ("bytes-total",),
            ("names-str",),
            ("widen-return",),
            ("ignore-total",),
        ],
    ),
    _t(
        "generic-stack",
        """\
from typing import Generic, TypeVar

T = TypeVar("T")


class Stack(Generic[T]):
    def __init__(self) -> None:
        self._items: list[T] = []

    def push(self, item: T) -> None:
        self._items.append(item)

    def pop(self) -> T | None:
        if not self._items:
            return None
        return self._items.pop()

    def peek(self) -> T:
        return self._items[-1]

    def __len__(self) -> int:
        return len(self._items)


def ${fld}_sum(values: list[int]) -> int:
    stack: Stack[int] = Stack()
    for value in values:
        stack.push(value)
    total = 0
    while len(stack):
        top = stack.pop()
        if top is not None:
            total += top
    return total + stack_default(stack)


def stack_default(stack: Stack[int]) -> int:
    return stack.peek() if len(stack) else 0
""",
        {
            "drop-check": (
                "        if top is not None:\n            total += top",
                "        total += top",
            ),
            "push-str": ("        stack.push(value)", "        stack.push(str(value))"),
            "peek-optional": (
                "    def peek(self) -> T:\n        return self._items[-1]",
                "    def peek(self) -> T | None:\n        return self._items[-1]",
            ),
            "cast-top": (
                "        if top is not None:\n            total += top",
                "        total += cast(int, top)",
            ),
            "ignore-top": (
                "        if top is not None:\n            total += top",
                "        total += top  # type: ignore[operator]",
            ),
            "return-str": (
                "    return total + stack_default(stack)",
                "    return str(total + stack_default(stack))",
            ),
            "stack-str": (
                "def stack_default(stack: Stack[int]) -> int:",
                "def stack_default(stack: Stack[str]) -> int:",
            ),
            "stack-object": (
                "    stack: Stack[int] = Stack()",
                "    stack: Stack[object] = Stack()",
            ),
        },
        [(), ("drop-check",), ("push-str",), ("peek-optional",), ("cast-top",), ("ignore-top",)],
    ),
    _t(
        "dataclass-fields",
        """\
from dataclasses import dataclass, field, replace


@dataclass(frozen=True)
class $Cls:
    ident: str
    $fld: int
    $key: str = "default"
    history: tuple[int, ...] = field(default=())


def bump(item: $Cls, delta: int) -> $Cls:
    return replace(item, $fld=item.$fld + delta, history=(*item.history, item.$fld))


def rollback(item: $Cls) -> $Cls:
    if not item.history:
        return item
    *rest, last = item.history
    return replace(item, $fld=last, history=tuple(rest))


def build(ident: str) -> $Cls:
    return $Cls(ident, 0)
""",
        {
            "order": (
                '    $key: str = "default"\n    history',
                '    $key: str = "default"\n    extra: int\n    history',
            ),
            "mutate": (
                "    return replace(item, $fld=item.$fld + delta, history=(*item.history, item.$fld))",
                "    item.$fld += delta\n    return item",
            ),
            "list-history": (
                "    return replace(item, $fld=last, history=tuple(rest))",
                "    return replace(item, $fld=last, history=rest)",
            ),
            "build-str": ("    return $Cls(ident, 0)", '    return $Cls(ident, "0")'),
            "build-kw": (
                "    return $Cls(ident, 0)",
                '    return $Cls(ident=ident, $fld=0, $key="x")',
            ),
            "history-list": (
                "    history: tuple[int, ...] = field(default=())",
                "    history: tuple[int, ...] = field(default_factory=tuple)",
            ),
            "rollback-none": (
                "    if not item.history:\n        return item",
                "    if not item.history:\n        return None",
            ),
            "unfrozen": ("@dataclass(frozen=True)", "@dataclass"),
        },
        [(), ("order",), ("mutate",), ("list-history",), ("build-str",), ("build-kw",)],
    ),
    _t(
        "callable-variance",
        """\
from collections.abc import Callable


def apply_all(fn: Callable[[int], str], values: list[int]) -> list[str]:
    return [fn(v) for v in values]


def show(value: object) -> str:
    return f"<{value}>"


def label_$fld(value: int) -> str:
    return "high" if value > 10 else "low"


def pick(flag: bool) -> Callable[[int], str]:
    return show if flag else label_$fld


print(apply_all(show, [1, 2]), apply_all(label_$fld, [5, 50]))
print(pick(True)(3))
""",
        {
            "narrow-show": ("def show(value: object) -> str:", "def show(value: bool) -> str:"),
            "str-label": (
                'def label_$fld(value: int) -> str:\n    return "high" if value > 10 else "low"',
                'def label_$fld(value: str) -> str:\n    return "high" if len(value) > 10 else "low"',
            ),
            "return-int": (
                "def pick(flag: bool) -> Callable[[int], str]:",
                "def pick(flag: bool) -> Callable[[str], str]:",
            ),
            "widen-apply": (
                "def apply_all(fn: Callable[[int], str], values: list[int]) -> list[str]:",
                "def apply_all(fn: Callable[[object], str], values: list[int]) -> list[str]:",
            ),
            "float-show": ("def show(value: object) -> str:", "def show(value: float) -> str:"),
            "lambda-pick": (
                "    return show if flag else label_$fld",
                "    return show if flag else (lambda v: v * 2)",
            ),
            "print-int": ("print(pick(True)(3))", 'print(pick(True)("3"))'),
            "apply-list": (
                "    return [fn(v) for v in values]",
                "    return [fn(str(v)) for v in values]",
            ),
        },
        [(), ("narrow-show",), ("str-label",), ("return-int",), ("widen-apply",), ("float-show",)],
    ),
    _t(
        "json-cast",
        """\
import json
from typing import cast


def load_${fld}s(raw: str) -> dict[str, int]:
    data = json.loads(raw)
    if not isinstance(data, dict):
        raise ValueError("expected an object")
    return {str(k): int(v) for k, v in data.items()}


def load_names(raw: str) -> list[str]:
    return cast(list[str], json.loads(raw))


def first_name(raw: str) -> str:
    names = load_names(raw)
    return names[0] if names else ""


def dump(${fld}s: dict[str, int]) -> str:
    return json.dumps(${fld}s, sort_keys=True)
""",
        {
            "drop-cast": (
                "    return cast(list[str], json.loads(raw))",
                "    return json.loads(raw)",
            ),
            "redundant-cast": (
                '    return names[0] if names else ""',
                '    return cast(str, names[0]) if names else ""',
            ),
            "raw-dict": ("    return {str(k): int(v) for k, v in data.items()}", "    return data"),
            "dump-cast": (
                "    return json.dumps(${fld}s, sort_keys=True)",
                "    return cast(str, json.dumps(${fld}s, sort_keys=True))",
            ),
            "ignore-drop": (
                "    return cast(list[str], json.loads(raw))",
                "    return json.loads(raw)  # type: ignore[no-any-return]",
            ),
            "names-set": (
                "def first_name(raw: str) -> str:\n    names = load_names(raw)",
                "def first_name(raw: str) -> str:\n    names = set(load_names(raw))",
            ),
            "dump-bytes": (
                "def dump(${fld}s: dict[str, int]) -> str:",
                "def dump(${fld}s: dict[str, int]) -> bytes:",
            ),
            "assert-dict": (
                '    if not isinstance(data, dict):\n        raise ValueError("expected an object")\n',
                "",
            ),
        },
        [(), ("drop-cast",), ("redundant-cast",), ("raw-dict",), ("dump-cast",), ("ignore-drop",)],
    ),
    _t(
        "missing-return",
        """\
from enum import Enum


class Tier(Enum):
    LOW = "low"
    MID = "mid"
    HIGH = "high"


def tier_for(${fld}: int) -> Tier:
    if ${fld} < 10:
        return Tier.LOW
    elif ${fld} < 100:
        return Tier.MID
    else:
        return Tier.HIGH


def fee(tier: Tier) -> float:
    if tier is Tier.LOW:
        return 0.0
    if tier is Tier.MID:
        return 2.5
    return 5.0


def describe(${fld}: int) -> str:
    tier = tier_for(${fld})
    return f"{tier.value}:{fee(tier):.2f}"
""",
        {
            "drop-else": (
                "    elif ${fld} < 100:\n        return Tier.MID\n    else:\n        return Tier.HIGH",
                "    elif ${fld} < 100:\n        return Tier.MID\n    elif ${fld} >= 100:\n        return Tier.HIGH",
            ),
            "fee-str": ("        return 2.5\n", '        return "2.5"\n'),
            "value-int": (
                '    return f"{tier.value}:{fee(tier):.2f}"',
                "    return tier.value + fee(tier)",
            ),
            "fee-none": ("    return 5.0\n", "    return None\n"),
            "raise-end": (
                "    elif ${fld} < 100:\n        return Tier.MID\n    else:\n        return Tier.HIGH",
                "    elif ${fld} < 100:\n        return Tier.MID\n    raise AssertionError(${fld})",
            ),
            "tier-str": ("def tier_for(${fld}: int) -> Tier:", "def tier_for(${fld}: int) -> str:"),
            "fee-int": ("def fee(tier: Tier) -> float:", "def fee(tier: Tier) -> int:"),
            "is-eq": ("    if tier is Tier.LOW:", "    if tier == Tier.LOW:"),
        },
        [(), ("drop-else",), ("fee-str",), ("value-int",), ("fee-none",), ("raise-end",)],
    ),
    _t(
        "literal-final",
        """\
from typing import Final, Literal

Mode = Literal["read", "write", "append"]
DEFAULT_MODE: Final = "read"
RETRIES: Final[int] = 3


def open_$obj(path: str, mode: Mode = DEFAULT_MODE) -> str:
    return f"{mode}:{path}"


def batch(paths: list[str], mode: Mode) -> list[str]:
    return [open_$obj(p, mode) for p in paths]


def retry_budget(extra: int) -> int:
    budget = RETRIES + extra
    return max(budget, 0)


print(batch(["a", "b"], "write"), retry_budget(2))
""",
        {
            "str-mode": (
                "def batch(paths: list[str], mode: Mode) -> list[str]:",
                "def batch(paths: list[str], mode: str) -> list[str]:",
            ),
            "bad-literal": (
                'print(batch(["a", "b"], "write"), retry_budget(2))',
                'print(batch(["a", "b"], "delete"), retry_budget(2))',
            ),
            "reassign": (
                "    budget = RETRIES + extra\n",
                "    global RETRIES\n    RETRIES = 4\n    budget = RETRIES + extra\n",
            ),
            "default-str": ('DEFAULT_MODE: Final = "read"', 'DEFAULT_MODE: str = "read"'),
            "final-str": ("RETRIES: Final[int] = 3", "RETRIES: Final[str] = 3"),
            "mode-var": (
                'print(batch(["a", "b"], "write"), retry_budget(2))',
                'mode = "write"\nprint(batch(["a", "b"], mode), retry_budget(2))',
            ),
            "final-mode": (
                'print(batch(["a", "b"], "write"), retry_budget(2))',
                'MODE: Final = "write"\nprint(batch(["a", "b"], MODE), retry_budget(2))',
            ),
            "budget-str": ("    return max(budget, 0)", '    return max(str(budget), "0")'),
        },
        [(), ("str-mode",), ("bad-literal",), ("reassign",), ("default-str",), ("final-str",)],
    ),
    _t(
        "abstract",
        """\
from abc import ABC, abstractmethod


class Exporter(ABC):
    @abstractmethod
    def export(self, rows: list[dict[str, str]]) -> str: ...

    @abstractmethod
    def suffix(self) -> str: ...

    def filename(self, stem: str) -> str:
        return f"{stem}.{self.suffix()}"


class CsvExporter(Exporter):
    def export(self, rows: list[dict[str, str]]) -> str:
        if not rows:
            return ""
        header = ",".join(rows[0])
        body = [",".join(row.values()) for row in rows]
        return "\\n".join([header, *body])

    def suffix(self) -> str:
        return "csv"


def run(rows: list[dict[str, str]]) -> tuple[str, str]:
    exporter = CsvExporter()
    return exporter.filename("$obj"), exporter.export(rows)
""",
        {
            "drop-suffix": ('    def suffix(self) -> str:\n        return "csv"\n', ""),
            "base-instance": ("    exporter = CsvExporter()", "    exporter = Exporter()"),
            "suffix-int": (
                '    def suffix(self) -> str:\n        return "csv"',
                "    def suffix(self) -> int:\n        return 1",
            ),
            "export-list": (
                "    def export(self, rows: list[dict[str, str]]) -> str:\n        if not rows:",
                "    def export(self, rows: list[dict[str, int]]) -> str:\n        if not rows:",
            ),
            "annotated-var": (
                "    exporter = CsvExporter()",
                "    exporter: Exporter = CsvExporter()",
            ),
            "filename-int": ('        return f"{stem}.{self.suffix()}"', "        return stem + 1"),
            "rows-values": (
                '        body = [",".join(row.values()) for row in rows]',
                '        body = [",".join(row) for row in rows]',
            ),
            "export-none": (
                '        if not rows:\n            return ""',
                "        if not rows:\n            return None",
            ),
        },
        [
            (),
            ("drop-suffix",),
            ("base-instance",),
            ("suffix-int",),
            ("export-list",),
            ("annotated-var",),
        ],
    ),
    _t(
        "exhaustive-match",
        """\
from enum import Enum, auto
from typing import assert_never


class State(Enum):
    PENDING = auto()
    ACTIVE = auto()
    CLOSED = auto()


def next_state(state: State) -> State:
    match state:
        case State.PENDING:
            return State.ACTIVE
        case State.ACTIVE:
            return State.CLOSED
        case State.CLOSED:
            return State.CLOSED
        case _:
            assert_never(state)


def is_open(state: State) -> bool:
    return state in (State.PENDING, State.ACTIVE)


def advance(states: list[State]) -> list[State]:
    return [next_state(s) for s in states if is_open(s)]
""",
        {
            "drop-case": ("        case State.CLOSED:\n            return State.CLOSED\n", ""),
            "drop-default": ("        case _:\n            assert_never(state)\n", ""),
            "bool-return": (
                "    return state in (State.PENDING, State.ACTIVE)",
                "    return state.name",
            ),
            "extra-member": (
                "    CLOSED = auto()\n",
                "    CLOSED = auto()\n    ARCHIVED = auto()\n",
            ),
            "return-none": (
                "        case State.ACTIVE:\n            return State.CLOSED",
                "        case State.ACTIVE:\n            return None",
            ),
            "advance-set": (
                "def advance(states: list[State]) -> list[State]:",
                "def advance(states: list[State]) -> set[State]:",
            ),
            "name-match": (
                "        case State.PENDING:\n            return State.ACTIVE",
                "        case State.PENDING:\n            return State.ACTIVE.name",
            ),
            "is-open-name": (
                "    return state in (State.PENDING, State.ACTIVE)",
                '    return state.name in ("PENDING", "ACTIVE")',
            ),
        },
        [
            (),
            ("drop-case",),
            ("drop-default",),
            ("bool-return",),
            ("extra-member",),
            ("return-none",),
            ("drop-case", "drop-default"),
        ],
    ),
    _t(
        "self-optional",
        """\
class Connection:
    def __init__(self, host: str) -> None:
        self.host = host
        self.sent: list[bytes] = []

    def send(self, payload: bytes) -> int:
        self.sent.append(payload)
        return len(payload)


class ${Cls}Client:
    def __init__(self) -> None:
        self.conn: Connection | None = None

    def connect(self, host: str) -> None:
        self.conn = Connection(host)

    def publish(self, message: str) -> int:
        if self.conn is None:
            raise RuntimeError("not connected")
        return self.conn.send(message.encode())

    def host(self) -> str:
        return self.conn.host if self.conn else ""
""",
        {
            "drop-check": (
                '        if self.conn is None:\n            raise RuntimeError("not connected")\n',
                "",
            ),
            "send-str": (
                "        return self.conn.send(message.encode())",
                "        return self.conn.send(message)",
            ),
            "host-direct": (
                '        return self.conn.host if self.conn else ""',
                "        return self.conn.host",
            ),
            "assert-conn": (
                '        if self.conn is None:\n            raise RuntimeError("not connected")\n',
                "        assert self.conn is not None\n",
            ),
            "ignore-host": (
                '        return self.conn.host if self.conn else ""',
                "        return self.conn.host  # type: ignore[union-attr]",
            ),
            "publish-none": (
                '            raise RuntimeError("not connected")',
                "            return None",
            ),
            "sent-str": (
                "        self.sent: list[bytes] = []",
                "        self.sent: list[str] = []",
            ),
            "walrus": (
                '        return self.conn.host if self.conn else ""',
                '        return conn.host if (conn := self.conn) else ""',
            ),
        },
        [(), ("drop-check",), ("send-str",), ("host-direct",), ("assert-conn",), ("ignore-host",)],
    ),
    _t(
        "returns-none",
        """\
def ranked(${fld}s: list[int]) -> list[int]:
    ordered = sorted(${fld}s, reverse=True)
    return ordered


def dedupe(values: list[str]) -> list[str]:
    seen: set[str] = set()
    out: list[str] = []
    for value in values:
        if value not in seen:
            seen.add(value)
            out.append(value)
    return out


def summary(${fld}s: list[int]) -> str:
    top = ranked(${fld}s)[:3]
    return ", ".join(str(v) for v in top)


def extend_all(groups: list[list[int]]) -> list[int]:
    flat: list[int] = []
    for group in groups:
        flat.extend(group)
    return flat
""",
        {
            "sort-inplace": (
                "    ordered = sorted(${fld}s, reverse=True)",
                "    ordered = ${fld}s.sort(reverse=True)",
            ),
            "append-result": (
                "            out.append(value)",
                "            out = out.append(value)",
            ),
            "extend-return": (
                "        flat.extend(group)\n    return flat",
                "        flat.extend(group)\n    return flat.extend([])",
            ),
            "sort-copy": (
                "    ordered = sorted(${fld}s, reverse=True)",
                "    ordered = list(${fld}s)\n    ordered.sort(reverse=True)",
            ),
            "join-int": ('    return ", ".join(str(v) for v in top)', '    return ", ".join(top)'),
            "dedupe-set": ("    seen: set[str] = set()", "    seen: list[str] = []"),
            "seen-add": ("            seen.add(value)", "            seen = seen | {value}"),
            "summary-top": ("    top = ranked(${fld}s)[:3]", "    top = ranked(${fld}s).reverse()"),
        },
        [
            (),
            ("sort-inplace",),
            ("append-result",),
            ("extend-return",),
            ("sort-copy",),
            ("join-int",),
        ],
    ),
    _t(
        "iterators",
        """\
from collections.abc import Iterable, Iterator


def windows(values: list[int], size: int) -> Iterator[tuple[int, ...]]:
    for start in range(len(values) - size + 1):
        yield tuple(values[start : start + size])


def running_total(values: Iterable[int]) -> Iterator[int]:
    total = 0
    for value in values:
        total += value
        yield total


def chunked(values: list[str], size: int) -> Iterator[list[str]]:
    for start in range(0, len(values), size):
        yield values[start : start + size]


def peak_window(values: list[int], size: int) -> int:
    return max((sum(w) for w in windows(values, size)), default=0)
""",
        {
            "yield-str": (
                "        total += value\n        yield total",
                "        total += value\n        yield str(total)",
            ),
            "return-list": (
                "def windows(values: list[int], size: int) -> Iterator[tuple[int, ...]]:",
                "def windows(values: list[int], size: int) -> list[tuple[int, ...]]:",
            ),
            "tuple-exact": (
                "def windows(values: list[int], size: int) -> Iterator[tuple[int, ...]]:",
                "def windows(values: list[int], size: int) -> Iterator[tuple[int, int]]:",
            ),
            "iterable-return": (
                "def chunked(values: list[str], size: int) -> Iterator[list[str]]:",
                "def chunked(values: list[str], size: int) -> Iterable[list[str]]:",
            ),
            "chunk-join": (
                "        yield values[start : start + size]",
                '        yield ",".join(values[start : start + size])',
            ),
            "peak-list": (
                "    return max((sum(w) for w in windows(values, size)), default=0)",
                "    return max([sum(w) for w in windows(values, size)])",
            ),
            "peak-none": (
                "    return max((sum(w) for w in windows(values, size)), default=0)",
                "    return max((sum(w) for w in windows(values, size)), default=None)",
            ),
            "total-float": ("    total = 0\n", "    total = 0.0\n"),
        },
        [
            (),
            ("yield-str",),
            ("return-list",),
            ("tuple-exact",),
            ("iterable-return",),
            ("chunk-join",),
        ],
    ),
    _t(
        "eq-override",
        """\
from dataclasses import dataclass


class Point:
    def __init__(self, x: int, y: int) -> None:
        self.x = x
        self.y = y

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, Point):
            return NotImplemented
        return (self.x, self.y) == (other.x, other.y)

    def __hash__(self) -> int:
        return hash((self.x, self.y))


@dataclass(frozen=True)
class ${Cls}Stop:
    name: str
    at: Point


def unique_points(stops: list[${Cls}Stop]) -> set[Point]:
    return {stop.at for stop in stops}
""",
        {
            "narrow-other": (
                "    def __eq__(self, other: object) -> bool:\n        if not isinstance(other, Point):\n            return NotImplemented\n",
                "    def __eq__(self, other: Point) -> bool:\n",
            ),
            "drop-isinstance": (
                "        if not isinstance(other, Point):\n            return NotImplemented\n",
                "",
            ),
            "hash-str": (
                "    def __hash__(self) -> int:\n        return hash((self.x, self.y))",
                "    def __hash__(self) -> str:\n        return str((self.x, self.y))",
            ),
            "list-return": (
                "    return {stop.at for stop in stops}",
                "    return [stop.at for stop in stops]",
            ),
            "ignore-eq": (
                "    def __eq__(self, other: object) -> bool:\n        if not isinstance(other, Point):\n            return NotImplemented\n",
                "    def __eq__(self, other: Point) -> bool:  # type: ignore[override]\n",
            ),
            "points-list": (
                "def unique_points(stops: list[${Cls}Stop]) -> set[Point]:",
                "def unique_points(stops: list[${Cls}Stop]) -> frozenset[Point]:",
            ),
            "at-optional": ("    at: Point\n", "    at: Point | None\n"),
            "eq-tuple": (
                "        return (self.x, self.y) == (other.x, other.y)",
                "        return (self.x, self.y) == other",
            ),
        },
        [
            (),
            ("narrow-other",),
            ("drop-isinstance",),
            ("hash-str",),
            ("list-return",),
            ("ignore-eq",),
        ],
    ),
    _t(
        "async",
        """\
import asyncio


async def fetch_$fld(ident: int) -> int:
    await asyncio.sleep(0)
    return ident * 2


async def gather_${fld}s(idents: list[int]) -> list[int]:
    results = await asyncio.gather(*(fetch_$fld(i) for i in idents))
    return list(results)


async def best_$fld(idents: list[int]) -> int:
    values = await gather_${fld}s(idents)
    return max(values, default=0)


def main() -> None:
    print(asyncio.run(best_$fld([1, 2, 3])))
""",
        {
            "no-await": (
                "    values = await gather_${fld}s(idents)",
                "    values = gather_${fld}s(idents)",
            ),
            "sync-fetch": (
                "async def fetch_$fld(ident: int) -> int:\n    await asyncio.sleep(0)\n",
                "def fetch_$fld(ident: int) -> int:\n",
            ),
            "run-direct": (
                "    print(asyncio.run(best_$fld([1, 2, 3])))",
                "    print(best_$fld([1, 2, 3]) + 1)",
            ),
            "tuple-results": ("    return list(results)", "    return results"),
            "sum-values": ("    return max(values, default=0)", "    return sum(values)"),
            "gather-list": (
                "    results = await asyncio.gather(*(fetch_$fld(i) for i in idents))",
                "    results = [await fetch_$fld(i) for i in idents]",
            ),
            "main-async": ("def main() -> None:", "async def main() -> None:"),
            "sleep-result": (
                "    await asyncio.sleep(0)\n    return ident * 2",
                "    delay = await asyncio.sleep(0)\n    return ident * 2 + delay",
            ),
        },
        [(), ("no-await",), ("sync-fetch",), ("run-direct",), ("tuple-results",), ("sum-values",)],
    ),
    _t(
        "namedtuple-property",
        """\
from typing import NamedTuple


class Range(NamedTuple):
    low: int
    high: int

    def width(self) -> int:
        return self.high - self.low


class ${Cls}Window:
    def __init__(self, low: int, high: int) -> None:
        self._range = Range(low, high)

    @property
    def span(self) -> Range:
        return self._range

    @span.setter
    def span(self, value: Range) -> None:
        self._range = value

    def widen(self, by: int) -> None:
        self.span = Range(self.span.low - by, self.span.high + by)
""",
        {
            "mutate-field": (
                "        self.span = Range(self.span.low - by, self.span.high + by)",
                "        self._range.low -= by",
            ),
            "setter-tuple": (
                "        self.span = Range(self.span.low - by, self.span.high + by)",
                "        self.span = (self.span.low - by, self.span.high + by)",
            ),
            "replace-method": (
                "        self.span = Range(self.span.low - by, self.span.high + by)",
                "        self.span = self.span._replace(low=self.span.low - by)",
            ),
            "width-float": (
                "    def width(self) -> int:\n        return self.high - self.low",
                "    def width(self) -> int:\n        return (self.high - self.low) / 2",
            ),
            "drop-setter": (
                "    @span.setter\n    def span(self, value: Range) -> None:\n        self._range = value\n",
                "",
            ),
            "setter-int": (
                "    def span(self, value: Range) -> None:",
                "    def span(self, value: int) -> None:",
            ),
            "width-prop": (
                "    def width(self) -> int:\n        return self.high - self.low",
                "    @property\n    def width(self) -> int:\n        return self.high - self.low",
            ),
            "low-str": (
                "        self._range = Range(low, high)",
                "        self._range = Range(str(low), high)",
            ),
        },
        [
            (),
            ("mutate-field",),
            ("setter-tuple",),
            ("replace-method",),
            ("width-float",),
            ("drop-setter",),
        ],
    ),
    _t(
        "annotations",
        """\
from collections import defaultdict


def group_by_$key(rows: list[tuple[str, int]]) -> dict[str, list[int]]:
    groups: defaultdict[str, list[int]] = defaultdict(list)
    for $key, value in rows:
        groups[$key].append(value)
    return dict(groups)


def largest_group(groups: dict[str, list[int]]) -> str:
    return max(groups, key=lambda k: len(groups[k]))


def averages(groups: dict[str, list[int]]) -> dict[str, float]:
    return {k: sum(v) / len(v) for k, v in groups.items() if v}


def format_row($key: str, avg: float) -> str:
    return f"{$key:<10}{avg:>8.2f}"
""",
        {
            "drop-return": (
                "def format_row($key: str, avg: float) -> str:",
                "def format_row($key: str, avg: float):",
            ),
            "drop-param": (
                "def largest_group(groups: dict[str, list[int]]) -> str:",
                "def largest_group(groups) -> str:",
            ),
            "bare-default": (
                "    groups: defaultdict[str, list[int]] = defaultdict(list)",
                "    groups = defaultdict(list)",
            ),
            "avg-int": (
                "def averages(groups: dict[str, list[int]]) -> dict[str, float]:",
                "def averages(groups: dict[str, list[int]]) -> dict[str, int]:",
            ),
            "any-param": (
                "def format_row($key: str, avg: float) -> str:",
                "def format_row($key: object, avg: float) -> str:",
            ),
            "lambda-key": (
                "    return max(groups, key=lambda k: len(groups[k]))",
                "    return max(groups.items(), key=lambda kv: len(kv[1]))",
            ),
            "avg-zero": (
                "    return {k: sum(v) / len(v) for k, v in groups.items() if v}",
                "    return {k: sum(v) // max(len(v), 1) for k, v in groups.items()}",
            ),
            "dict-items": ("    return dict(groups)", "    return groups"),
        },
        [(), ("drop-return",), ("drop-param",), ("bare-default",), ("avg-int",), ("any-param",)],
    ),
    _t(
        "tuple-unpack",
        """\
def split_$obj(record: str) -> tuple[str, int, float]:
    name, count, price = record.split(";")
    return name.strip(), int(count), float(price)


def line_total(record: str) -> float:
    _, count, price = split_$obj(record)
    return count * price


def names(records: list[str]) -> list[str]:
    return [split_$obj(r)[0] for r in records]


def totals(records: list[str]) -> tuple[float, ...]:
    return tuple(line_total(r) for r in records)


def cheapest(records: list[str]) -> str:
    return min(records, key=line_total)


print(totals(["a;2;1.5", "b;1;4.0"]), names(["x;1;1"]))
""",
        {
            "two-unpack": (
                "    _, count, price = split_$obj(record)",
                "    count, price = split_$obj(record)",
            ),
            "index-3": (
                "    return [split_$obj(r)[0] for r in records]",
                "    return [split_$obj(r)[3] for r in records]",
            ),
            "fixed-totals": (
                "def totals(records: list[str]) -> tuple[float, ...]:",
                "def totals(records: list[str]) -> tuple[float, float]:",
            ),
            "int-total": (
                "def line_total(record: str) -> float:",
                "def line_total(record: str) -> int:",
            ),
            "star-unpack": (
                "    _, count, price = split_$obj(record)",
                "    *_, count, price = split_$obj(record)",
            ),
            "names-tuple": (
                "    return [split_$obj(r)[0] for r in records]",
                "    return [split_$obj(r)[:1] for r in records]",
            ),
            "return-list": (
                "    return name.strip(), int(count), float(price)",
                "    return [name.strip(), int(count), float(price)]",
            ),
            "float-count": (
                "    return name.strip(), int(count), float(price)",
                "    return name.strip(), float(count), float(price)",
            ),
        },
        [(), ("two-unpack",), ("index-3",), ("fixed-totals",), ("int-total",), ("star-unpack",)],
    ),
]

# ``cast`` has to be importable in every template that uses it in an edit.
TEMPLATES: dict[str, TypeTemplate] = {}
for _tpl in RAW:
    _src = _tpl.source
    if any("cast(" in new for _, new in _tpl.edits.values()) and "import cast" not in _src:
        _src = "from typing import cast\n" + _src
    TEMPLATES[_tpl.name] = TypeTemplate(_tpl.name, _src, _tpl.edits, _tpl.variants)


def variants(template: TypeTemplate) -> list[tuple[str, ...]]:
    """The listed variants plus every two-edit combination, in a stable order."""
    names = sorted(template.edits)
    out = list(dict.fromkeys([*template.variants, *[(e,) for e in names]]))
    for i, first in enumerate(names):
        for second in names[i + 1 :]:
            if (first, second) not in out and (second, first) not in out:
                out.append((first, second))
    return out


def render(template: TypeTemplate, variant: tuple[str, ...], values: dict[str, str]) -> str | None:
    """The base with the variant's edits applied; None if an edit does not apply."""
    text = Template(template.source).substitute(values)
    for name in variant:
        old, new = (Template(part).substitute(values) for part in template.edits[name])
        if old not in text:
            return None
        text = text.replace(old, new, 1)
    return text


# ----------------------------------------------------------------- builder

NAME_VARIANTS = 4
MYPY_FLAGS = ("--strict", "--python-version", "3.12", "--no-error-summary", "--show-error-codes")


def mypy_version() -> str:
    proc = subprocess.run(
        [sys.executable, "-m", "mypy", "--version"], capture_output=True, text=True, check=False
    )
    if proc.returncode != 0:
        raise RuntimeError("mypy is not installed for this interpreter")
    return " ".join(proc.stdout.split()[:2])  # "mypy 2.1.0"


def mypy_check(snippets: list[str], cache_dir: Path, version: str) -> dict[str, list[str]]:
    """Errors per snippet under ``mypy --strict`` (empty list means it passes).

    Uncached snippets are checked in one mypy run, each as its own module, in
    a scratch directory with an empty config so no project settings leak in.
    """
    results: dict[str, list[str]] = {}
    todo: dict[str, str] = {}
    for text in dict.fromkeys(snippets):
        key = content_hash(version, " ".join(MYPY_FLAGS), text)
        path = cache_dir / key[:2] / f"{key}.json"
        if path.exists():
            results[text] = json.loads(path.read_text())
        else:
            todo[f"m_{key[:20]}"] = text
    if todo:
        with tempfile.TemporaryDirectory(prefix="verdict-v2-mypy-") as tmp:
            root = Path(tmp)
            (root / "mypy.ini").write_text("[mypy]\n")
            for module, text in todo.items():
                (root / f"{module}.py").write_text(text)
            proc = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "mypy",
                    *MYPY_FLAGS,
                    "--config-file",
                    "mypy.ini",
                    "--cache-dir",
                    str(root / ".cache"),
                    *[f"{m}.py" for m in todo],
                ],
                cwd=root,
                capture_output=True,
                text=True,
                timeout=900,
                check=False,
            )
        if proc.returncode not in (0, 1):
            raise RuntimeError(f"mypy crashed: {proc.stderr[-2000:]}")
        errors: dict[str, list[str]] = {m: [] for m in todo}
        for line in proc.stdout.splitlines():
            module, _, rest = line.partition(".py:")
            if module in errors and ": error:" in rest:
                errors[module].append(rest.split(": error:", 1)[1].strip())
        for module, text in todo.items():
            key = content_hash(version, " ".join(MYPY_FLAGS), text)
            path = cache_dir / key[:2] / f"{key}.json"
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps(errors[module]))
            results[text] = errors[module]
    return results


def _count_optional(text: str) -> int:
    return len(re.findall(r"\bOptional\b|\bNone\b", text))


def _marked(text: str) -> bool:
    return "cast(" in text or "# type:" in text


def pair_shortcuts(a: str, b: str) -> dict[str, str]:
    shorter = "B" if len(b) < len(a) else "A"
    fewer = "B" if _count_optional(b) < _count_optional(a) else "A"
    marked = "B" if _marked(b) and not _marked(a) else "A"
    return {"shorter": shorter, "fewer_optional": fewer, "cast_or_ignore": marked}


@dataclass(frozen=True)
class Pair:
    template: str
    passing: str
    failing: str
    logical: tuple[str, tuple[str, ...], tuple[str, ...]]
    error: str


def _pair_pools(ctx: BuildContext, family: str, version: str) -> dict[str, list[Pair]]:
    """Every pair of variants one edit apart where exactly one passes mypy."""
    rendered: list[tuple[str, int, tuple[str, ...], str]] = []
    for name in sorted(TEMPLATES):
        template = TEMPLATES[name]
        for k in range(NAME_VARIANTS):
            values = names(random.Random(f"{ctx.seed}:{family}:{name}:{k}"))
            for variant in variants(template):
                text = render(template, variant, values)
                if text is not None:
                    rendered.append((name, k, tuple(sorted(variant)), text))
    checked = mypy_check([r[3] for r in rendered], cache_root(ctx) / family, version)
    by_group: dict[tuple[str, int], dict[tuple[str, ...], str]] = {}
    for name, k, variant, text in rendered:
        by_group.setdefault((name, k), {})[variant] = text
    pools: dict[str, list[Pair]] = {}
    for (name, _k), group in sorted(by_group.items()):
        keys = sorted(group)
        for i, va in enumerate(keys):
            for vb in keys[i + 1 :]:
                ea, eb = checked[group[va]], checked[group[vb]]
                if len(set(va) ^ set(vb)) != 1 or bool(ea) == bool(eb):
                    continue
                passing, failing = (group[va], group[vb]) if not ea else (group[vb], group[va])
                pools.setdefault(name, []).append(
                    Pair(name, passing, failing, (name, va, vb), (eb or ea)[0])
                )
    return pools


def build_type_check_pair(ctx: BuildContext) -> list[Item]:
    family = "type-check-pair"
    version = mypy_version()
    pools = _pair_pools(ctx, family, version)
    rng = random.Random(f"{ctx.seed}:{family}")
    for pool in pools.values():
        rng.shuffle(pool)
    units = sorted(pools)
    balancer = ChoiceBalancer()
    used_logical: set[tuple[str, tuple[str, ...], tuple[str, ...]]] = set()
    items: list[Item] = []
    while len(items) < ctx.per_family and any(pools[u] for u in units):
        for unit in units:
            if len(items) >= ctx.per_family or not pools[unit]:
                continue
            fresh = [p for p in pools[unit] if p.logical not in used_logical] or pools[unit]
            configs = []
            for pair in fresh[:24]:
                for passing_label in ("A", "B"):
                    a, b = (
                        (pair.passing, pair.failing)
                        if passing_label == "A"
                        else (pair.failing, pair.passing)
                    )
                    configs.append(
                        ChoiceConfig(
                            {"A": a, "B": b}, passing_label, pair_shortcuts(a, b), payload=pair
                        )
                    )
            chosen = balancer.choose(configs)
            pair = chosen.payload
            assert isinstance(pair, Pair)
            pools[unit].remove(pair)
            used_logical.add(pair.logical)
            a, b = chosen.texts["A"], chosen.texts["B"]
            state = (
                "Two versions of the same Python module differ by one small edit. Exactly one of "
                f"them passes `mypy --strict` ({version}, --python-version 3.12) with no errors; "
                "the other reports at least one error.\n\n"
                f"Snippet A:\n```python\n{a}```\n\nSnippet B:\n```python\n{b}```\n"
            )
            items.append(
                make_item(
                    family=family,
                    key=f"{ctx.seed}:{a}:{b}",
                    source_unit=f"{family}:{unit}",
                    state=state,
                    question=choice_question(
                        "Which snippet passes mypy --strict with no errors?",
                        {"A": "Snippet A", "B": "Snippet B"},
                    ),
                    answer=chosen.correct,
                    reference="tool",
                    shortcuts=chosen.shortcuts,
                    source={
                        "template": unit,
                        "variants": [list(pair.logical[1]), list(pair.logical[2])],
                        "failing_error": pair.error,
                        "checker": version,
                    },
                )
            )
    return items
