"""Hidden test for oss-sqlglot-star-cte-struct-shadow.

In BigQuery a table column can be a STRUCT, and ``column.*`` expands that struct's fields.
When qualifying ``alias.*``, an unreferenced CTE whose name matches a struct column must
NOT shadow that struct column: star expansion has to look at the correct source set. When
the same-named CTE IS selected in the FROM clause, it takes precedence. Expected outputs
captured from the upstream fix (sqlglot PR #7718).
"""
import sqlglot
from sqlglot.optimizer.qualify import qualify

SCHEMA = {"structs": {"one": "STRUCT<a_1 INT, b_1 INT>"}}


def _q(sql: str) -> str:
    return qualify(
        sqlglot.parse_one(sql, read="bigquery"), schema=SCHEMA, dialect="bigquery"
    ).sql(dialect="bigquery")


def test_unreferenced_cte_does_not_shadow_struct_star():
    # `one` is defined as a CTE but NOT selected in FROM, so `one.*` must expand the struct
    # column `structs.one`, not the CTE.
    got = _q("WITH one AS (SELECT 1 AS x) SELECT one.* FROM structs")
    assert got == (
        "WITH `one` AS (SELECT 1 AS `x`) "
        "SELECT `structs`.`one`.`a_1` AS `a_1`, `structs`.`one`.`b_1` AS `b_1` "
        "FROM `structs` AS `structs`"
    )


def test_cte_in_from_takes_precedence():
    # pass_to_pass anchor: when the CTE is actually selected in FROM, `one.*` is the CTE.
    got = _q("WITH one AS (SELECT 1 AS x) SELECT one.* FROM structs, one")
    assert got == (
        "WITH `one` AS (SELECT 1 AS `x`) SELECT `one`.`x` AS `x` "
        "FROM `structs` AS `structs` CROSS JOIN `one` AS `one`"
    )
