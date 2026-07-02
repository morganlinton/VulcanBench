"""Hidden test for oss-sqlglot-star-ilike-rename.

Snowflake allows filtering a star projection by column-name pattern
(``SELECT * ILIKE 'a%'``) and combining that filter with ``RENAME``
(``SELECT * ILIKE 'a%' RENAME (aa AS d)``). The parser must accept the
combination, it must round-trip through the generator, and the optimizer's
qualify step must expand the star to exactly the columns whose names match
the pattern (case-insensitive), applying any renames. Expected behavior
captured from the upstream fix (tobymao/sqlglot PR #7720).
"""
import sqlglot
from sqlglot.optimizer.qualify import qualify

SCHEMA = {"x": {"aa": "INT", "ab": "INT", "b": "INT"}}


def _qualified(sql: str) -> str:
    expr = sqlglot.parse_one(sql, read="snowflake")
    return qualify(expr, schema=SCHEMA, dialect="snowflake").sql("snowflake")


def test_star_ilike_rename_parses_and_qualifies():
    sql = "SELECT * ILIKE 'a%' RENAME(aa AS d) FROM x"
    expr = sqlglot.parse_one(sql, read="snowflake")
    # Round-trips through the generator.
    assert expr.sql("snowflake") == "SELECT * ILIKE 'a%' RENAME (aa AS d) FROM x"
    # Star expands to the pattern-matching columns with the rename applied.
    assert _qualified(sql) == 'SELECT "X"."AA" AS "D", "X"."AB" AS "AB" FROM "X" AS "X"'


def test_star_ilike_qualifies_to_matching_columns():
    # The ILIKE filter selects columns by name pattern; it is not a boolean
    # expression on values.
    sql = "SELECT * ILIKE 'a%' FROM x"
    assert _qualified(sql) == 'SELECT "X"."AA" AS "AA", "X"."AB" AS "AB" FROM "X" AS "X"'


def test_star_rename_without_ilike_stable():
    schema = {"x": {"a": "INT", "b": "INT"}}
    expr = sqlglot.parse_one("SELECT * RENAME(a AS d) FROM x", read="snowflake")
    out = qualify(expr, schema=schema, dialect="snowflake").sql("snowflake")
    assert out == 'SELECT "X"."A" AS "D", "X"."B" AS "B" FROM "X" AS "X"'
