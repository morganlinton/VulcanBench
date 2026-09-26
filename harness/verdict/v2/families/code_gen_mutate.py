"""Single-edit mutations for Python (AST guided) and JavaScript (token based).

Python mutants are produced as text-span edits located by the AST, so the rest
of the file stays byte-identical: diffs are one small hunk and line numbers
outside the edit do not move. Every mutant is re-parsed; ones that do not
parse, or parse to the same tree as the original, are dropped.
"""

from __future__ import annotations

import ast
import re
from dataclasses import dataclass

BINOP_SYMBOL: dict[type[ast.operator], str] = {
    ast.Add: "+",
    ast.Sub: "-",
    ast.Mult: "*",
    ast.Div: "/",
    ast.FloorDiv: "//",
    ast.Mod: "%",
    ast.Pow: "**",
    ast.LShift: "<<",
    ast.RShift: ">>",
    ast.BitAnd: "&",
    ast.BitOr: "|",
    ast.BitXor: "^",
}
BINOP_SWAPS: dict[str, tuple[str, ...]] = {
    "+": ("-",),
    "-": ("+",),
    "*": ("+", "//"),
    "/": ("//", "*"),
    "//": ("/", "%"),
    "%": ("//",),
    "**": ("*",),
    "<<": (">>",),
    ">>": ("<<",),
    "&": ("|",),
    "|": ("&",),
    "^": ("|",),
}
CMP_SYMBOL: dict[type[ast.cmpop], str] = {
    ast.Lt: "<",
    ast.LtE: "<=",
    ast.Gt: ">",
    ast.GtE: ">=",
    ast.Eq: "==",
    ast.NotEq: "!=",
    ast.Is: "is",
    ast.IsNot: "is not",
    ast.In: "in",
    ast.NotIn: "not in",
}
CMP_SWAPS: dict[str, tuple[str, ...]] = {
    "<": ("<=", ">"),
    "<=": ("<", ">="),
    ">": (">=", "<"),
    ">=": (">", "<="),
    "==": ("!=",),
    "!=": ("==",),
    "is": ("is not",),
    "is not": ("is",),
    "in": ("not in",),
    "not in": ("in",),
}
NAME_CALL_SWAPS = {
    "min": "max",
    "max": "min",
    "any": "all",
    "all": "any",
    "round": "int",
    "sorted": "reversed",
    "reversed": "sorted",
}
ATTR_CALL_SWAPS = {
    "append": "extend",
    "extend": "append",
    "strip": "rstrip",
    "lstrip": "strip",
    "rstrip": "strip",
    "startswith": "endswith",
    "endswith": "startswith",
    "upper": "lower",
    "lower": "upper",
    "find": "rfind",
    "rfind": "find",
    "index": "find",
    "split": "rsplit",
    "rsplit": "split",
    "keys": "values",
    "values": "keys",
    "add": "discard",
    "remove": "discard",
    "popleft": "pop",
    "appendleft": "append",
    "pop": "popitem",
    "insert": "__setitem__",
    "update": "setdefault",
    "issubset": "issuperset",
    "issuperset": "issubset",
    "union": "intersection",
    "intersection": "union",
}
COPY_CALLS = {"list", "dict", "set", "tuple", "sorted"}


@dataclass(frozen=True)
class Mutant:
    """One single-edit mutant: the edit, where it is, and the new source."""

    kind: str
    line: int
    before: str
    after: str
    source: str


@dataclass(frozen=True)
class _Edit:
    kind: str
    line: int
    col: int
    end_line: int
    end_col: int
    text: str


class _Source:
    """Maps AST (line, UTF-8 byte column) positions to string offsets."""

    def __init__(self, text: str) -> None:
        self.text = text
        self.lines = text.splitlines(keepends=True)
        self.starts: list[int] = []
        pos = 0
        for line in self.lines:
            self.starts.append(pos)
            pos += len(line)
        self.starts.append(pos)

    def offset(self, line: int, col: int) -> int:
        raw = self.lines[line - 1].encode()[:col].decode(errors="replace")
        return self.starts[line - 1] + len(raw)

    def segment(self, line: int, col: int, end_line: int, end_col: int) -> str:
        return self.text[self.offset(line, col) : self.offset(end_line, end_col)]

    def node(self, node: ast.AST) -> str:
        return self.segment(
            node.lineno,  # type: ignore[attr-defined]
            node.col_offset,  # type: ignore[attr-defined]
            node.end_lineno,  # type: ignore[attr-defined]
            node.end_col_offset,  # type: ignore[attr-defined]
        )

    def apply(self, edit: _Edit) -> str:
        start = self.offset(edit.line, edit.col)
        end = self.offset(edit.end_line, edit.end_col)
        return self.text[:start] + edit.text + self.text[end:]


def _pos(node: ast.AST) -> tuple[int, int, int, int]:
    return (
        node.lineno,  # type: ignore[attr-defined]
        node.col_offset,  # type: ignore[attr-defined]
        node.end_lineno,  # type: ignore[attr-defined]
        node.end_col_offset,  # type: ignore[attr-defined]
    )


def _gap_edits(
    src: _Source, left: ast.AST, right: ast.AST, symbol: str, swaps: tuple[str, ...], kind: str
) -> list[_Edit]:
    """Swap an operator token that sits between two sub-expressions."""
    _, _, l_end_line, l_end_col = _pos(left)
    r_line, r_col, _, _ = _pos(right)
    gap = src.segment(l_end_line, l_end_col, r_line, r_col)
    core = gap.replace("\\\n", " ").strip().strip("()").strip()
    if " ".join(core.split()) != symbol:
        return []
    pattern = r"\s+".join(re.escape(word) for word in symbol.split())
    match = re.search(pattern, gap)
    if match is None:
        return []
    return [
        _Edit(
            kind,
            l_end_line,
            l_end_col,
            r_line,
            r_col,
            gap[: match.start()] + new + gap[match.end() :],
        )
        for new in swaps
    ]


def _edits_for(node: ast.AST, src: _Source, allow_delete: bool) -> list[_Edit]:  # noqa: PLR0912, dispatch on node type
    edits: list[_Edit] = []
    if isinstance(node, ast.BinOp):
        symbol = BINOP_SYMBOL.get(type(node.op))
        if symbol:
            edits += _gap_edits(src, node.left, node.right, symbol, BINOP_SWAPS[symbol], "binop")
    elif isinstance(node, ast.AugAssign):
        symbol = BINOP_SYMBOL.get(type(node.op))
        if symbol:
            edits += _gap_edits(
                src,
                node.target,
                node.value,
                symbol + "=",
                tuple(s + "=" for s in BINOP_SWAPS[symbol]),
                "augassign",
            )
    elif isinstance(node, ast.Compare):
        prev: ast.AST = node.left
        for op, right in zip(node.ops, node.comparators, strict=True):
            symbol = CMP_SYMBOL[type(op)]
            edits += _gap_edits(src, prev, right, symbol, CMP_SWAPS[symbol], "compare")
            prev = right
    elif isinstance(node, ast.BoolOp):
        symbol = "and" if isinstance(node.op, ast.And) else "or"
        other = "or" if symbol == "and" else "and"
        for left, right in zip(node.values, node.values[1:], strict=False):
            edits += _gap_edits(src, left, right, symbol, (other,), "boolop")
    elif isinstance(node, ast.Constant):
        value = node.value
        line, col, end_line, end_col = _pos(node)
        if isinstance(value, bool):
            edits.append(_Edit("constant", line, col, end_line, end_col, str(not value)))
        elif isinstance(value, int) and src.node(node).strip().isdigit():
            for new in (value + 1, value - 1):
                if new >= 0:
                    edits.append(_Edit("constant", line, col, end_line, end_col, str(new)))
    elif isinstance(node, ast.UnaryOp) and isinstance(node.op, (ast.Not, ast.USub)):
        line, col, end_line, end_col = _pos(node)
        edits.append(_Edit("unary", line, col, end_line, end_col, src.node(node.operand)))
    elif isinstance(node, ast.If):
        line, col, end_line, end_col = _pos(node.test)
        text = src.node(node.test)
        edits.append(_Edit("negate-if", line, col, end_line, end_col, f"not ({text})"))
    elif isinstance(node, ast.Break):
        edits.append(_Edit("break", *_pos(node), "continue"))
    elif isinstance(node, ast.Continue):
        edits.append(_Edit("continue", *_pos(node), "break"))
    elif isinstance(node, ast.Subscript) and isinstance(node.slice, ast.Slice):
        edits += _slice_edits(node, src)
    elif isinstance(node, ast.Call):
        edits += _call_edits(node, src)
    elif allow_delete and isinstance(node, ast.Expr):
        if not isinstance(node.value, ast.Constant):
            edits.append(_Edit("delete", *_pos(node), "pass"))
    if allow_delete and isinstance(node, ast.AugAssign):
        edits.append(_Edit("delete", *_pos(node), "pass"))
    return edits


def _slice_edits(node: ast.Subscript, src: _Source) -> list[_Edit]:
    sl = node.slice
    assert isinstance(sl, ast.Slice)
    parts = [src.node(p) if p is not None else None for p in (sl.lower, sl.upper, sl.step)]
    lower, upper, step = parts
    variants: list[tuple[str | None, str | None, str | None]] = []
    if lower is None and upper is None and step is None:
        return [_Edit("slice-copy", *_pos(node), src.node(node.value))]
    if lower is not None:
        variants.append((None, upper, step))
    if upper is not None:
        variants.append((lower, None, step))
    if step is not None:
        variants.append((lower, upper, None))
    elif lower is not None or upper is not None:
        variants.append((upper, lower, None))
    edits = []
    for lo, up, st in variants:
        text = f"{lo or ''}:{up or ''}" + (f":{st}" if st is not None else "")
        edits.append(_Edit("slice", *_pos(sl), text))
    return edits


def _call_edits(node: ast.Call, src: _Source) -> list[_Edit]:
    edits: list[_Edit] = []
    func = node.func
    if isinstance(func, ast.Name):
        if func.id in NAME_CALL_SWAPS and node.args:
            edits.append(_Edit("call-swap", *_pos(func), NAME_CALL_SWAPS[func.id]))
        if func.id in COPY_CALLS and len(node.args) == 1 and not node.keywords:
            edits.append(_Edit("drop-copy", *_pos(node), src.node(node.args[0])))
        if func.id == "range" and node.args:
            last = node.args[-1]
            text = src.node(last)
            wrapped = (
                text if isinstance(last, (ast.Name, ast.Constant, ast.Attribute)) else f"({text})"
            )
            if not isinstance(last, ast.Constant):
                edits.append(_Edit("range", *_pos(last), f"{wrapped} + 1"))
                edits.append(_Edit("range", *_pos(last), f"{wrapped} - 1"))
    elif isinstance(func, ast.Attribute):
        _, _, end_line, end_col = _pos(func)
        if func.attr in ATTR_CALL_SWAPS:
            new = ATTR_CALL_SWAPS[func.attr]
            edits.append(
                _Edit("method-swap", end_line, end_col - len(func.attr), end_line, end_col, new)
            )
        if func.attr == "copy" and not node.args:
            edits.append(_Edit("drop-copy", *_pos(node), src.node(func.value)))
    return edits


def _dump(tree: ast.AST) -> str:
    return ast.dump(tree, include_attributes=False)


def mutants(  # noqa: PLR0912
    source: str,
    region: tuple[int, int] | None = None,
    allow_delete: bool = False,
    skip_lines: frozenset[int] = frozenset(),
    skip_data: bool = False,
) -> list[Mutant]:
    """Every distinct single-edit mutant, in a stable order.

    ``region`` limits edits to an inclusive line range (a function body).
    ``skip_lines`` excludes lines (docstrings, decorators). ``skip_data``
    leaves literal elements of list, tuple, set and dict displays alone, so a
    mutant changes what the code does rather than the data it is given.
    """
    tree = ast.parse(source)
    skip_ids: set[int] = set()
    if skip_data:
        for node in ast.walk(tree):
            if isinstance(node, (ast.List, ast.Tuple, ast.Set, ast.Dict)):
                elements = node.values if isinstance(node, ast.Dict) else node.elts
                keys = [k for k in node.keys if k is not None] if isinstance(node, ast.Dict) else []
                for element in [*elements, *keys]:
                    for sub in ast.walk(element):
                        if isinstance(sub, (ast.Constant, ast.UnaryOp)):
                            skip_ids.add(id(sub))
    top_dumps: dict[int, str] = {}

    def same_tree(new_tree: ast.Module, line: int) -> bool:
        # Only the top-level statement holding the edit can differ, so compare
        # that one (whole-module dumps are slow on large library files).
        if len(new_tree.body) != len(tree.body):
            return False
        for i, (old, new) in enumerate(zip(tree.body, new_tree.body, strict=True)):
            if old.lineno <= line <= (old.end_lineno or old.lineno):
                if i not in top_dumps:
                    top_dumps[i] = _dump(old)
                return top_dumps[i] == _dump(new)
        return _dump(new_tree) == _dump(tree)

    src = _Source(source)
    seen: set[str] = set()
    out: list[Mutant] = []
    for node in ast.walk(tree):
        if not hasattr(node, "lineno"):
            continue
        line = node.lineno
        if region and not region[0] <= line <= region[1]:
            continue
        if line in skip_lines or id(node) in skip_ids:
            continue
        for edit in _edits_for(node, src, allow_delete):
            new_source = src.apply(edit)
            if new_source == source or new_source in seen:
                continue
            try:
                new_tree = ast.parse(new_source)
            except SyntaxError:
                continue
            if same_tree(new_tree, edit.line):
                continue
            seen.add(new_source)
            before = src.lines[edit.line - 1].rstrip("\n")
            after_lines = new_source.splitlines()
            after = after_lines[edit.line - 1] if edit.line - 1 < len(after_lines) else ""
            out.append(Mutant(edit.kind, edit.line, before, after, new_source))
    return out


# JavaScript: a light tokenizer is enough for the short generated programs.
_JS_TOKEN = re.compile(
    r"""(?P<ws>\s+)
    |(?P<comment>//[^\n]*|/\*.*?\*/)
    |(?P<string>`(?:\\.|[^`\\])*`|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*')
    |(?P<number>\d+(?:\.\d+)?)
    |(?P<ident>[A-Za-z_$][A-Za-z0-9_$]*)
    |(?P<op>===|!==|\+\+|--|&&|\|\||<=|>=|=>|==|!=|\+=|-=|\*=|[-+*/%<>=!?:;,.(){}\[\]])
    |(?P<other>.)""",
    re.VERBOSE | re.DOTALL,
)
JS_OP_SWAPS: dict[str, tuple[str, ...]] = {
    "<": ("<=",),
    "<=": ("<",),
    ">": (">=",),
    ">=": (">",),
    "===": ("!==",),
    "!==": ("===",),
    "==": ("===",),
    "+": ("-",),
    "-": ("+",),
    "*": ("+",),
    "/": ("*",),
    "%": ("/",),
    "&&": ("||",),
    "||": ("&&",),
    "++": ("--",),
    "+=": ("-=",),
    "-=": ("+=",),
}
JS_IDENT_SWAPS: dict[str, tuple[str, ...]] = {
    "let": ("var",),
    "var": ("let",),
    "floor": ("round", "ceil"),
    "round": ("floor",),
    "ceil": ("floor",),
    "slice": ("splice",),
    "splice": ("slice",),
    "push": ("unshift",),
    "unshift": ("push",),
    "shift": ("pop",),
    "pop": ("shift",),
    "true": ("false",),
    "false": ("true",),
    "min": ("max",),
    "max": ("min",),
    "indexOf": ("lastIndexOf",),
    "lastIndexOf": ("indexOf",),
    "some": ("every",),
    "every": ("some",),
    "trim": ("trimStart",),
    "toUpperCase": ("toLowerCase",),
    "toLowerCase": ("toUpperCase",),
}


def js_mutants(source: str) -> list[str]:
    """Distinct single-token JavaScript mutants, in source order."""
    tokens = [(m.lastgroup or "other", m.group()) for m in _JS_TOKEN.finditer(source)]
    out: list[str] = []
    seen = {source}
    for i, (kind, text) in enumerate(tokens):
        alternatives: tuple[str, ...] = ()
        if kind == "op":
            alternatives = JS_OP_SWAPS.get(text, ())
        elif kind == "ident":
            alternatives = JS_IDENT_SWAPS.get(text, ())
        elif kind == "number" and text.isdigit():
            value = int(text)
            alternatives = tuple(str(v) for v in (value + 1, value - 1) if v >= 0)
        for new in alternatives:
            mutated = (
                "".join(t for _, t in tokens[:i]) + new + "".join(t for _, t in tokens[i + 1 :])
            )
            if mutated not in seen:
                seen.add(mutated)
                out.append(mutated)
    return out
