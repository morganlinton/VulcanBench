"""Hidden test for oss-sqlglot-unpivot-cte-star.

Qualifying ``SELECT *`` over an ``UNPIVOT`` of a CTE source must expand the star to include
the columns produced by the UNPIVOT (the value column and the name column) alongside the
columns that were not unpivoted, and must NOT qualify the UNPIVOT-generated column names
against the source table. A plain star expansion over a CTE (no UNPIVOT) must be unchanged.
Expected outputs captured from the upstream fix (sqlglot PR #7550).
"""
import sqlglot
from sqlglot.optimizer.qualify import qualify


def _q(sql: str) -> str:
    return qualify(
        sqlglot.parse_one(sql, dialect="bigquery"), dialect="bigquery"
    ).sql(dialect="bigquery")


def test_unpivot_cte_star_expands():
    got = _q(
        "WITH produce AS (SELECT 'Kale' AS product, 51 AS q1, 23 AS q2) "
        "SELECT * FROM produce UNPIVOT(sales FOR quarter IN (q1, q2))"
    )
    assert got == (
        "WITH `produce` AS (SELECT 'Kale' AS `product`, 51 AS `q1`, 23 AS `q2`) "
        "SELECT `produce`.`product` AS `product`, `produce`.`quarter` AS `quarter`, "
        "`produce`.`sales` AS `sales` FROM `produce` AS `produce` "
        "UNPIVOT(`sales` FOR `quarter` IN (`produce`.`q1`, `produce`.`q2`)) AS `produce`"
    )


def test_plain_cte_star_unaffected():
    # pass_to_pass anchor: basic star expansion over a CTE (no UNPIVOT) is unchanged.
    got = _q("WITH produce AS (SELECT 'Kale' AS product, 51 AS q1) SELECT * FROM produce")
    assert got == (
        "WITH `produce` AS (SELECT 'Kale' AS `product`, 51 AS `q1`) "
        "SELECT `produce`.`product` AS `product`, `produce`.`q1` AS `q1` "
        "FROM `produce` AS `produce`"
    )
