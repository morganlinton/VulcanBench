"""Hidden test for oss-sqlglot-resolver-flatten-recursion.

The optimizer's qualify/resolver step must resolve the columns of a LATERAL table
function (e.g. Snowflake ``LATERAL FLATTEN``) whose source table is absent from the
schema passed to ``qualify``. When the source table is missing, column resolution must
fail with a clean ``OptimizeError`` instead of recursing infinitely. A valid lateral
query whose columns can be resolved must still qualify correctly. Expected behavior
captured from the upstream fix (sqlglot PR #7737).
"""
import pytest
import sqlglot
from sqlglot.optimizer.qualify import qualify
from sqlglot.errors import OptimizeError


def test_lateral_flatten_missing_table_raises_optimize_error():
    # my_db.raw.events is referenced by the query but is not in the schema. Resolving the
    # unqualified column inside LATERAL FLATTEN(items) must raise OptimizeError rather than
    # recursing forever (which surfaces as a RecursionError on the buggy build).
    expr = sqlglot.parse_one(
        "SELECT f.value AS v FROM my_db.raw.events, LATERAL FLATTEN(items) AS f",
        read="snowflake",
    )
    with pytest.raises(OptimizeError):
        qualify(
            expr,
            schema={"my_db": {"other": {"some_view": {"v": "VARIANT"}}}},
            dialect="snowflake",
        )


def test_lateral_struct_field_resolves():
    # pass_to_pass anchor: an unqualified struct field is disambiguated through the
    # lateral's extended columns and must qualify to the lateral alias.
    got = qualify(
        sqlglot.parse_one(
            "SELECT name FROM my_table LATERAL VIEW EXPLODE(items) ci AS ci",
            read="spark",
        ),
        schema={"my_table": {"items": "ARRAY<STRUCT<name STRING, age INT>>"}},
        dialect="spark",
    ).sql(dialect="spark")
    assert got == (
        "SELECT `ci`.`name` AS `name` FROM `my_table` AS `my_table` "
        "LATERAL VIEW EXPLODE(`my_table`.`items`) ci AS `ci`"
    )
