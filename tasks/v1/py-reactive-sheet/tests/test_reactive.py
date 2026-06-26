"""A reactive sheet: recomputation must propagate transitively and in order.

The work is split across two files: ``reactive/graph.py`` (dependency tracking +
topological ordering + cycle detection) and ``reactive/sheet.py`` (cell values,
formulas, and recomputation driven by the graph). Both must be implemented and
kept consistent.
"""

import pytest

from reactive import CycleError, DependencyGraph, Sheet


def test_construct():
    # passes pre-fix: only construction, no method calls
    assert isinstance(Sheet(), Sheet)
    assert isinstance(DependencyGraph(), DependencyGraph)


def test_basic_formula():
    s = Sheet()
    s.set_value("a", 2)
    s.set_formula("b", ["a"], lambda v: v["a"] * 10)
    assert s.get("b") == 20


def test_transitive_update():
    s = Sheet()
    s.set_value("a", 2)
    s.set_formula("b", ["a"], lambda v: v["a"] * 10)
    s.set_formula("c", ["b"], lambda v: v["b"] + 1)
    assert s.get("c") == 21
    s.set_value("a", 3)
    assert s.get("b") == 30
    assert s.get("c") == 31  # transitive recompute reached c


def test_topological_order_diamond():
    s = Sheet()
    s.set_value("a", 2)
    s.set_formula("b", ["a"], lambda v: v["a"] + 1)
    s.set_formula("c", ["a"], lambda v: v["a"] * 2)
    s.set_formula("d", ["b", "c"], lambda v: v["b"] + v["c"])  # depends on both
    assert s.get("d") == 7
    s.set_value("a", 10)
    assert (s.get("b"), s.get("c"), s.get("d")) == (11, 20, 31)  # d recomputed after b and c


def test_formula_rebind_clears_old_deps():
    s = Sheet()
    s.set_value("a", 1)
    s.set_value("x", 100)
    s.set_formula("b", ["a"], lambda v: v["a"])
    assert s.get("b") == 1
    s.set_formula("b", ["x"], lambda v: v["x"])  # rebind b to depend on x, not a
    assert s.get("b") == 100
    s.set_value("a", 999)
    assert s.get("b") == 100  # a no longer affects b
    s.set_value("x", 7)
    assert s.get("b") == 7  # new dependency is live


def test_cycle_detection():
    s = Sheet()
    s.set_value("a", 1)
    s.set_formula("b", ["a"], lambda v: v["a"])
    with pytest.raises(CycleError):
        s.set_formula("a", ["b"], lambda v: v["b"])  # a <-> b cycle
    with pytest.raises(CycleError):
        Sheet().set_formula("z", ["z"], lambda v: v["z"])  # self cycle


def test_get_unknown_raises():
    with pytest.raises(KeyError):
        Sheet().get("missing")
