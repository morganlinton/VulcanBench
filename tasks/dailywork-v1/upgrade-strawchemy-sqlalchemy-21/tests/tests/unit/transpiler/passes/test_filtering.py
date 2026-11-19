"""Tests for the ``Filtering`` pass: WHERE predicates, the joins they need and the EXISTS they test."""

from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING, Any, cast

import pytest
from inline_snapshot import snapshot
from sqlalchemy import create_engine, select, update
from sqlalchemy.orm import DeclarativeBase, Mapped, Session, mapped_column
from strawberry.types import get_object_definition

from strawchemy import Strawchemy
from strawchemy.transpiler import Transpiler
from tests.duplicate_reads import assert_no_duplicate_reads
from tests.unit.models import Color, Department, Fruit, Group, Tag, User, UUIDBase
from tests.unit.schemas.optimizations import ColorFilter, schema
from tests.unit.transpiler.passes.utils import plan_sql
from tests.unit.utils import SQLA_DIALECTS
from tests.utils import format_sql

if TYPE_CHECKING:
    from collections.abc import Iterator

    from strawchemy.dto.strawberry import AggregateFilterDTO, BooleanFilterDTO
    from strawchemy.typing import SelectOf

_DIALECTS = pytest.mark.parametrize("dialect_name", ["postgresql", "sqlite", "mysql"])
_SEEDED_TABLES = ("color", "fruit", "tag", "group", "user", "department", "user_department_join_table")
_USERS_NOT_TAGGED_IN_UNNAMED_DEPARTMENT = (
    '{ groups(filter: { users: { _not: { tag: { name: { eq: "x" } }, departments: { name: { isNull: true } } } } }) '
    "{ name } }"
)


class _PairBase(DeclarativeBase):
    pass


class _Pair(_PairBase):
    __tablename__ = "pair"

    left_id: Mapped[int] = mapped_column(primary_key=True)
    right_id: Mapped[int] = mapped_column(primary_key=True)
    name: Mapped[str]


def _named_pair(statement: SelectOf[_Pair], value: str, **_ctx: object) -> SelectOf[_Pair]:
    return statement.where(_Pair.name == value)


_strawchemy = Strawchemy("postgresql")


@_strawchemy.filter(_Pair, include=["name"])
class _PairCustomFilter:
    named_in: str = _strawchemy.filter_field(apply=_named_pair, join="in")


@dataclass(frozen=True)
class _SessionContext:
    session: Session


def _input_type(dto: type[Any], name: str) -> type[Any]:
    field = next(field for field in get_object_definition(dto, strict=True).fields if field.python_name == name)
    return cast("type[Any]", getattr(field.type, "of_type", field.type))


def _queried_names(session: Session, query: str, field: str) -> set[str]:
    """Runs ``query`` on ``session`` and returns the names of the rows of its ``field``."""
    result = schema.execute_sync(query, context_value=_SessionContext(session))
    assert not result.errors, result.errors
    assert result.data is not None
    return {row["name"] for row in result.data[field]}


def _fruit_named(name: str) -> BooleanFilterDTO:
    fruit_filter = _input_type(ColorFilter, "fruits")
    return fruit_filter(name=_input_type(fruit_filter, "name")(eq=name))


def _more_fruits_than(count: int) -> AggregateFilterDTO:
    aggregation_filter = _input_type(ColorFilter, "fruits_aggregate")
    count_filter = _input_type(aggregation_filter, "count")
    return aggregation_filter(count=count_filter(predicate=_input_type(count_filter, "predicate")(gt=count)))


def _updated_colors(session: Session, dto_filter: BooleanFilterDTO) -> set[str]:
    """Seeds colors named after their fruits, updates those ``dto_filter`` matches on SQLite and returns their names."""
    fruit_names = {"two": ["x", "z"], "three": ["x", "z", "w"], "no_z": ["x", "w", "v"], "no_x": ["z", "w", "v"]}
    session.add_all(
        Color(name=name, private="", fruits=[Fruit(name=fruit, sweetness=0, private="") for fruit in fruits])
        for name, fruits in {**fruit_names, "empty": []}.items()
    )
    session.flush()
    expressions = Transpiler(Color, SQLA_DIALECTS["sqlite"]).filter_expressions(dto_filter)
    session.execute(update(Color).where(*expressions).values(private="updated"))
    return set(session.scalars(select(Color.name).where(Color.private == "updated")))


@pytest.fixture
def sqlite_session() -> Iterator[Session]:
    """Yields a session on an in-memory SQLite database holding the tables the seeded tests write."""
    engine = create_engine("sqlite://")
    UUIDBase.metadata.create_all(engine, tables=[UUIDBase.metadata.tables[name] for name in _SEEDED_TABLES])
    with Session(engine) as session:
        yield session
    engine.dispose()


@_DIALECTS
def test_to_one_filter_inner_join(dialect_name: str) -> None:
    """A to-one filter inner-joins its relation once, and the selection of that relation reads the same join."""
    lines = plan_sql('{ groups(filter: { color: { name: { eq: "x" } } }) { color { name } } }', dialect_name)

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    'SELECT "group".id,',
                    "       color_1.name,",
                    "       color_1.id AS id_1",
                    '  FROM "group" AS "group"',
                    "  JOIN color AS color_1",
                    '    ON color_1.id = "group".color_id',
                    " WHERE color_1.name = %(name_1)s",
                    ' ORDER BY "group".id ASC,',
                    "          color_1.id ASC",
                ],
                "sqlite": [
                    'SELECT "group".id,',
                    "       color_1.name,",
                    "       color_1.id AS id_1",
                    '  FROM "group" AS "group"',
                    "  JOIN color AS color_1",
                    '    ON color_1.id = "group".color_id',
                    " WHERE color_1.name = ?",
                    ' ORDER BY "group".id ASC,',
                    "          color_1.id ASC",
                ],
                "mysql": [
                    "SELECT `group`.id,",
                    "       color_1.name,",
                    "       color_1.id AS id_1",
                    "  FROM `group` AS `group`",
                    " INNER JOIN color AS color_1",
                    "    ON color_1.id = `group`.color_id",
                    " WHERE color_1.name = %s",
                    " ORDER BY `group`.id ASC,",
                    "          color_1.id ASC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_to_many_filter_exists_without_root_copy(dialect_name: str) -> None:
    """A to-many filter is an EXISTS on the related table correlated to the root, without a copy of the root."""
    lines = plan_sql("{ colors(filter: { fruits: { sweetness: { gt: 1 } } }) { fruits { sweetness } } }", dialect_name)

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "SELECT color.id,",
                    "       fruit_1.sweetness,",
                    "       fruit_1.id AS id_1",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN fruit AS fruit_1",
                    "    ON color.id = fruit_1.color_id",
                    " WHERE EXISTS (",
                    "        SELECT 1",
                    "          FROM fruit AS fruit_2",
                    "         WHERE color.id = fruit_2.color_id",
                    "           AND fruit_2.sweetness > %(sweetness_1)s",
                    "       )",
                    " ORDER BY color.id ASC,",
                    "          fruit_1.id ASC",
                ],
                "sqlite": [
                    "SELECT color.id,",
                    "       fruit_1.sweetness,",
                    "       fruit_1.id AS id_1",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN fruit AS fruit_1",
                    "    ON color.id = fruit_1.color_id",
                    " WHERE EXISTS (",
                    "        SELECT 1",
                    "          FROM fruit AS fruit_2",
                    "         WHERE color.id = fruit_2.color_id",
                    "           AND fruit_2.sweetness > ?",
                    "       )",
                    " ORDER BY color.id ASC,",
                    "          fruit_1.id ASC",
                ],
                "mysql": [
                    "SELECT color.id,",
                    "       fruit_1.sweetness,",
                    "       fruit_1.id AS id_1",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN fruit AS fruit_1",
                    "    ON color.id = fruit_1.color_id",
                    " WHERE EXISTS (",
                    "        SELECT 1",
                    "          FROM fruit AS fruit_2",
                    "         WHERE color.id = fruit_2.color_id",
                    "           AND fruit_2.sweetness > %s",
                    "       )",
                    " ORDER BY color.id ASC,",
                    "          fruit_1.id ASC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_or_of_to_many_and_root_predicate_splits_exists(dialect_name: str) -> None:
    """A to-many branch OR-ed with a root predicate is an EXISTS on the relation OR the predicate, without root copy."""
    lines = plan_sql(
        '{ colors(filter: { _or: [{ fruits: { sweetness: { gt: 5 } } }, { name: { eq: "red" } }] }) { name } }',
        dialect_name,
    )

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " WHERE (EXISTS (SELECT 1 FROM fruit AS fruit_1 WHERE color.id = fruit_1.color_id AND fruit_1.sweetness > %(sweetness_1)s))",
                    "    OR color.name = %(name_1)s",
                    " ORDER BY color.id ASC",
                ],
                "sqlite": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " WHERE (EXISTS (SELECT 1 FROM fruit AS fruit_1 WHERE color.id = fruit_1.color_id AND fruit_1.sweetness > ?))",
                    "    OR color.name = ?",
                    " ORDER BY color.id ASC",
                ],
                "mysql": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " WHERE (EXISTS (SELECT 1 FROM fruit AS fruit_1 WHERE color.id = fruit_1.color_id AND fruit_1.sweetness > %s))",
                    "    OR color.name = %s",
                    " ORDER BY color.id ASC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_or_across_two_relations_is_two_exists(dialect_name: str) -> None:
    """An OR across two relations is one EXISTS per relation, joined by OR, without root copy or outer join."""
    lines = plan_sql(
        '{ groups(filter: { _or: [{ users: { name: { eq: "x" } } }, { color: { fruits: { name: { eq: "y" } } } }] }) '
        "{ name } }",
        dialect_name,
    )

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    'SELECT "group".name,',
                    '       "group".id',
                    '  FROM "group" AS "group"',
                    ' WHERE (EXISTS (SELECT 1 FROM "user" AS user_1 WHERE "group".id = user_1.group_id AND user_1.name = %(name_1)s))',
                    '    OR (EXISTS (SELECT 1 FROM color AS color_1 JOIN fruit AS fruit_1 ON color_1.id = fruit_1.color_id WHERE color_1.id = "group".color_id AND fruit_1.name = %(name_2)s))',
                    ' ORDER BY "group".id ASC',
                ],
                "sqlite": [
                    'SELECT "group".name,',
                    '       "group".id',
                    '  FROM "group" AS "group"',
                    ' WHERE (EXISTS (SELECT 1 FROM USER AS user_1 WHERE "group".id = user_1.group_id AND user_1.name = ?))',
                    '    OR (EXISTS (SELECT 1 FROM color AS color_1 JOIN fruit AS fruit_1 ON color_1.id = fruit_1.color_id WHERE color_1.id = "group".color_id AND fruit_1.name = ?))',
                    ' ORDER BY "group".id ASC',
                ],
                "mysql": [
                    "SELECT `group`.name,",
                    "       `group`.id",
                    "  FROM `group` AS `group`",
                    " WHERE (EXISTS (SELECT 1 FROM USER AS user_1 WHERE `group`.id = user_1.group_id AND user_1.name = %s))",
                    "    OR (EXISTS (SELECT 1 FROM color AS color_1 INNER JOIN fruit AS fruit_1 ON color_1.id = fruit_1.color_id WHERE color_1.id = `group`.color_id AND fruit_1.name = %s))",
                    " ORDER BY `group`.id ASC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_not_of_or_distributes(dialect_name: str) -> None:
    """A NOT over an OR of a to-many branch and a root predicate tests their EXISTS OR predicate is not true."""
    lines = plan_sql(
        '{ colors(filter: { _not: { _or: [{ fruits: { sweetness: { gt: 5 } } }, { name: { eq: "red" } }] } }) '
        "{ name } }",
        dialect_name,
    )

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " WHERE ((EXISTS (SELECT 1 FROM fruit AS fruit_1 WHERE color.id = fruit_1.color_id AND fruit_1.sweetness > %(sweetness_1)s)) OR color.name = %(name_1)s) IS NOT TRUE",
                    " ORDER BY color.id ASC",
                ],
                "sqlite": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " WHERE ((EXISTS (SELECT 1 FROM fruit AS fruit_1 WHERE color.id = fruit_1.color_id AND fruit_1.sweetness > ?)) OR color.name = ?) IS NOT 1",
                    " ORDER BY color.id ASC",
                ],
                "mysql": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " WHERE ((EXISTS (SELECT 1 FROM fruit AS fruit_1 WHERE color.id = fruit_1.color_id AND fruit_1.sweetness > %s)) OR color.name = %s) IS NOT TRUE",
                    " ORDER BY color.id ASC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_not_of_or_under_to_one_distributes(dialect_name: str) -> None:
    """Under a to-one relation, a NOT over an OR of a to-many branch and a column is tested on the joined relation."""
    lines = plan_sql(
        "{ groups(filter: { color: { _not: { _or: [{ fruits: { sweetness: { gt: 5 } } }, "
        '{ name: { eq: "red" } }] } } }) { name } }',
        dialect_name,
    )

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    'SELECT "group".name,',
                    '       "group".id',
                    '  FROM "group" AS "group"',
                    "  JOIN color AS color_1",
                    '    ON color_1.id = "group".color_id',
                    " WHERE ((EXISTS (SELECT 1 FROM fruit AS fruit_1 WHERE color_1.id = fruit_1.color_id AND fruit_1.sweetness > %(sweetness_1)s)) OR color_1.name = %(name_1)s) IS NOT TRUE",
                    ' ORDER BY "group".id ASC',
                ],
                "sqlite": [
                    'SELECT "group".name,',
                    '       "group".id',
                    '  FROM "group" AS "group"',
                    "  JOIN color AS color_1",
                    '    ON color_1.id = "group".color_id',
                    " WHERE ((EXISTS (SELECT 1 FROM fruit AS fruit_1 WHERE color_1.id = fruit_1.color_id AND fruit_1.sweetness > ?)) OR color_1.name = ?) IS NOT 1",
                    ' ORDER BY "group".id ASC',
                ],
                "mysql": [
                    "SELECT `group`.name,",
                    "       `group`.id",
                    "  FROM `group` AS `group`",
                    " INNER JOIN color AS color_1",
                    "    ON color_1.id = `group`.color_id",
                    " WHERE ((EXISTS (SELECT 1 FROM fruit AS fruit_1 WHERE color_1.id = fruit_1.color_id AND fruit_1.sweetness > %s)) OR color_1.name = %s) IS NOT TRUE",
                    " ORDER BY `group`.id ASC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_not_of_or_with_aggregate_under_to_one_reads_no_copy(dialect_name: str) -> None:
    """Under a to-one relation, a NOT over an OR of an aggregate and a to-many branch is tested on the joined relation."""
    lines = plan_sql(
        "{ groups(filter: { color: { _not: { _or: [{ fruitsAggregate: { count: { predicate: { gt: 2 } } } }, "
        '{ fruits: { name: { eq: "kiwi" } } }] } } }) { name } }',
        dialect_name,
    )

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    'SELECT "group".name,',
                    '       "group".id',
                    '  FROM "group" AS "group"',
                    "  JOIN color AS color_1",
                    '    ON color_1.id = "group".color_id',
                    "  JOIN LATERAL (",
                    "        SELECT count(*) AS count_1",
                    "          FROM fruit AS fruit_1",
                    "         WHERE color_1.id = fruit_1.color_id",
                    "       ) AS anon_1",
                    "    ON TRUE",
                    " WHERE (anon_1.count_1 > %(param_1)s OR (EXISTS (SELECT 1 FROM fruit AS fruit_2 WHERE color_1.id = fruit_2.color_id AND fruit_2.name = %(name_1)s))) IS NOT TRUE",
                    ' ORDER BY "group".id ASC',
                ],
                "sqlite": [
                    "WITH anon_1 AS (",
                    "        SELECT count(*) AS count_1,",
                    "               fruit_1.color_id AS color_id",
                    "          FROM fruit AS fruit_1",
                    "         WHERE fruit_1.color_id IS NOT NULL",
                    "         GROUP BY fruit_1.color_id",
                    '       ) SELECT "group".name,',
                    '       "group".id',
                    '  FROM "group" AS "group"',
                    "  JOIN color AS color_1",
                    '    ON color_1.id = "group".color_id',
                    "  LEFT OUTER JOIN anon_1",
                    "    ON color_1.id = anon_1.color_id",
                    " WHERE (coalesce(anon_1.count_1, ?) > ? OR (EXISTS (SELECT 1 FROM fruit AS fruit_2 WHERE color_1.id = fruit_2.color_id AND fruit_2.name = ?))) IS NOT 1",
                    ' ORDER BY "group".id ASC',
                ],
                "mysql": [
                    "WITH anon_1 AS (",
                    "        SELECT count(*) AS count_1,",
                    "               fruit_1.color_id AS color_id",
                    "          FROM fruit AS fruit_1",
                    "         WHERE fruit_1.color_id IS NOT NULL",
                    "         GROUP BY fruit_1.color_id",
                    "       ) SELECT `group`.name,",
                    "       `group`.id",
                    "  FROM `group` AS `group`",
                    " INNER JOIN color AS color_1",
                    "    ON color_1.id = `group`.color_id",
                    "  LEFT OUTER JOIN anon_1",
                    "    ON color_1.id = anon_1.color_id",
                    " WHERE (coalesce(anon_1.count_1, %s) > %s OR (EXISTS (SELECT 1 FROM fruit AS fruit_2 WHERE color_1.id = fruit_2.color_id AND fruit_2.name = %s))) IS NOT TRUE",
                    " ORDER BY `group`.id ASC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_not_of_or_with_aggregate_under_to_many_reads_no_copy(dialect_name: str) -> None:
    """Under a to-many relation, a NOT over an OR of an aggregate and a to-many branch reads the relation once."""
    lines = plan_sql(
        "{ groups(filter: { users: { _not: { _or: [{ departmentsAggregate: { count: { predicate: { gt: 0 } } } }, "
        '{ departments: { name: { eq: "d" } } }] } } }) { name } }',
        dialect_name,
    )

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    'SELECT "group".name,',
                    '       "group".id',
                    '  FROM "group" AS "group"',
                    " WHERE EXISTS (",
                    "        SELECT 1",
                    '          FROM "user" AS user_1',
                    "          JOIN LATERAL (",
                    "                SELECT count(*) AS count_1",
                    "                  FROM department AS department_1",
                    "                  JOIN user_department_join_table AS user_department_join_table_1",
                    "                    ON department_1.id = user_department_join_table_1.department_id",
                    "                 WHERE user_1.id = user_department_join_table_1.user_id",
                    "               ) AS anon_1",
                    "            ON TRUE",
                    '         WHERE "group".id = user_1.group_id',
                    "           AND (anon_1.count_1 > %(param_1)s OR (EXISTS (SELECT 1 FROM department AS department_2 JOIN user_department_join_table AS user_department_join_table_2 ON department_2.id = user_department_join_table_2.department_id WHERE user_1.id = user_department_join_table_2.user_id AND department_2.name = %(name_1)s))) IS NOT TRUE",
                    "       )",
                    ' ORDER BY "group".id ASC',
                ],
                "sqlite": [
                    "WITH anon_1 AS (",
                    "        SELECT count(*) AS count_1,",
                    "               user_2.id AS id",
                    "          FROM USER AS user_2",
                    "          JOIN user_department_join_table AS user_department_join_table_1",
                    "            ON user_2.id = user_department_join_table_1.user_id",
                    "          JOIN department AS department_1",
                    "            ON department_1.id = user_department_join_table_1.department_id",
                    "         GROUP BY user_2.id",
                    '       ) SELECT "group".name,',
                    '       "group".id',
                    '  FROM "group" AS "group"',
                    " WHERE EXISTS (",
                    "        SELECT 1",
                    "          FROM USER AS user_1",
                    "          LEFT OUTER JOIN anon_1",
                    "            ON user_1.id = anon_1.id",
                    '         WHERE "group".id = user_1.group_id',
                    "           AND (coalesce(anon_1.count_1, ?) > ? OR (EXISTS (SELECT 1 FROM department AS department_2 JOIN user_department_join_table AS user_department_join_table_2 ON department_2.id = user_department_join_table_2.department_id WHERE user_1.id = user_department_join_table_2.user_id AND department_2.name = ?))) IS NOT 1",
                    "       )",
                    ' ORDER BY "group".id ASC',
                ],
                "mysql": [
                    "WITH anon_1 AS (",
                    "        SELECT count(*) AS count_1,",
                    "               user_2.id AS id",
                    "          FROM USER AS user_2",
                    "         INNER JOIN user_department_join_table AS user_department_join_table_1",
                    "            ON user_2.id = user_department_join_table_1.user_id",
                    "         INNER JOIN department AS department_1",
                    "            ON department_1.id = user_department_join_table_1.department_id",
                    "         GROUP BY user_2.id",
                    "       ) SELECT `group`.name,",
                    "       `group`.id",
                    "  FROM `group` AS `group`",
                    " WHERE EXISTS (",
                    "        SELECT 1",
                    "          FROM USER AS user_1",
                    "          LEFT OUTER JOIN anon_1",
                    "            ON user_1.id = anon_1.id",
                    "         WHERE `group`.id = user_1.group_id",
                    "           AND (coalesce(anon_1.count_1, %s) > %s OR (EXISTS (SELECT 1 FROM department AS department_2 INNER JOIN user_department_join_table AS user_department_join_table_2 ON department_2.id = user_department_join_table_2.department_id WHERE user_1.id = user_department_join_table_2.user_id AND department_2.name = %s))) IS NOT TRUE",
                    "       )",
                    " ORDER BY `group`.id ASC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_and_inside_or_branch_keeps_one_exists(dialect_name: str) -> None:
    """Two predicates ANDed on one to-many relation in an OR branch stay in one EXISTS, so one row passes both."""
    lines = plan_sql(
        '{ colors(filter: { _or: [{ fruits: { sweetness: { gt: 5 }, name: { eq: "apple" } } }, '
        '{ name: { eq: "red" } }] }) { name } }',
        dialect_name,
    )

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " WHERE (EXISTS (SELECT 1 FROM fruit AS fruit_1 WHERE color.id = fruit_1.color_id AND (fruit_1.name = %(name_1)s AND fruit_1.sweetness > %(sweetness_1)s)))",
                    "    OR color.name = %(name_2)s",
                    " ORDER BY color.id ASC",
                ],
                "sqlite": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " WHERE (EXISTS (SELECT 1 FROM fruit AS fruit_1 WHERE color.id = fruit_1.color_id AND (fruit_1.name = ? AND fruit_1.sweetness > ?)))",
                    "    OR color.name = ?",
                    " ORDER BY color.id ASC",
                ],
                "mysql": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " WHERE (EXISTS (SELECT 1 FROM fruit AS fruit_1 WHERE color.id = fruit_1.color_id AND (fruit_1.name = %s AND fruit_1.sweetness > %s)))",
                    "    OR color.name = %s",
                    " ORDER BY color.id ASC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_not_adds_null_check(dialect_name: str) -> None:
    """Under NOT, a comparison also requires its column to be non-null, so that a NULL fails it."""
    lines = plan_sql('{ colors(filter: { _not: { name: { eq: "x" } } }) { name } }', dialect_name)

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " WHERE NOT (color.name = %(name_1)s AND color.name IS NOT NULL)",
                    " ORDER BY color.id ASC",
                ],
                "sqlite": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " WHERE NOT (color.name = ? AND color.name IS NOT NULL)",
                    " ORDER BY color.id ASC",
                ],
                "mysql": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " WHERE NOT (color.name = %s AND color.name IS NOT NULL)",
                    " ORDER BY color.id ASC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_and_branch_aggregating_a_shared_to_many_is_tested_on_the_relation_rows(dialect_name: str) -> None:
    """Under a to-one, an ``_and`` branch aggregating a to-many the fields also join is tested on the joined rows."""
    lines = plan_sql(
        '{ groups(filter: { color: { fruits: { name: { eq: "x" } }, _and: [{ fruitsAggregate: { count: { predicate: '
        '{ gt: 1 } } }, fruits: { name: { eq: "z" } } }] } }) { name } }',
        dialect_name,
    )

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    'SELECT "group".name,',
                    '       "group".id',
                    '  FROM "group" AS "group"',
                    " WHERE EXISTS (",
                    "        SELECT 1",
                    "          FROM color AS color_1",
                    "          JOIN LATERAL (",
                    "                SELECT count(*) AS count_1",
                    "                  FROM fruit AS fruit_1",
                    "                 WHERE color_1.id = fruit_1.color_id",
                    "               ) AS anon_1",
                    "            ON TRUE",
                    "          JOIN fruit AS fruit_2",
                    "            ON color_1.id = fruit_2.color_id",
                    '         WHERE color_1.id = "group".color_id',
                    "           AND (anon_1.count_1 > %(param_1)s AND (EXISTS (SELECT 1 FROM fruit AS fruit_3 WHERE color_1.id = fruit_3.color_id AND fruit_3.name = %(name_1)s)) AND fruit_2.name = %(name_2)s)",
                    "       )",
                    ' ORDER BY "group".id ASC',
                ],
                "sqlite": [
                    "WITH anon_1 AS (",
                    "        SELECT count(*) AS count_1,",
                    "               fruit_1.color_id AS color_id",
                    "          FROM fruit AS fruit_1",
                    "         WHERE fruit_1.color_id IS NOT NULL",
                    "         GROUP BY fruit_1.color_id",
                    '       ) SELECT "group".name,',
                    '       "group".id',
                    '  FROM "group" AS "group"',
                    " WHERE EXISTS (",
                    "        SELECT 1",
                    "          FROM color AS color_1",
                    "          LEFT OUTER JOIN anon_1",
                    "            ON color_1.id = anon_1.color_id",
                    "          JOIN fruit AS fruit_2",
                    "            ON color_1.id = fruit_2.color_id",
                    '         WHERE color_1.id = "group".color_id',
                    "           AND (coalesce(anon_1.count_1, ?) > ? AND (EXISTS (SELECT 1 FROM fruit AS fruit_3 WHERE color_1.id = fruit_3.color_id AND fruit_3.name = ?)) AND fruit_2.name = ?)",
                    "       )",
                    ' ORDER BY "group".id ASC',
                ],
                "mysql": [
                    "WITH anon_1 AS (",
                    "        SELECT count(*) AS count_1,",
                    "               fruit_1.color_id AS color_id",
                    "          FROM fruit AS fruit_1",
                    "         WHERE fruit_1.color_id IS NOT NULL",
                    "         GROUP BY fruit_1.color_id",
                    "       ) SELECT `group`.name,",
                    "       `group`.id",
                    "  FROM `group` AS `group`",
                    " WHERE EXISTS (",
                    "        SELECT 1",
                    "          FROM color AS color_1",
                    "          LEFT OUTER JOIN anon_1",
                    "            ON color_1.id = anon_1.color_id",
                    "         INNER JOIN fruit AS fruit_2",
                    "            ON color_1.id = fruit_2.color_id",
                    "         WHERE color_1.id = `group`.color_id",
                    "           AND (coalesce(anon_1.count_1, %s) > %s AND (EXISTS (SELECT 1 FROM fruit AS fruit_3 WHERE color_1.id = fruit_3.color_id AND fruit_3.name = %s)) AND fruit_2.name = %s)",
                    "       )",
                    " ORDER BY `group`.id ASC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_or_on_outer_relation_guards_missing_row(dialect_name: str) -> None:
    """A to-one relation tested under one OR branch only is outer-joined, and its branch requires a related row."""
    lines = plan_sql(
        '{ groups(filter: { _or: [{ color: { name: { eq: "x" } } }, { name: { eq: "y" } }] }) { name } }',
        dialect_name,
    )

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    'SELECT "group".name,',
                    '       "group".id',
                    '  FROM "group" AS "group"',
                    "  LEFT OUTER JOIN color AS color_1",
                    '    ON color_1.id = "group".color_id',
                    " WHERE (color_1.name = %(name_1)s AND color_1.id IS NOT NULL)",
                    '    OR "group".name = %(name_2)s',
                    ' ORDER BY "group".id ASC',
                ],
                "sqlite": [
                    'SELECT "group".name,',
                    '       "group".id',
                    '  FROM "group" AS "group"',
                    "  LEFT OUTER JOIN color AS color_1",
                    '    ON color_1.id = "group".color_id',
                    " WHERE (color_1.name = ? AND color_1.id IS NOT NULL)",
                    '    OR "group".name = ?',
                    ' ORDER BY "group".id ASC',
                ],
                "mysql": [
                    "SELECT `group`.name,",
                    "       `group`.id",
                    "  FROM `group` AS `group`",
                    "  LEFT OUTER JOIN color AS color_1",
                    "    ON color_1.id = `group`.color_id",
                    " WHERE (color_1.name = %s AND color_1.id IS NOT NULL)",
                    "    OR `group`.name = %s",
                    " ORDER BY `group`.id ASC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_custom_filter_in_single_pk(dialect_name: str) -> None:
    """A custom filter joined with ``in`` matches the root's single primary key against the callback's keys."""
    lines = plan_sql('{ colorsCustomFilter(filter: { namedIn: "x" }) { name } }', dialect_name)

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " WHERE color.id IN (",
                    "        SELECT custom_filter.id",
                    "          FROM color AS custom_filter",
                    "         WHERE custom_filter.name = %(name_1)s",
                    "       )",
                    " ORDER BY color.id ASC",
                ],
                "sqlite": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " WHERE color.id IN (",
                    "        SELECT custom_filter.id",
                    "          FROM color AS custom_filter",
                    "         WHERE custom_filter.name = ?",
                    "       )",
                    " ORDER BY color.id ASC",
                ],
                "mysql": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " WHERE color.id IN (",
                    "        SELECT custom_filter.id",
                    "          FROM color AS custom_filter",
                    "         WHERE custom_filter.name = %s",
                    "       )",
                    " ORDER BY color.id ASC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_custom_filter_in_composite_pk(dialect_name: str) -> None:
    """A custom filter joined with ``in`` on a composite primary key matches the key tuple against the callback's."""
    dialect = SQLA_DIALECTS[dialect_name]

    expressions = Transpiler(_Pair, dialect).filter_expressions(_PairCustomFilter(named_in="x"))  # ty: ignore[unknown-argument]  # input fields are generated at runtime

    statement = update(_Pair).values(name="y").where(*expressions)
    lines = format_sql(str(statement.compile(dialect=dialect))).splitlines()
    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "UPDATE pair",
                    "   SET name = %(name)s",
                    " WHERE (pair.left_id, pair.right_id) IN (",
                    "        SELECT custom_filter.left_id,",
                    "               custom_filter.right_id",
                    "          FROM pair AS custom_filter",
                    "         WHERE custom_filter.name = %(name_1)s",
                    "       )",
                ],
                "sqlite": [
                    "UPDATE pair",
                    "   SET name = ?",
                    " WHERE (pair.left_id, pair.right_id) IN (",
                    "        SELECT custom_filter.left_id,",
                    "               custom_filter.right_id",
                    "          FROM pair AS custom_filter",
                    "         WHERE custom_filter.name = ?",
                    "       )",
                ],
                "mysql": [
                    "UPDATE pair",
                    "   SET name = %s",
                    " WHERE (pair.left_id, pair.right_id) IN (",
                    "        SELECT custom_filter.left_id,",
                    "               custom_filter.right_id",
                    "          FROM pair AS custom_filter",
                    "         WHERE custom_filter.name = %s",
                    "       )",
                ],
            }
        )[dialect_name]
    )


def test_aggregate_filter_shares_join() -> None:
    """A filtered and selected aggregation is computed by one LATERAL join."""
    lines = plan_sql(
        "{ colors(filter: { fruitsAggregate: { count: { predicate: { gt: 0 } } } }) { fruitsAggregate { count } } }",
        "postgresql",
    )

    assert lines == snapshot(
        [
            "SELECT color.id,",
            "       anon_1.count_1",
            "  FROM color AS color",
            "  JOIN LATERAL (",
            "        SELECT count(*) AS count_1",
            "          FROM fruit AS fruit_1",
            "         WHERE color.id = fruit_1.color_id",
            "       ) AS anon_1",
            "    ON TRUE",
            " WHERE anon_1.count_1 > %(param_1)s",
            " ORDER BY color.id ASC",
        ]
    )


def test_filter_expressions_mysql_derived_table() -> None:
    """On MySQL, a DML relation filter reads the matched keys through ``dml_matched``, correlated to the real table."""
    fruit_filter = _input_type(ColorFilter, "fruits")
    text_comparison = _input_type(fruit_filter, "name")
    dto_filter = ColorFilter(fruits=fruit_filter(name=text_comparison(eq="x")))  # ty: ignore[unknown-argument]  # input fields are generated at runtime
    dialect = SQLA_DIALECTS["mysql"]

    expressions = Transpiler(Color, dialect).filter_expressions(dto_filter)

    statement = select(Color.id).where(*expressions)
    assert_no_duplicate_reads(statement, dialect)
    assert format_sql(str(statement.compile(dialect=dialect))).splitlines() == snapshot(
        [
            "SELECT color.id",
            "  FROM color",
            " WHERE EXISTS (",
            "        SELECT 1",
            "          FROM (",
            "                SELECT color_1.id AS id",
            "                  FROM color AS color_1",
            "                 INNER JOIN fruit AS fruit_1",
            "                    ON color_1.id = fruit_1.color_id",
            "                 WHERE fruit_1.name = %s",
            "               ) AS dml_matched",
            "         WHERE dml_matched.id = color.id",
            "       )",
        ]
    )


def _dml_sql(dto_filter: BooleanFilterDTO, dialect_name: str) -> list[str]:
    dialect = SQLA_DIALECTS[dialect_name]
    statement = update(Color).values(name="y").where(*Transpiler(Color, dialect).filter_expressions(dto_filter))
    return format_sql(str(statement.compile(dialect=dialect))).splitlines()


@pytest.mark.parametrize("dialect_name", ["postgresql", "sqlite"])
def test_filter_expressions_not_to_many_is_a_not_exists_on_the_table(dialect_name: str) -> None:
    """A DML ``_not`` over a to-many relation is a NOT EXISTS correlated to the table, without a copy of the root."""
    fruit_filter = _input_type(ColorFilter, "fruits")
    text_comparison = _input_type(fruit_filter, "name")
    dto_filter = ColorFilter(not_=ColorFilter(fruits=fruit_filter(name=text_comparison(eq="x"))))  # ty: ignore[unknown-argument]  # input fields are generated at runtime

    lines = _dml_sql(dto_filter, dialect_name)

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "UPDATE color",
                    "   SET name = %(name)s",
                    " WHERE NOT (EXISTS (SELECT 1 FROM fruit AS fruit_1 WHERE color.id = fruit_1.color_id AND fruit_1.name = %(name_1)s))",
                ],
                "sqlite": [
                    "UPDATE color",
                    "   SET name = ?",
                    " WHERE NOT (EXISTS (SELECT 1 FROM fruit AS fruit_1 WHERE color.id = fruit_1.color_id AND fruit_1.name = ?))",
                ],
            }
        )[dialect_name]
    )


@pytest.mark.parametrize(
    ("negated", "updated"),
    [pytest.param(False, {"three"}, id="and"), pytest.param(True, {"two", "no_z", "no_x", "empty"}, id="not")],
)
def test_filter_expressions_and_branch_aggregating_a_shared_to_many(
    negated: bool, updated: set[str], sqlite_session: Session
) -> None:
    """A DML ``_and`` branch counting a to-many relation the fields also filter, or its ``_not``, updates the matches."""
    branch = ColorFilter(fruits_aggregate=_more_fruits_than(2), fruits=_fruit_named("z"))  # ty: ignore[unknown-argument]  # input fields are generated at runtime
    dto_filter = ColorFilter(fruits=_fruit_named("x"), and_=[branch])  # ty: ignore[unknown-argument]  # input fields are generated at runtime

    names = _updated_colors(sqlite_session, ColorFilter(not_=dto_filter) if negated else dto_filter)  # ty: ignore[unknown-argument]  # input fields are generated at runtime

    assert names == updated


def test_filter_expressions_not_of_not_aggregating_a_to_many(sqlite_session: Session) -> None:
    """A DML ``_not`` of a ``_not`` testing a to-many relation and counting it updates the colors passing both."""
    tested = ColorFilter(fruits=_fruit_named("x"), fruits_aggregate=_more_fruits_than(2))  # ty: ignore[unknown-argument]  # input fields are generated at runtime

    names = _updated_colors(sqlite_session, ColorFilter(not_=ColorFilter(not_=tested)))  # ty: ignore[unknown-argument]  # input fields are generated at runtime

    assert names == {"three", "no_z"}


@pytest.mark.parametrize("dialect_name", ["postgresql", "sqlite"])
def test_filter_expressions_not_of_or_fails_null_column(dialect_name: str) -> None:
    """A DML NOT over an OR of a to-many branch and a column makes a NULL column fail the column predicate."""
    fruit_filter = _input_type(ColorFilter, "fruits")
    text_comparison = _input_type(fruit_filter, "name")
    fruit_branch = ColorFilter(fruits=fruit_filter(name=text_comparison(eq="x")))  # ty: ignore[unknown-argument]  # input fields are generated at runtime
    name_branch = ColorFilter(name=text_comparison(eq="y"))  # ty: ignore[unknown-argument]  # input fields are generated at runtime
    dto_filter = ColorFilter(not_=ColorFilter(or_=[fruit_branch, name_branch]))  # ty: ignore[unknown-argument]  # input fields are generated at runtime

    lines = _dml_sql(dto_filter, dialect_name)

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "UPDATE color",
                    "   SET name = %(name)s",
                    " WHERE NOT ((EXISTS (SELECT 1 FROM fruit AS fruit_1 WHERE color.id = fruit_1.color_id AND fruit_1.name = %(name_1)s)) OR (color.name = %(name_2)s AND color.name IS NOT NULL))",
                ],
                "sqlite": [
                    "UPDATE color",
                    "   SET name = ?",
                    " WHERE NOT ((EXISTS (SELECT 1 FROM fruit AS fruit_1 WHERE color.id = fruit_1.color_id AND fruit_1.name = ?)) OR (color.name = ? AND color.name IS NOT NULL))",
                ],
            }
        )[dialect_name]
    )


@pytest.mark.parametrize("dialect_name", ["postgresql", "sqlite"])
def test_filter_expressions_or_of_relation_and_column_splits_exists(dialect_name: str) -> None:
    """A DML OR of a to-many branch and a column is an EXISTS correlated to the table OR the column predicate."""
    fruit_filter = _input_type(ColorFilter, "fruits")
    text_comparison = _input_type(fruit_filter, "name")
    dto_filter = ColorFilter(
        or_=[ColorFilter(fruits=fruit_filter(name=text_comparison(eq="x"))), ColorFilter(name=text_comparison(eq="y"))]  # ty: ignore[unknown-argument]  # input fields are generated at runtime
    )

    lines = _dml_sql(dto_filter, dialect_name)

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "UPDATE color",
                    "   SET name = %(name)s",
                    " WHERE (EXISTS (SELECT 1 FROM fruit AS fruit_1 WHERE color.id = fruit_1.color_id AND fruit_1.name = %(name_1)s))",
                    "    OR color.name = %(name_2)s",
                ],
                "sqlite": [
                    "UPDATE color",
                    "   SET name = ?",
                    " WHERE (EXISTS (SELECT 1 FROM fruit AS fruit_1 WHERE color.id = fruit_1.color_id AND fruit_1.name = ?))",
                    "    OR color.name = ?",
                ],
            }
        )[dialect_name]
    )


@pytest.mark.parametrize("dialect_name", ["postgresql", "sqlite"])
def test_filter_expressions_aggregation_reads_a_copy_of_the_root(dialect_name: str) -> None:
    """A DML aggregation filter reads a root copy: a known old-planner exception, pending the user's decision."""
    aggregation_filter = _input_type(ColorFilter, "fruits_aggregate")
    count_filter = _input_type(aggregation_filter, "count")
    predicate = _input_type(count_filter, "predicate")
    dto_filter = ColorFilter(fruits_aggregate=aggregation_filter(count=count_filter(predicate=predicate(gt=2))))  # ty: ignore[unknown-argument]  # input fields are generated at runtime

    lines = _dml_sql(dto_filter, dialect_name)

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "UPDATE color",
                    "   SET name = %(name)s",
                    " WHERE EXISTS (",
                    "        SELECT 1",
                    "          FROM color AS color_1",
                    "          JOIN LATERAL (",
                    "                SELECT count(*) AS count_1",
                    "                  FROM fruit AS fruit_1",
                    "                 WHERE color_1.id = fruit_1.color_id",
                    "               ) AS anon_1",
                    "            ON TRUE",
                    "         WHERE anon_1.count_1 > %(param_1)s",
                    "           AND color_1.id = color.id",
                    "       )",
                ],
                "sqlite": [
                    "WITH anon_1 AS (",
                    "        SELECT count(*) AS count_1,",
                    "               fruit_1.color_id AS color_id",
                    "          FROM fruit AS fruit_1",
                    "         WHERE fruit_1.color_id IS NOT NULL",
                    "         GROUP BY fruit_1.color_id",
                    "       ) UPDATE color",
                    "   SET name = ?",
                    " WHERE EXISTS (",
                    "        SELECT 1",
                    "          FROM color AS color_1",
                    "          LEFT OUTER JOIN anon_1",
                    "            ON color_1.id = anon_1.color_id",
                    "         WHERE coalesce(anon_1.count_1, ?) > ?",
                    "           AND color_1.id = color.id",
                    "       )",
                ],
            }
        )[dialect_name]
    )


def test_or_next_to_column_predicate_is_parenthesized() -> None:
    """An ``_or`` next to a column predicate is grouped, so that the predicate applies to both branches."""
    lines = plan_sql(
        '{ colors(filter: { name: { eq: "x" }, _or: [{ name: { eq: "a" } }, { name: { eq: "b" } }] }) { id } }',
        "sqlite",
    )

    assert lines == snapshot(
        [
            "SELECT color.id",
            "  FROM color AS color",
            " WHERE color.name = ?",
            "   AND (color.name = ? OR color.name = ?)",
            " ORDER BY color.id ASC",
        ]
    )


def test_and_under_or_keeps_its_parentheses() -> None:
    """A multi-predicate ``_and`` branch of an ``_or`` is grouped, so that OR does not split it."""
    lines = plan_sql(
        '{ colors(filter: { _or: [{ _and: [{ name: { eq: "a" } }, { name: { neq: "c" } }] }, { name: { eq: "b" } }] }) '
        "{ id } }",
        "sqlite",
    )

    assert lines == snapshot(
        [
            "SELECT color.id",
            "  FROM color AS color",
            " WHERE (color.name = ? AND color.name != ?)",
            "    OR color.name = ?",
            " ORDER BY color.id ASC",
        ]
    )


def test_predicates_next_to_or_are_grouped() -> None:
    """Several column predicates next to an ``_or`` are grouped apart from it."""
    lines = plan_sql(
        '{ colors(filter: { name: { eq: "x", neq: "y" }, _or: [{ name: { eq: "a" } }, { name: { eq: "b" } }] }) '
        "{ id } }",
        "sqlite",
    )

    assert lines == snapshot(
        [
            "SELECT color.id",
            "  FROM color AS color",
            " WHERE (color.name = ? AND color.name != ?)",
            "   AND (color.name = ? OR color.name = ?)",
            " ORDER BY color.id ASC",
        ]
    )


@_DIALECTS
def test_two_split_ors_are_each_an_or_of_exists(dialect_name: str) -> None:
    """Two ``_or`` of a to-many branch and a root predicate are AND-ed, each its own EXISTS OR predicate."""
    lines = plan_sql(
        '{ colors(filter: { _and: [{ _or: [{ fruits: { sweetness: { gt: 5 } } }, { name: { eq: "red" } }] }, '
        '{ _or: [{ fruits: { name: { eq: "kiwi" } } }, { name: { eq: "blue" } }] }] }) { name } }',
        dialect_name,
    )

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " WHERE ((EXISTS (SELECT 1 FROM fruit AS fruit_1 WHERE color.id = fruit_1.color_id AND fruit_1.name = %(name_1)s)) OR color.name = %(name_2)s)",
                    "   AND ((EXISTS (SELECT 1 FROM fruit AS fruit_2 WHERE color.id = fruit_2.color_id AND fruit_2.sweetness > %(sweetness_1)s)) OR color.name = %(name_3)s)",
                    " ORDER BY color.id ASC",
                ],
                "sqlite": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " WHERE ((EXISTS (SELECT 1 FROM fruit AS fruit_1 WHERE color.id = fruit_1.color_id AND fruit_1.name = ?)) OR color.name = ?)",
                    "   AND ((EXISTS (SELECT 1 FROM fruit AS fruit_2 WHERE color.id = fruit_2.color_id AND fruit_2.sweetness > ?)) OR color.name = ?)",
                    " ORDER BY color.id ASC",
                ],
                "mysql": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " WHERE ((EXISTS (SELECT 1 FROM fruit AS fruit_1 WHERE color.id = fruit_1.color_id AND fruit_1.name = %s)) OR color.name = %s)",
                    "   AND ((EXISTS (SELECT 1 FROM fruit AS fruit_2 WHERE color.id = fruit_2.color_id AND fruit_2.sweetness > %s)) OR color.name = %s)",
                    " ORDER BY color.id ASC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_to_many_not_of_to_one_and_secondary_to_many_is_one_not_exists(dialect_name: str) -> None:
    """A to-many ``_not`` of a to-one and a secondary to-many is one NOT EXISTS reading both from the related row."""
    lines = plan_sql(_USERS_NOT_TAGGED_IN_UNNAMED_DEPARTMENT, dialect_name)

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    'SELECT "group".name,',
                    '       "group".id',
                    '  FROM "group" AS "group"',
                    " WHERE EXISTS (",
                    "        SELECT 1",
                    '          FROM "user" AS user_1',
                    '         WHERE "group".id = user_1.group_id',
                    "           AND NOT (EXISTS (SELECT 1 FROM tag AS tag_1, department AS department_1 JOIN user_department_join_table AS user_department_join_table_1 ON department_1.id = user_department_join_table_1.department_id WHERE tag_1.id = user_1.tag_id AND user_1.id = user_department_join_table_1.user_id AND tag_1.name = %(name_1)s AND department_1.name IS NULL))",
                    "       )",
                    ' ORDER BY "group".id ASC',
                ],
                "sqlite": [
                    'SELECT "group".name,',
                    '       "group".id',
                    '  FROM "group" AS "group"',
                    " WHERE EXISTS (",
                    "        SELECT 1",
                    "          FROM USER AS user_1",
                    '         WHERE "group".id = user_1.group_id',
                    "           AND NOT (EXISTS (SELECT 1 FROM tag AS tag_1, department AS department_1 JOIN user_department_join_table AS user_department_join_table_1 ON department_1.id = user_department_join_table_1.department_id WHERE tag_1.id = user_1.tag_id AND user_1.id = user_department_join_table_1.user_id AND tag_1.name = ? AND department_1.name IS NULL))",
                    "       )",
                    ' ORDER BY "group".id ASC',
                ],
                "mysql": [
                    "SELECT `group`.name,",
                    "       `group`.id",
                    "  FROM `group` AS `group`",
                    " WHERE EXISTS (",
                    "        SELECT 1",
                    "          FROM USER AS user_1",
                    "         WHERE `group`.id = user_1.group_id",
                    "           AND NOT (EXISTS (SELECT 1 FROM tag AS tag_1, department AS department_1 INNER JOIN user_department_join_table AS user_department_join_table_1 ON department_1.id = user_department_join_table_1.department_id WHERE tag_1.id = user_1.tag_id AND user_1.id = user_department_join_table_1.user_id AND tag_1.name = %s AND department_1.name IS NULL))",
                    "       )",
                    " ORDER BY `group`.id ASC",
                ],
            }
        )[dialect_name]
    )


def test_to_many_not_of_to_one_and_secondary_to_many_keeps_groups_with_a_passing_user(sqlite_session: Session) -> None:
    """A group passes a users ``_not`` of a tag and a department filter when one of its users fails either."""
    tag_x, tag_y, color = Tag(name="x", private=""), Tag(name="y", private=""), Color(name="c", private="")
    unnamed, named = Department(name=None, private=""), Department(name="d", private="")

    def user(tag: Tag | None, *departments: Department) -> User:
        return User(name="u", private="", tag=tag, departments=list(departments))

    users = {
        "no_user": [],
        "tagged_in_unnamed": [user(tag_x, unnamed)],
        "other_tag": [user(tag_y, unnamed)],
        "no_tag": [user(None, unnamed)],
        "named_department": [user(tag_x, named)],
        "one_passing": [user(tag_x, unnamed), user(tag_x)],
    }
    sqlite_session.add_all(
        Group(name=name, private="", tag=tag_x, color=color, users=group_users) for name, group_users in users.items()
    )
    sqlite_session.flush()

    names = _queried_names(sqlite_session, _USERS_NOT_TAGGED_IN_UNNAMED_DEPARTMENT, "groups")

    assert names == {"other_tag", "no_tag", "named_department", "one_passing"}


def test_aggregate_under_one_of_two_to_many_paths_joins_its_lateral_from_its_relation() -> None:
    """An EXISTS over two to-many paths, one counted, joins the count's LATERAL from the counted relation's row."""
    lines = plan_sql(
        "{ groups(filter: { users: { departmentsAggregate: { count: { predicate: { gt: 1 } } } }, "
        'color: { fruits: { name: { eq: "x" } } } }) { name } }',
        "postgresql",
    )

    assert lines == snapshot(
        [
            'SELECT "group".name,',
            '       "group".id',
            '  FROM "group" AS "group"',
            " WHERE EXISTS (",
            "        SELECT 1",
            '          FROM "user" AS user_1',
            "          JOIN LATERAL (",
            "                SELECT count(*) AS count_1",
            "                  FROM department AS department_1",
            "                  JOIN user_department_join_table AS user_department_join_table_1",
            "                    ON department_1.id = user_department_join_table_1.department_id",
            "                 WHERE user_1.id = user_department_join_table_1.user_id",
            "               ) AS anon_1",
            "            ON TRUE,",
            "               color AS color_1",
            "          JOIN fruit AS fruit_1",
            "            ON color_1.id = fruit_1.color_id",
            '         WHERE "group".id = user_1.group_id',
            '           AND color_1.id = "group".color_id',
            "           AND anon_1.count_1 > %(param_1)s",
            "           AND fruit_1.name = %(name_1)s",
            "       )",
            ' ORDER BY "group".id ASC',
        ]
    )
