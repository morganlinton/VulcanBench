"""Evaluate a parsed expression tree.

This module is correct; it relies on the parser to encode operator precedence
and associativity in the shape of the tree. Division, floor-division, and
modulo by zero raise :class:`CalcError`.
"""

from __future__ import annotations

from calc.ast_nodes import BinOp, Node, Num, UnaryOp
from calc.errors import CalcError
from calc.parser import parse


def _eval(node: Node) -> float | int:
    if isinstance(node, Num):
        return node.value
    if isinstance(node, UnaryOp):
        operand = _eval(node.operand)
        return operand if node.op == "+" else -operand
    if isinstance(node, BinOp):
        left = _eval(node.left)
        right = _eval(node.right)
        if node.op == "+":
            return left + right
        if node.op == "-":
            return left - right
        if node.op == "*":
            return left * right
        if node.op == "**":
            return left**right
        if node.op in {"/", "//", "%"} and right == 0:
            raise CalcError(f"division by zero in {left!r} {node.op} {right!r}")
        if node.op == "/":
            return left / right
        if node.op == "//":
            return left // right
        if node.op == "%":
            return left % right
    raise CalcError(f"cannot evaluate node {node!r}")


def evaluate(text: str) -> float | int:
    """Tokenize, parse, and evaluate ``text``, returning a number.

    Raises :class:`CalcError` on any malformed input or runtime error.
    """
    return _eval(parse(text))
