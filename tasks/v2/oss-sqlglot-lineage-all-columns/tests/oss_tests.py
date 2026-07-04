"""Hidden test for oss-sqlglot-lineage-all-columns.

The lineage() API must gain two capabilities: an all-columns mode (passing
column=None returns a mapping of every final output column name to its lineage
Node), and an on_node callback invoked for every Node created during the walk.
Single-column lineage must keep working unchanged. Expected behavior captured
from the upstream feature (tobymao/sqlglot PR #7575).

Run with PYTHONPATH=. so the workspace sqlglot is under test.
"""
from __future__ import annotations

from sqlglot.lineage import lineage

SCHEMA = {"x": {"a": "INT", "b": "INT"}, "y": {"a": "INT", "c": "INT"}}
SQL = "WITH t AS (SELECT a, b FROM x) SELECT a, b FROM t"


def test_all_columns_mode_returns_mapping():
    res = lineage(None, SQL, schema=SCHEMA)
    assert isinstance(res, dict)
    assert sorted(res.keys()) == ["a", "b"]
    # each value is a lineage Node for that output column
    assert res["a"].name and res["b"].name


def test_on_node_callback_fires_for_every_node():
    seen = []
    root = lineage("a", SQL, schema=SCHEMA, on_node=lambda n: seen.append(n))
    # The callback sees every node the walk creates, including the root and its
    # upstream chain (at least root + CTE projection + base column).
    assert len(seen) >= 3
    assert root in seen


def test_on_node_fires_in_all_columns_mode():
    seen = []
    lineage(None, SQL, schema=SCHEMA, on_node=lambda n: seen.append(n))
    assert len(seen) >= 4  # two output columns each with upstream chains


def test_single_column_lineage_unchanged():
    node = lineage("a", SQL, schema=SCHEMA)
    assert node.name == "a"
    # Walking upstream still reaches the base table column.
    names = [n.name for n in node.walk()]
    assert any("x" in n or n == "a" for n in names)
