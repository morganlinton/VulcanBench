"""Tiny expression AST shared by the parser and the evaluator."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class Num:
    value: float | int


@dataclass(frozen=True)
class UnaryOp:
    op: str  # "+" or "-"
    operand: Node


@dataclass(frozen=True)
class BinOp:
    op: str  # one of: + - * / // % **
    left: Node
    right: Node


Node = Num | UnaryOp | BinOp
