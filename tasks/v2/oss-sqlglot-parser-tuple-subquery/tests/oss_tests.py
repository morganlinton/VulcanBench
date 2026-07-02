"""Hidden test for oss-sqlglot-parser-tuple-subquery.

When a parenthesized expression is the FIRST element of a multi-element comma list, it
must parse as a Tuple, not swallow the rest of the list as a Subquery. Behavior captured
from the upstream fix (sqlglot PR #7807).
"""
import sqlglot
from sqlglot import exp, parse_one


def test_parenthesized_subquery_first_in_tuple():
    e = parse_one("((SELECT 42), 1)")
    assert isinstance(e, exp.Tuple), f"expected Tuple, got {type(e).__name__}"
    assert e.sql() == "((SELECT 42), 1)"


def test_plain_tuple_unaffected():
    # pass_to_pass no-regression anchor.
    assert isinstance(parse_one("(1, 2)"), exp.Tuple)
