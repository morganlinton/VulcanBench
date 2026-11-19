"""Tests for the ``UserStatement`` pass: the user statement restricting the root rows."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, cast

import pytest
from inline_snapshot import snapshot
from sqlalchemy import delete, select
from strawberry.types import get_object_definition

from strawchemy.transpiler import Transpiler
from tests.unit.models import Color
from tests.unit.schemas.optimizations import ColorFilter
from tests.unit.transpiler.passes.utils import plan_sql
from tests.unit.utils import SQLA_DIALECTS
from tests.utils import format_sql

if TYPE_CHECKING:
    from strawchemy.dto.strawberry import BooleanFilterDTO

_DIALECTS = pytest.mark.parametrize("dialect_name", ["postgresql", "sqlite", "mysql"])


@_DIALECTS
def test_user_statement_where_only_inlined(dialect_name: str) -> None:
    """A user statement adding only a WHERE has it copied onto the root alias, with no join or subquery."""
    lines = plan_sql("{ colorsNamedRed { name } }", dialect_name)

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " WHERE color.name = %(name_1)s",
                    " ORDER BY color.id ASC",
                ],
                "sqlite": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " WHERE color.name = ?",
                    " ORDER BY color.id ASC",
                ],
                "mysql": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " WHERE color.name = %s",
                    " ORDER BY color.id ASC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_user_statement_with_join_uses_pk_join(dialect_name: str) -> None:
    """A user statement with a join is joined to the root on its primary key, as a subquery named ``user_statement``."""
    lines = plan_sql("{ colorsWithSweetFruits { name } }", dialect_name)

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    "  JOIN (",
                    "        SELECT color.id AS id",
                    "          FROM color",
                    "          JOIN fruit",
                    "            ON color.id = fruit.color_id",
                    "         WHERE fruit.sweetness > %(sweetness_1)s",
                    "       ) AS user_statement",
                    "    ON color.id = user_statement.id",
                    " ORDER BY color.id ASC",
                ],
                "sqlite": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    "  JOIN (",
                    "        SELECT color.id AS id",
                    "          FROM color",
                    "          JOIN fruit",
                    "            ON color.id = fruit.color_id",
                    "         WHERE fruit.sweetness > ?",
                    "       ) AS user_statement",
                    "    ON color.id = user_statement.id",
                    " ORDER BY color.id ASC",
                ],
                "mysql": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " INNER JOIN (",
                    "        SELECT color.id AS id",
                    "          FROM color",
                    "         INNER JOIN fruit",
                    "            ON color.id = fruit.color_id",
                    "         WHERE fruit.sweetness > %s",
                    "       ) AS user_statement",
                    "    ON color.id = user_statement.id",
                    " ORDER BY color.id ASC",
                ],
            }
        )[dialect_name]
    )


def _name_filter() -> BooleanFilterDTO:
    field = next(
        field for field in get_object_definition(ColorFilter, strict=True).fields if field.python_name == "name"
    )
    text_comparison = cast("type[Any]", getattr(field.type, "of_type", field.type))
    return ColorFilter(name=text_comparison(neq="zzz"))  # ty: ignore[unknown-argument]  # input fields are generated at runtime


@_DIALECTS
def test_filter_expressions_add_where_only_statement(dialect_name: str) -> None:
    """A WHERE-only user statement adds its WHERE, on the real table, to the DML predicates of the filter."""
    dialect = SQLA_DIALECTS[dialect_name]
    transpiler = Transpiler(Color, dialect, statement=select(Color).where(Color.id == 2))

    statement = delete(Color).where(*transpiler.filter_expressions(_name_filter()))

    sql = format_sql(str(statement.compile(dialect=dialect, compile_kwargs={"literal_binds": True})))
    assert (
        sql.splitlines()
        == snapshot(
            {
                "postgresql": ["DELETE", "  FROM color", " WHERE color.id = 2", "   AND color.name != 'zzz'"],
                "sqlite": ["DELETE", "  FROM color", " WHERE color.id = 2", "   AND color.name != 'zzz'"],
                "mysql": ["DELETE", "  FROM color", " WHERE color.id = 2", "   AND color.name != 'zzz'"],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_filter_expressions_ignore_statement_with_join(dialect_name: str) -> None:
    """A user statement with a join adds nothing to the DML predicates, which join no table."""
    dialect = SQLA_DIALECTS[dialect_name]
    statement = select(Color).join(Color.fruits).where(Color.id == 2)
    transpiler = Transpiler(Color, dialect, statement=statement)

    expressions = transpiler.filter_expressions(_name_filter())

    assert len(expressions) == 1


@_DIALECTS
def test_filter_expressions_ignore_statement_without_where(dialect_name: str) -> None:
    """A user statement that is the bare model select adds nothing to the DML predicates of the filter."""
    dialect = SQLA_DIALECTS[dialect_name]
    transpiler = Transpiler(Color, dialect, statement=select(Color))

    statement = delete(Color).where(*transpiler.filter_expressions(_name_filter()))

    sql = format_sql(str(statement.compile(dialect=dialect, compile_kwargs={"literal_binds": True})))
    assert (
        sql.splitlines()
        == snapshot(
            {
                "postgresql": ["DELETE", "  FROM color", " WHERE color.name != 'zzz'"],
                "sqlite": ["DELETE", "  FROM color", " WHERE color.name != 'zzz'"],
                "mysql": ["DELETE", "  FROM color", " WHERE color.name != 'zzz'"],
            }
        )[dialect_name]
    )
