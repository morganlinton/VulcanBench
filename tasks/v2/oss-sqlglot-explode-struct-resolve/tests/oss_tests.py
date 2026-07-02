"""Hidden test for oss-sqlglot-explode-struct-resolve.

The optimizer's resolver must resolve qualified struct fields (``ci.name``, ``ci.age``) that
are produced by a ``LATERAL VIEW EXPLODE`` of an ``ARRAY<STRUCT<...>>`` column. On the buggy
build those fields cannot be resolved and qualification raises ``OptimizeError``. Expected
output captured from the upstream fix (sqlglot PR #7549).
"""
import sqlglot
from sqlglot.optimizer.qualify import qualify

SCHEMA = {"my_table": {"items": "ARRAY<STRUCT<name STRING, age INT>>"}}


def _q(sql: str) -> str:
    return qualify(
        sqlglot.parse_one(sql, read="spark"), schema=SCHEMA, dialect="spark"
    ).sql(dialect="spark")


def test_explode_struct_fields_resolve():
    got = _q("SELECT ci.name, ci.age FROM my_table LATERAL VIEW EXPLODE(items) ci AS ci")
    assert got == (
        "SELECT `ci`.`name` AS `name`, `ci`.`age` AS `age` "
        "FROM `my_table` AS `my_table` "
        "LATERAL VIEW EXPLODE(`my_table`.`items`) ci AS `ci`"
    )


def test_simple_select_unaffected():
    # pass_to_pass anchor: a plain column select is unchanged.
    got = _q("SELECT items FROM my_table")
    assert got == "SELECT `my_table`.`items` AS `items` FROM `my_table` AS `my_table`"
