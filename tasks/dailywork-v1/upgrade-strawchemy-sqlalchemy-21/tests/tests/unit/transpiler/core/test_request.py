"""Tests for ``QueryRequest`` and the aggregate function builders."""

from __future__ import annotations

import typing
from decimal import Decimal
from typing import Any, cast

import pytest
from inline_snapshot import snapshot
from sqlalchemy.dialects import mysql, postgresql
from sqlalchemy.orm import aliased

from strawchemy import Strawchemy
from strawchemy.dto.strawberry import BooleanFilterDTO, ExistsFilter, OrderByEnum, QueryNode
from strawchemy.exceptions import TranspilingError
from strawchemy.schema.filters.inputs import OrderComparison, TextComparison
from strawchemy.transpiler._core.functions import AggregateFunction, build
from strawchemy.transpiler._core.request import QueryRequest
from strawchemy.transpiler._core.split import FilterScope
from strawchemy.typing import QueryNodeType
from tests.unit.models import Color, Fruit
from tests.utils import as_dto

_strawchemy = Strawchemy("postgresql")


@_strawchemy.type(Color, include="all")
class _ColorType: ...


@_strawchemy.filter(Color, include="all")
class _ColorFilter: ...


@_strawchemy.order(Color, include="all")
class _ColorOrder: ...


def _unwrap(type_: object) -> type[Any]:
    """Returns the non-None member of an optional annotation."""
    args = [arg for arg in typing.get_args(type_) if arg is not type(None)]
    return cast("type[Any]", args[0] if args else type_)


@_strawchemy.filter(Fruit, include="all")
class _FruitFilter: ...


_FRUITS_FILTER = _unwrap(_ColorFilter.__annotations__["fruits"])
_FRUITS_AGGREGATE_FILTER = _unwrap(_ColorFilter.__annotations__["fruits_aggregate"])
_COUNT_FILTER = _unwrap(_FRUITS_AGGREGATE_FILTER.__annotations__["count"])
_FRUITS_AGGREGATE_ORDER = _unwrap(_ColorOrder.__annotations__["fruits_aggregate"])
_SUM_ORDER = _unwrap(_FRUITS_AGGREGATE_ORDER.__annotations__["sum"])


def _request(**kwargs: object) -> QueryRequest:
    fields: dict[str, Any] = {
        "model": Color,
        "selection_tree": None,
        "dto_filter": None,
        "order_by": (),
        "distinct_on": (),
        "limit": None,
        "offset": None,
        "allow_null": False,
    }
    fields.update(kwargs)
    return QueryRequest(**fields)


def _selection_with_max_sweetness() -> QueryNodeType:
    root = QueryNode.root_node(Color)
    aggregate = root.insert_child(as_dto(_ColorType).__dto_field_definitions__["fruits_aggregate"])
    aggregate_type = as_dto(_ColorType).__dto_field_definitions__["fruits_aggregate"].type_
    function = aggregate.insert_child(aggregate_type.__dto_field_definitions__["max"])
    function.insert_child(function.value.type_.__dto_field_definitions__["sweetness"])
    return root


def test_selection_defaults_to_primary_keys() -> None:
    """With no selection tree, the leaves of ``selection`` are the model's primary key."""
    request = _request()

    assert [leaf.value.model_field_name for leaf in request.selection.leaves()] == ["id"]


def test_selection_falls_back_to_primary_keys_without_nodes() -> None:
    """With root aggregations but no ``nodes`` child, ``selection`` is the primary key tree."""
    root = QueryNode.root_node(Color)
    root.graph_metadata.metadata.root_aggregations = True
    request = _request(selection_tree=root)

    assert [leaf.value.model_field_name for leaf in request.selection.leaves()] == ["id"]
    assert request.root_aggregation_tree is None


def test_order_by_nodes_keep_client_order() -> None:
    """Two order-by inputs (name DESC, then id ASC) give leaves in that order."""
    request = _request(
        order_by=(_ColorOrder(name=OrderByEnum.DESC), _ColorOrder(id=OrderByEnum.ASC)),  # ty: ignore[unknown-argument]  # input fields are generated at runtime
    )

    assert [node.value.model_field_name for node in request.order_by_nodes] == ["name", "id"]


def test_aggregate_functions_union_of_filter_order_selection() -> None:
    """Filter count, order sum.sweetness and selected max.sweetness give three functions on one node."""
    dto_filter = _ColorFilter(
        fruits_aggregate=_FRUITS_AGGREGATE_FILTER(count=_COUNT_FILTER(predicate=OrderComparison(gt=1)))  # ty: ignore[unknown-argument]  # input fields are generated at runtime
    )
    order = _ColorOrder(fruits_aggregate=_FRUITS_AGGREGATE_ORDER(sum=_SUM_ORDER(sweetness=OrderByEnum.DESC)))  # ty: ignore[unknown-argument]  # input fields are generated at runtime
    selection = _selection_with_max_sweetness()
    request = _request(selection_tree=selection, dto_filter=dto_filter, order_by=(order,))
    aggregation_node = selection.find_child(lambda child: child.value.is_aggregate)
    assert aggregation_node is not None

    functions = request.aggregate_functions(aggregation_node)

    assert sorted(function.name for function in functions.values()) == ["count", "max", "sum"]
    assert {function.name for function in functions.values() if function.arguments} == {"max", "sum"}
    assert {
        function.name for node, function in functions.items() if node in request.selected_functions(aggregation_node)
    } == {"max"}
    assert len(request.selected_functions(aggregation_node)) == 1


def test_aggregate_functions_same_node_same_function() -> None:
    """Asking twice for the functions of a node returns the very same ``AggregateFunction`` objects."""
    selection = _selection_with_max_sweetness()
    request = _request(selection_tree=selection)
    aggregation_node = selection.find_child(lambda child: child.value.is_aggregate)
    assert aggregation_node is not None

    first = request.aggregate_functions(aggregation_node)
    second = request.aggregate_functions(aggregation_node)

    assert all(first[node] is second[node] for node in first)


def test_aggregate_functions_empty_for_unknown_node() -> None:
    """A node no part of the request aggregates on has no functions."""
    request = _request()

    assert not request.aggregate_functions(request.selection)
    assert request.selected_functions(request.selection) == frozenset()


def test_for_relation_reads_relation_filter() -> None:
    """``for_relation`` takes ordering, DISTINCT ON and pagination from the node's relation filter."""
    root = QueryNode.root_node(Color)
    node = root.insert_child(as_dto(_ColorType).__dto_field_definitions__["fruits"])
    order = _ColorOrder(name=OrderByEnum.ASC)  # ty: ignore[unknown-argument]  # input fields are generated at runtime
    node.metadata.data.relation_filter = type(node.metadata.data.relation_filter)(limit=3, offset=1, order_by=(order,))

    request = QueryRequest.for_relation(node)

    assert (request.model, request.limit, request.offset, request.order_by) == (Fruit, 3, 1, (order,))
    assert request.selection_tree is node
    assert request.dto_filter is None


def test_for_relation_selection_is_the_relation_under_root_aggregations() -> None:
    """In a query with root aggregations, a relation's ``selection`` is its own node, not a primary key tree."""
    root = QueryNode.root_node(Color)
    root.graph_metadata.metadata.root_aggregations = True
    node = root.insert_child(as_dto(_ColorType).__dto_field_definitions__["fruits"])

    assert QueryRequest.for_relation(node).selection is node


@pytest.mark.parametrize("name", ["median", "mode"])
def test_unknown_function_raises(name: str) -> None:
    """Building a function whose name is not an aggregate raises ``TranspilingError``."""
    node = QueryNode.root_node(Fruit)
    function = AggregateFunction(node=node, name=name, arguments=(), distinct=False)

    with pytest.raises(TranspilingError, match=f"Unknown function {name}"):
        build(function, aliased(Fruit.__mapper__), "postgresql")


def test_build_count_without_arguments() -> None:
    """``count`` without arguments builds ``count(*)``; with ``over`` it gets an empty window."""
    node = QueryNode.root_node(Fruit)
    function = AggregateFunction(node=node, name="count", arguments=(), distinct=False)
    alias = aliased(Fruit.__mapper__)

    plain = build(function, alias, "postgresql")
    windowed = build(function, alias, "postgresql", over=True)

    assert str(plain.element.compile(dialect=postgresql.psycopg2.dialect())) == "count(*)"
    assert str(windowed.element.compile(dialect=postgresql.psycopg2.dialect())) == snapshot("count(*) OVER ()")


def test_build_distinct_argument_function() -> None:
    """A distinct function wraps its arguments in ``DISTINCT``."""
    selection = _selection_with_max_sweetness()
    argument = next(iter(selection.leaves()))
    function = AggregateFunction(node=argument, name="count", arguments=(argument,), distinct=True)

    label = build(function, aliased(Fruit.__mapper__), "postgresql")

    assert str(label.element.compile(dialect=postgresql.psycopg2.dialect())) == snapshot(
        "count(DISTINCT fruit_1.sweetness)"
    )


@pytest.mark.parametrize(
    ("name", "value", "expected"),
    [
        pytest.param("avg", Decimal("1.5"), 1.5, id="statistical-float"),
        pytest.param("sum", Decimal(2), 2, id="sum-int"),
    ],
)
@pytest.mark.parametrize(
    ("dialect", "numeric_coltype"),
    [
        pytest.param(postgresql.psycopg2.dialect(), 1700, id="postgresql"),
        pytest.param(mysql.dialect(), 246, id="mysql"),
    ],
)
def test_build_integer_argument_result_has_graphql_python_type(
    name: str, value: Decimal, expected: float, dialect: Any, numeric_coltype: int
) -> None:
    """Drivers return ``Decimal`` for these aggregates over integers; the result must be what GraphQL serializes."""
    argument = next(iter(_selection_with_max_sweetness().leaves()))
    function = AggregateFunction(node=argument, name=name, arguments=(argument,), distinct=False)

    label = build(function, aliased(Fruit.__mapper__), "postgresql")
    processor = label.type.dialect_impl(dialect).result_processor(dialect, numeric_coltype)

    assert processor is not None
    result = processor(value)
    assert result == expected
    assert type(result) is type(expected)


def _count_filter() -> object:
    return _FRUITS_AGGREGATE_FILTER(count=_COUNT_FILTER(predicate=OrderComparison(gt=1)))  # ty: ignore[unknown-argument]  # input fields are generated at runtime


def _mixed_filter() -> BooleanFilterDTO:
    return _ColorFilter(
        fruits=_FRUITS_FILTER(name=TextComparison(eq="apple")),  # ty: ignore[unknown-argument]  # input fields are generated at runtime
        fruits_aggregate=_count_filter(),  # ty: ignore[unknown-argument]  # input fields are generated at runtime
    )


def _aggregation_node(request: QueryRequest) -> QueryNodeType:
    node = request.selection.find_child(lambda child: child.value.is_aggregate)
    assert node is not None
    return node


def test_filter_split_query_scope_moves_to_many_branch_to_exists() -> None:
    """In the query scope, a to-many branch goes to ``exists`` and the count branch stays direct."""
    selection = _selection_with_max_sweetness()
    request = _request(selection_tree=selection, dto_filter=_mixed_filter())

    split = request.filter_split

    assert split.direct is not None
    assert len(list(split.direct.iter_aggregation_filters())) == 1
    assert split.exists is not None
    functions = request.aggregate_functions(_aggregation_node(request))
    assert {function.name for function in functions.values()} == {"count", "max"}
    assert request.selected_functions(_aggregation_node(request)) == {
        node for node, function in functions.items() if function.name == "max"
    }


def test_filter_split_only_to_many_branch_has_no_direct_filter() -> None:
    """A filter made of a to-many branch only is entirely ``exists`` and adds no function."""
    dto_filter = _ColorFilter(fruits=_FRUITS_FILTER(name=TextComparison(eq="apple")))  # ty: ignore[unknown-argument]  # input fields are generated at runtime
    request = _request(selection_tree=_selection_with_max_sweetness(), dto_filter=dto_filter)

    split = request.filter_split

    assert split.direct is None
    assert split.exists is dto_filter
    assert {function.name for function in request.aggregate_functions(_aggregation_node(request)).values()} == {"max"}


def test_filter_split_exists_scope_keeps_and_branches_direct() -> None:
    """In the exists scope the whole filter is direct, including the to-many branch."""
    dto_filter = _mixed_filter()
    request = _request(selection_tree=_selection_with_max_sweetness(), dto_filter=dto_filter, filter_scope="exists")

    split = request.filter_split

    assert split.direct is not None
    assert split.exists is None
    assert {function.name for function in request.aggregate_functions(_aggregation_node(request)).values()} == {
        "count",
        "max",
    }


@pytest.mark.parametrize("filter_scope", ["query", "exists", "dml"])
def test_filter_split_splits_or_at_the_exists(filter_scope: FilterScope) -> None:
    """An OR of a to-many branch and a column stays direct, its to-many branch alone in an EXISTS."""
    to_many = _ColorFilter(fruits=_FRUITS_FILTER(name=TextComparison(eq="apple")))  # ty: ignore[unknown-argument]  # input fields are generated at runtime
    dto_filter = _ColorFilter(or_=[to_many, _ColorFilter(name=TextComparison(eq="red"))])  # ty: ignore[unknown-argument]  # input fields are generated at runtime
    request = _request(dto_filter=dto_filter, filter_scope=filter_scope)

    split = request.filter_split

    assert split.exists is None
    assert split.direct is not None
    exists_branch, column_branch = split.direct.or_
    assert [type(value) for value in exists_branch.and_] == [ExistsFilter]
    exists_filter = exists_branch.and_[0]
    assert isinstance(exists_filter, ExistsFilter)
    assert exists_filter.dto_filter.and_ == [to_many]
    assert [leaf.field_node.value.model_field_name for leaf in column_branch.iter_leaves()] == ["name"]
    assert split.join_path == ()


def test_filter_split_dml_scope_moves_relation_branch_to_exists() -> None:
    """In the dml scope, a to-one relation branch goes to ``exists``."""
    dto_filter = _FruitFilter(
        name=TextComparison(eq="apple"),  # ty: ignore[unknown-argument]  # input fields are generated at runtime
        color=_unwrap(_FruitFilter.__annotations__["color"])(name=TextComparison(eq="red")),  # ty: ignore[unknown-argument]  # input fields are generated at runtime
    )
    request = _request(model=Fruit, dto_filter=dto_filter, filter_scope="dml")

    split = request.filter_split

    assert split.direct is not None
    assert split.exists is not None
    assert split.exists is not dto_filter


def test_filter_split_without_filter_is_empty() -> None:
    """A request without a filter has nothing direct and nothing in EXISTS."""
    split = _request().filter_split

    assert (split.direct, split.exists, split.join_path) == (None, None, ())
