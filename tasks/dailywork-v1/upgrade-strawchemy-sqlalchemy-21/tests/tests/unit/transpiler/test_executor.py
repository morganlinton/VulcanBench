"""DB-free tests for QueryExecutor plan-driven statement assembly."""

from __future__ import annotations

from dataclasses import replace
from types import SimpleNamespace
from typing import TYPE_CHECKING, Any, cast
from unittest.mock import MagicMock
from uuid import uuid4

import pytest
from inline_snapshot import snapshot
from sqlalchemy import Result, Select, func
from sqlalchemy.dialects import postgresql
from sqlalchemy.exc import MultipleResultsFound
from sqlalchemy.orm import aliased

from strawchemy.config.databases import DatabaseFeatures
from strawchemy.exceptions import QueryResultError
from strawchemy.transpiler._core.level import Level, PlanContext
from strawchemy.transpiler._core.plan import QueryPlan
from strawchemy.transpiler._core.request import QueryRequest
from strawchemy.transpiler._core.rowset import AliasPage, Projection, RowSet
from strawchemy.transpiler._executor import NodeResult, SyncQueryExecutor
from strawchemy.transpiler._passes import DEFAULT_PIPELINES
from tests.unit.models import Color, Fruit
from tests.utils import format_sql

if TYPE_CHECKING:
    from sqlalchemy.sql import ColumnElement

    from strawchemy.typing import QueryNodeType


class _Node:
    def __init__(self, parent: object = None) -> None:
        self.parent = parent


def _plan(limit: int | None = None) -> QueryPlan:
    """Plans a query over the Fruit model for executor tests."""
    context = PlanContext.create(Fruit, postgresql.psycopg2.dialect(), pipelines=DEFAULT_PIPELINES)
    request = QueryRequest(Fruit, None, None, (), (), limit, None, False)
    return context.pipelines.root.plan(Level.root(request, context))


def _result(*models: object) -> MagicMock:
    """Builds a mock ``Result`` yielding one computed-value-free row per model.

    Returns:
        A ``MagicMock`` standing in for a SQLAlchemy ``Result``.
    """
    rows: list[MagicMock] = []
    for model in models:
        row = MagicMock()
        row.__getitem__.return_value = model
        row._mapping = {}  # noqa: SLF001  # Row exposes computed values only via _mapping.
        rows.append(row)
    result = MagicMock(spec=Result)
    result.all.return_value = rows
    return result


def _shared_fruit_plan(
    page_a: AliasPage | None, page_b: AliasPage | None
) -> tuple[QueryPlan, QueryNodeType, QueryNodeType]:
    """Plans colors whose relation nodes ``a`` and ``b`` both read one fruit alias.

    Returns:
        The plan, and the nodes ``a`` and ``b``.
    """
    color, fruit = aliased(Color.__mapper__, name="color"), aliased(Fruit.__mapper__, name="fruit")
    root = cast("QueryNodeType", _Node())
    a, b = cast("QueryNodeType", _Node(root)), cast("QueryNodeType", _Node(root))
    projection = Projection.over(root, color)
    for node, page in ((a, page_a), (b, page_b)):
        projection = replace(projection, entities={**projection.entities, node: fruit})
        if page is not None:
            projection = projection.with_page(node, page)
    plan = QueryPlan(
        rows=RowSet.over(color),
        projection=projection,
        context=cast("Any", SimpleNamespace(db_features=DatabaseFeatures(dialect="postgresql"))),
    )
    return plan, a, b


def _ranked_result(
    color: Color, ranks: tuple[ColumnElement[int], ColumnElement[int]], *fruits: tuple[Fruit, int, int]
) -> MagicMock:
    """Builds a mock ``Result`` with one ``(color, fruit, rank_a, rank_b)`` row per fruit.

    Returns:
        A ``MagicMock`` standing in for a SQLAlchemy ``Result``.
    """
    rank_a, rank_b = ranks
    rows: list[MagicMock] = []
    for fruit, value_a, value_b in fruits:
        row = MagicMock()
        row.__getitem__.side_effect = (color, fruit, value_a, value_b).__getitem__
        row._mapping = {rank_a: value_a, rank_b: value_b}  # noqa: SLF001  # Row exposes ranks only via _mapping.
        rows.append(row)
    result = MagicMock(spec=Result)
    result.all.return_value = rows
    return result


def _ranks() -> tuple[ColumnElement[int], ColumnElement[int]]:
    return func.row_number().over().label("rank_a"), func.row_number().over().label("rank_b")


def test_shared_entity_fills_each_alias_by_its_page() -> None:
    """Two nodes on one fruit entity each keep only the fruits whose rank falls inside their own page."""
    ranks = _ranks()
    rank_a, rank_b = ranks
    plan, a, b = _shared_fruit_plan(AliasPage(rank_a, None, 2), AliasPage(rank_b, 1, 1))
    color, fruits = Color(), [Fruit(), Fruit(), Fruit()]
    session = MagicMock()
    session.execute.return_value = _ranked_result(color, ranks, (fruits[0], 1, 3), (fruits[1], 2, 2), (fruits[2], 3, 1))

    related = SyncQueryExecutor(plan=plan, id_field_definitions=[]).list(session).related_objects

    assert related[id(a)][id(color)] == fruits[:2]
    assert related[id(b)][id(color)] == [fruits[1]]


def test_shared_entity_orders_each_alias_by_its_rank() -> None:
    """Rows arriving in the rank order of ``a`` fill ``b`` in its own rank order; ``a``, without a page, keeps row order."""
    ranks = _ranks()
    _, rank_b = ranks
    plan, a, b = _shared_fruit_plan(None, AliasPage(rank_b, None, None))
    color, fruits = Color(), [Fruit(), Fruit(), Fruit()]
    session = MagicMock()
    session.execute.return_value = _ranked_result(color, ranks, (fruits[0], 1, 3), (fruits[1], 2, 2), (fruits[2], 3, 1))

    related = SyncQueryExecutor(plan=plan, id_field_definitions=[]).list(session).related_objects

    assert related[id(a)][id(color)] == fruits
    assert related[id(b)][id(color)] == fruits[::-1]


def test_repeated_object_keeps_its_first_rank() -> None:
    """An object on several shared rows with different ranks is placed by its smallest rank."""
    ranks = _ranks()
    _, rank_b = ranks
    plan, _, b = _shared_fruit_plan(None, AliasPage(rank_b, None, None))
    color, fruits = Color(), [Fruit(), Fruit()]
    session = MagicMock()
    session.execute.return_value = _ranked_result(color, ranks, (fruits[0], 1, 1), (fruits[1], 2, 2), (fruits[0], 3, 3))

    related = SyncQueryExecutor(plan=plan, id_field_definitions=[]).list(session).related_objects

    assert related[id(b)][id(color)] == fruits


def test_executor_emits_plan_in_statement() -> None:
    """statement() emits the held plan into a Select."""
    executor = SyncQueryExecutor(plan=_plan(), id_field_definitions=[])
    assert isinstance(executor.statement(), Select)


@pytest.mark.parametrize("limit", [None, 2], ids=["plain", "paginated"])
def test_executor_add_where_reads_the_root_alias(limit: int | None, request: pytest.FixtureRequest) -> None:
    """add_where predicates on the unaliased model are moved onto the plan's root alias, adding no FROM."""
    executor = SyncQueryExecutor(plan=_plan(limit), id_field_definitions=[])
    planned_froms = len(executor.statement().get_final_froms())
    executor.add_where(Fruit.id == uuid4())
    statement = executor.statement()
    assert (
        format_sql(str(statement)).splitlines()
        == snapshot(
            {
                "plain": ["SELECT fruit.id", "  FROM fruit AS fruit", " WHERE fruit.id = :id_1"],
                "paginated": [
                    "SELECT fruit.id",
                    "  FROM (",
                    "        SELECT fruit.id AS id",
                    "          FROM fruit AS fruit",
                    "         LIMIT :param_1",
                    "       ) AS fruit",
                    " WHERE fruit.id = :id_1",
                ],
            }
        )[request.node.callspec.id]
    )
    assert len(statement.get_final_froms()) == planned_froms


def test_executor_rejects_several_roots_when_fetching_one() -> None:
    """get_one_or_none raises when the returned rows fold onto more than one root."""
    session = MagicMock()
    session.execute.return_value = _result(Fruit(), Fruit())
    executor = SyncQueryExecutor(plan=_plan(), id_field_definitions=[])
    with pytest.raises(MultipleResultsFound):
        executor.get_one_or_none(session)


def test_executor_folds_rows_onto_their_root() -> None:
    """Rows repeating the same root collapse onto a single node."""
    session = MagicMock()
    fruit = Fruit()
    session.execute.return_value = _result(fruit, fruit)
    executor = SyncQueryExecutor(plan=_plan(), id_field_definitions=[])
    assert executor.list(session).nodes == [fruit]


def test_node_result_rejects_relation_not_selected() -> None:
    """value() raises for a relation node the query did not select, rather than reading the model attribute."""
    relation = SimpleNamespace(
        value=SimpleNamespace(is_computed=False, is_relation=True, name="color"),
        metadata=SimpleNamespace(data=SimpleNamespace(is_transform=False)),
    )
    with pytest.raises(QueryResultError, match="'color'"):
        NodeResult(model=Fruit(), computed_values={}).value(cast("Any", relation))
