from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

from tests.integration.typing import RawRecordData
from tests.typing import AnyQueryExecutor
from tests.utils import maybe_async

if TYPE_CHECKING:
    from syrupy.assertion import SnapshotAssertion

    from tests.integration.fixtures import QueryTracker

pytestmark = [pytest.mark.integration]


@pytest.mark.parametrize("fruits_query", ["fruitsHooks", "fruitsPaginatedHooks"])
@pytest.mark.snapshot
async def test_load_columns_hook(
    fruits_query: str,
    any_async_query: AnyQueryExecutor,
    raw_fruits: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    result = await maybe_async(any_async_query(f"{{ {fruits_query} {{ description }} }}"))

    assert not result.errors
    assert result.data
    assert result.data[fruits_query] == [
        {"description": f"The {fruit['name']} color id is {fruit['color_id']}"} for fruit in raw_fruits
    ]

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize("fruits_query", ["fruitsHooks", "fruitsPaginatedHooks"])
@pytest.mark.snapshot
async def test_load_relationships_with_columns(
    fruits_query: str,
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    raw_fruits: RawRecordData,
    raw_colors: RawRecordData,
) -> None:
    result = await maybe_async(any_async_query(f"{{ {fruits_query} {{ prettyColor }} }}"))

    assert not result.errors
    assert result.data
    assert result.data[fruits_query] == [
        {"prettyColor": f"Color is {next(color['name'] for color in raw_colors if color['id'] == fruit['color_id'])}"}
        for fruit in raw_fruits
    ]

    query_tracker.assert_statements(2, "select", sql_snapshot)


@pytest.mark.parametrize("fruits_query", ["fruitsHooks", "fruitsPaginatedHooks"])
@pytest.mark.snapshot
async def test_load_relationships_no_columns(
    fruits_query: str,
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    raw_fruits: RawRecordData,
    raw_farms: RawRecordData,
) -> None:
    result = await maybe_async(any_async_query(f"{{ {fruits_query} {{ prettyFarms }} }}"))

    assert not result.errors
    assert result.data
    farm_names = [
        ", ".join(farm["name"] for farm in raw_farms if farm["fruit_id"] == fruit["id"]) for fruit in raw_fruits
    ]
    assert result.data[fruits_query] == [{"prettyFarms": f"Farms are: {name}"} for name in farm_names]

    query_tracker.assert_statements(2, "select", sql_snapshot)


@pytest.mark.parametrize("query", ["colorsWithFilteredFruits", "colorsWithFilteredFruitsPaginated"])
@pytest.mark.snapshot
async def test_load_relationships_nested(
    query: str,
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    raw_colors: RawRecordData,
    raw_fruits: RawRecordData,
    raw_farms: RawRecordData,
) -> None:
    result = await maybe_async(any_async_query(f"{{ {query} {{ farms }} }}"))

    assert not result.errors
    assert result.data

    # The query asks for no ordering, so the farms of a fruit come back in a dialect-dependent order.
    farm_names = [
        sorted(
            farm["name"]
            for fruit in raw_fruits
            if fruit["color_id"] == color["id"]
            for farm in raw_farms
            if farm["fruit_id"] == fruit["id"]
        )
        for color in raw_colors
    ]
    assert [
        sorted(entry["farms"].removeprefix("Farms are: ").split(", ")) for entry in result.data[query]
    ] == farm_names

    query_tracker.assert_statements(2, "select", sql_snapshot)


@pytest.mark.parametrize("query", ["colorsWithFilteredFruits", "colorsWithFilteredFruitsPaginated"])
@pytest.mark.snapshot
async def test_load_nested_relationships_no_columns(
    query: str,
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    raw_colors: RawRecordData,
    raw_fruits: RawRecordData,
    raw_farms: RawRecordData,
) -> None:
    result = await maybe_async(any_async_query(f"{{ {query} {{ farmsNoColumns }} }}"))

    assert not result.errors
    assert result.data

    farm_names = [
        sorted(
            farm["name"]
            for fruit in raw_fruits
            if fruit["color_id"] == color["id"]
            for farm in raw_farms
            if farm["fruit_id"] == fruit["id"]
        )
        for color in raw_colors
    ]
    assert [
        sorted(entry["farmsNoColumns"].removeprefix("Farms are: ").split(", ")) for entry in result.data[query]
    ] == farm_names

    query_tracker.assert_statements(2, "select", sql_snapshot)


@pytest.mark.parametrize("query", ["colorsHooks", "colorsHooksPaginated"])
@pytest.mark.snapshot
async def test_load_relationships_on_nested_field(
    query: str,
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    raw_colors: RawRecordData,
    raw_fruits: RawRecordData,
) -> None:
    result = await maybe_async(any_async_query(f"{{ {query} {{ id fruits {{ prettyColor }} }} }}"))

    assert not result.errors
    assert result.data
    assert result.data[query] == [
        {
            "id": color["id"],
            "fruits": [
                {"prettyColor": f"Color is {color['name']}"}
                for _ in range(len([fruit for fruit in raw_fruits if fruit["color_id"] == color["id"]]))
            ],
        }
        for color in raw_colors
    ]

    query_tracker.assert_statements(2, "select", sql_snapshot)


@pytest.mark.parametrize("fruits_query", ["fruitsHooks", "fruitsPaginatedHooks"])
async def test_empty_query_hook(
    fruits_query: str, any_async_query: AnyQueryExecutor, raw_fruits: RawRecordData
) -> None:
    result = await maybe_async(any_async_query(f"{{ {fruits_query} {{ emptyQueryHook }} }}"))

    assert not result.errors
    assert result.data
    assert len(result.data[fruits_query]) == len(raw_fruits)
    assert result.data[fruits_query] == [{"emptyQueryHook": "success"} for _ in range(len(raw_fruits))]


@pytest.mark.parametrize("query", ["filteredFruits", "filteredFruitsPaginated"])
@pytest.mark.snapshot
async def test_custom_query_hook_where(
    query: str, any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    result = await maybe_async(any_async_query(f"{{ {query} {{ name }} }}"))

    assert not result.errors
    assert result.data
    assert len(result.data[query]) == 1
    assert result.data[query] == [{"name": "Apple"}]

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize("query", ["filteredFruits", "filteredFruitsPaginated", "fieldFilteredFruitsPaginated"])
@pytest.mark.snapshot
async def test_custom_query_hook_where_on_unselected_column(
    query: str,
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    raw_fruits: RawRecordData,
) -> None:
    """Test that a root query hook filters on a column the query does not select."""
    result = await maybe_async(any_async_query(f"{{ {query} {{ id }} }}"))

    assert not result.errors
    assert result.data
    apple_id = next(fruit["id"] for fruit in raw_fruits if fruit["name"] == "Apple")
    assert next(iter(result.data.values())) == [{"id": apple_id}]

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize("query", ["orderedFruits", "orderedFruitsPaginated"])
@pytest.mark.snapshot
async def test_custom_query_hook_order_by(
    query: str,
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    raw_fruits: RawRecordData,
) -> None:
    result = await maybe_async(any_async_query(f"{{ {query} {{ waterPercent }} }}"))

    assert not result.errors
    assert result.data
    assert len(result.data[query]) == len(raw_fruits)
    assert result.data[query] == sorted(
        [{"waterPercent": fruit["water_percent"]} for fruit in raw_fruits], key=lambda fruit: fruit["waterPercent"]
    )

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    ("query", "path"),
    [
        pytest.param("{ orderedFruitsPaginated { id waterPercent } }", ("orderedFruitsPaginated",), id="root-subquery"),
        pytest.param(
            "{ colorsWithOrderedFruits { fruits { id waterPercent } } }",
            ("colorsWithOrderedFruits", "fruits"),
            id="relation",
        ),
        pytest.param(
            "{ colorsWithPaginatedOrderedFruits { fruits(limit: 2) { id waterPercent } } }",
            ("colorsWithPaginatedOrderedFruits", "fruits"),
            id="paginated-relation",
        ),
    ],
)
async def test_query_hook_loaded_column_selected_once(
    query: str,
    path: tuple[str, ...],
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    raw_fruits: RawRecordData,
) -> None:
    """Test that a column loaded by a hook and selected by the query is selected once and read correctly."""
    result = await maybe_async(any_async_query(query))

    assert not result.errors
    assert result.data
    fruits = result.data[path[0]]
    if len(path) > 1:
        fruits = [fruit for color in fruits for fruit in color[path[1]]]
    assert fruits
    water_percents = {fruit["id"]: fruit["water_percent"] for fruit in raw_fruits}
    assert all(fruit["waterPercent"] == water_percents[fruit["id"]] for fruit in fruits)
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted.count(".water_percent AS ") == 1


@pytest.mark.snapshot
async def test_query_hook_on_type(
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    raw_colors: RawRecordData,
) -> None:
    result = await maybe_async(any_async_query("{ colorsWithFilteredFruits { name fruits { name } } }"))

    assert not result.errors
    assert result.data
    assert {
        color["name"]: [fruit["name"] for fruit in color["fruits"]] for color in result.data["colorsWithFilteredFruits"]
    } == {color["name"]: ["Apple"] if color["name"] == "Red" else [] for color in raw_colors}

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize("query", ["fieldFilteredFruits", "fieldFilteredFruitsPaginated"])
@pytest.mark.snapshot
async def test_root_field_query_hook_where(
    query: str, any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    """Test that a root list field's query hook filters the root rows."""
    result = await maybe_async(any_async_query(f"{{ {query} {{ name }} }}"))

    assert not result.errors
    assert result.data
    assert result.data[query] == [{"name": "Apple"}]

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(("apple", "expected"), [(True, {"name": "Apple"}), (False, None)])
@pytest.mark.snapshot
async def test_root_field_query_hook_where_get_by_id(
    apple: bool,
    expected: dict[str, str] | None,
    any_async_query: AnyQueryExecutor,
    raw_fruits: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Test that a root get-by-id field's query hook hides the rows it filters out."""
    fruit_id = next(fruit["id"] for fruit in raw_fruits if (fruit["name"] == "Apple") is apple)
    result = await maybe_async(any_async_query(f"{{ fieldFilteredFruit(id: {fruit_id}) {{ name }} }}"))

    assert not result.errors
    assert result.data == {"fieldFilteredFruit": expected}

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_root_field_query_hook_where_root_aggregations(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    """Test that a root aggregation field's query hook filters both nodes and aggregations."""
    result = await maybe_async(
        any_async_query("{ fieldFilteredFruitAggregations { aggregations { count } nodes { name } } }")
    )

    assert not result.errors
    assert result.data
    assert result.data["fieldFilteredFruitAggregations"] == {"aggregations": {"count": 1}, "nodes": [{"name": "Apple"}]}

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_root_field_query_hook_load(
    any_async_query: AnyQueryExecutor,
    raw_fruits: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Test that a root field's query hook loads its columns on the root model."""
    result = await maybe_async(any_async_query("{ fieldLoadFruits { name } }"))

    assert not result.errors
    assert result.data
    assert result.data["fieldLoadFruits"] == [{"name": fruit["name"]} for fruit in raw_fruits]

    assert query_tracker.query_count == 1
    assert "water_percent" in query_tracker[0].statement_str
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize("query", ["colorsWithSweetFruits", "colorsWithSweetFruitsPaginated"])
@pytest.mark.snapshot
async def test_query_hook_on_relation_field(
    query: str,
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    raw_colors: RawRecordData,
    raw_fruits: RawRecordData,
) -> None:
    """Test that a relation field's `query_hook` applies to the related model, not the parent."""
    result = await maybe_async(any_async_query(f"{{ {query} {{ name fruits {{ name }} }} }}"))

    assert not result.errors
    assert result.data
    sweet_fruits = {
        color["name"]: sorted(
            fruit["name"] for fruit in raw_fruits if fruit["color_id"] == color["id"] and fruit["sweetness"] > 5
        )
        for color in raw_colors
    }
    assert {
        color["name"]: sorted(fruit["name"] for fruit in color["fruits"]) for color in result.data[query]
    } == sweet_fruits

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_query_hook_on_relation_keeps_parent_page_full(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    """Test that a relation hook does not drop parents from a page, when none of their related rows match it."""
    result = await maybe_async(any_async_query("{ colorsWithSweetFruitsPaginated(limit: 2) { name fruits { name } } }"))

    assert not result.errors
    assert result.data
    assert result.data["colorsWithSweetFruitsPaginated"] == [
        {"name": "Red", "fruits": [{"name": "Cherry"}]},
        {"name": "Yellow", "fruits": []},
    ]

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_query_hook_on_paginated_relation(
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    raw_colors: RawRecordData,
    raw_fruits: RawRecordData,
) -> None:
    """Test that a relation hook filters the related rows before their pagination applies."""
    result = await maybe_async(any_async_query("{ colorsWithPaginatedSweetFruits { name fruits(limit: 1) { name } } }"))

    assert not result.errors
    assert result.data
    sweet_fruits = {
        color["name"]: {
            fruit["name"] for fruit in raw_fruits if fruit["color_id"] == color["id"] and fruit["sweetness"] > 5
        }
        for color in raw_colors
    }
    fruits_by_color = {
        color["name"]: [fruit["name"] for fruit in color["fruits"]]
        for color in result.data["colorsWithPaginatedSweetFruits"]
    }
    assert fruits_by_color.keys() == sweet_fruits.keys()
    for name, fruits in fruits_by_color.items():
        assert len(fruits) == min(1, len(sweet_fruits[name]))
        assert set(fruits) <= sweet_fruits[name]

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_query_hook_joining_on_relation(
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    raw_colors: RawRecordData,
) -> None:
    """Test that a relation hook adding a join restricts the related rows only."""
    result = await maybe_async(any_async_query("{ colorsWithMultiFarmFruits { name fruits { name } } }"))

    assert not result.errors
    assert result.data
    assert {
        color["name"]: sorted(fruit["name"] for fruit in color["fruits"])
        for color in result.data["colorsWithMultiFarmFruits"]
    } == {color["name"]: ["Apple", "Cherry"] if color["name"] == "Red" else [] for color in raw_colors}

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


def _fruits_by_water_percent(raw_colors: RawRecordData, raw_fruits: RawRecordData) -> list[tuple[str, list[str]]]:
    return [
        (
            color["name"],
            [
                fruit["name"]
                for fruit in sorted(raw_fruits, key=lambda fruit: fruit["water_percent"])
                if fruit["color_id"] == color["id"]
            ],
        )
        for color in raw_colors
    ]


@pytest.mark.parametrize(
    "fruits_field",
    [
        pytest.param("fruits { name }", id="hook-only"),
        pytest.param("fruits(orderBy: { sweetness: DESC }) { name sweetness }", id="client-order-by"),
    ],
)
@pytest.mark.snapshot
async def test_query_hook_ordering_on_relation(
    fruits_field: str,
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    raw_colors: RawRecordData,
    raw_fruits: RawRecordData,
) -> None:
    """Test that the ORDER BY of a hook on a related type orders the related rows, ahead of the client ordering."""
    result = await maybe_async(any_async_query(f"{{ colorsWithOrderedFruits {{ name {fruits_field} }} }}"))

    assert not result.errors
    assert result.data
    assert [
        (color["name"], [fruit["name"] for fruit in color["fruits"]])
        for color in result.data["colorsWithOrderedFruits"]
    ] == _fruits_by_water_percent(raw_colors, raw_fruits)

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_query_hook_ordering_on_paginated_relation(
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    raw_colors: RawRecordData,
    raw_fruits: RawRecordData,
) -> None:
    """Test that the ORDER BY of a hook on a related type decides which related rows a page keeps."""
    result = await maybe_async(
        any_async_query("{ colorsWithPaginatedOrderedFruits { name fruits(limit: 2, offset: 1) { name } } }")
    )

    assert not result.errors
    assert result.data
    assert [
        (color["name"], [fruit["name"] for fruit in color["fruits"]])
        for color in result.data["colorsWithPaginatedOrderedFruits"]
    ] == [(name, fruits[1:3]) for name, fruits in _fruits_by_water_percent(raw_colors, raw_fruits)]

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize("query", ["colorsWithSweetFruits", "colorsWithSweetFruitsPaginated"])
@pytest.mark.snapshot
async def test_query_hook_on_relation_ignored_by_filter(
    query: str,
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    raw_colors: RawRecordData,
    raw_fruits: RawRecordData,
) -> None:
    """Test that a filter on a hooked relation considers the related rows the hook leaves out."""
    result = await maybe_async(
        any_async_query(
            f"{{ {query}(filter: {{ fruits: {{ sweetness: {{ lte: 5 }} }} }}) {{ name fruits {{ name }} }} }}"
        )
    )

    assert not result.errors
    assert result.data
    expected = {
        color["name"]: sorted(
            fruit["name"] for fruit in raw_fruits if fruit["color_id"] == color["id"] and fruit["sweetness"] > 5
        )
        for color in raw_colors
        if any(fruit["color_id"] == color["id"] and fruit["sweetness"] <= 5 for fruit in raw_fruits)
    }
    assert {
        color["name"]: sorted(fruit["name"] for fruit in color["fruits"]) for color in result.data[query]
    } == expected

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_query_hook_joining_on_relation_ignored_by_filter(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    """Test that a filter on a relation whose hook adds a join considers the related rows the hook leaves out."""
    result = await maybe_async(
        any_async_query(
            '{ colorsWithMultiFarmFruits(filter: { fruits: { name: { in: ["Apple", "Banana"] } } }) { name fruits { name } } }'
        )
    )

    assert not result.errors
    assert result.data
    assert {
        color["name"]: sorted(fruit["name"] for fruit in color["fruits"])
        for color in result.data["colorsWithMultiFarmFruits"]
    } == {"Red": ["Apple", "Cherry"], "Yellow": []}

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


async def test_order_by_aggregation_of_relation_with_ordering_hook(
    any_async_query: AnyQueryExecutor, raw_fruits: RawRecordData, query_tracker: QueryTracker
) -> None:
    """Test ordering by an aggregate of a to-one relation whose query hook orders its rows."""
    result = await maybe_async(
        any_async_query(
            """
            {
                fruitsWithOrderedColorPaginated(orderBy: { color: { fruitsAggregate: { count: DESC } } }, limit: 3) {
                    id
                    color { id fruitsAggregate { count } }
                }
            }
            """
        )
    )
    assert not result.errors
    assert result.data
    color_of = {fruit["id"]: fruit["color_id"] for fruit in raw_fruits}
    fruit_counts = {color_id: list(color_of.values()).count(color_id) for color_id in color_of.values()}
    fruits = result.data["fruitsWithOrderedColorPaginated"]
    for fruit in fruits:
        assert fruit["color"] == {
            "id": color_of[fruit["id"]],
            "fruitsAggregate": {"count": fruit_counts[color_of[fruit["id"]]]},
        }
    assert [fruit["color"]["fruitsAggregate"]["count"] for fruit in fruits] == sorted(
        (fruit_counts[color_id] for color_id in color_of.values()), reverse=True
    )[:3]
    assert query_tracker.query_count == 1


async def test_order_by_relation_with_ordering_hook(
    any_async_query: AnyQueryExecutor, raw_fruits: RawRecordData, raw_colors: RawRecordData, query_tracker: QueryTracker
) -> None:
    """Test ordering by a column of a to-one relation whose query hook orders its rows."""
    result = await maybe_async(
        any_async_query("{ fruitsWithOrderedColor(orderBy: { color: { name: DESC } }) { id color { id name } } }")
    )
    assert not result.errors
    assert result.data
    color_names = {color["id"]: color["name"] for color in raw_colors}
    fruits = result.data["fruitsWithOrderedColor"]
    assert sorted(fruit["id"] for fruit in fruits) == sorted(fruit["id"] for fruit in raw_fruits)
    for fruit in fruits:
        color_id = next(raw["color_id"] for raw in raw_fruits if raw["id"] == fruit["id"])
        assert fruit["color"] == {"id": color_id, "name": color_names[color_id]}
    names = [fruit["color"]["name"] for fruit in fruits]
    assert names == sorted(names, reverse=True)
    assert query_tracker.query_count == 1


@pytest.mark.parametrize(
    ("query", "expected"),
    [
        pytest.param(
            "colorNameOrderedFruits(orderBy: [{ sweetness: ASC }])",
            [
                "Lemon",
                "Banana",
                "Quince",
                "Apple",
                "Cherry",
                "Watermelon",
                "Pears",
                "Orange",
                "clementine",
                "Cantaloupe",
                "Strawberry",
            ],
            id="plain",
        ),
        pytest.param(
            "colorNameOrderedFruitsPaginated(orderBy: [{ sweetness: ASC }], limit: 3, offset: 2)",
            ["Quince", "Apple", "Cherry"],
            id="paginated",
        ),
        pytest.param(
            "colorNameOrderedFruits(distinctOn: [colorId], orderBy: [{ sweetness: ASC }])",
            ["Lemon", "Apple", "Watermelon", "Orange", "Cantaloupe"],
            id="distinct-on",
        ),
        pytest.param(
            "colorNameOrderedFruitsPaginated(distinctOn: [colorId], orderBy: [{ sweetness: ASC }], limit: 2, offset: 1)",
            ["Apple", "Watermelon"],
            id="distinct-on-paginated",
        ),
    ],
)
@pytest.mark.snapshot
async def test_query_hook_order_by_joined_alias(
    query: str,
    expected: list[str],
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Test that a root query hook ordering by an alias it joins orders by that joined alias."""
    result = await maybe_async(any_async_query(f"{{ {query} {{ name }} }}"))
    assert not result.errors
    assert result.data

    assert [fruit["name"] for fruit in next(iter(result.data.values()))] == expected
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    ("arguments", "expected"),
    [
        pytest.param(
            "orderBy: [{ sweetness: DESC }]",
            {
                "Red": ["Cherry", "Apple"],
                "Yellow": ["Quince", "Banana", "Lemon"],
                "Orange": ["clementine", "Orange"],
                "Green": ["Strawberry", "Cantaloupe"],
                "Pink": ["Pears", "Watermelon"],
            },
            id="order-by",
        ),
        pytest.param(
            "distinctOn: [colorId], orderBy: [{ sweetness: DESC }]",
            {
                "Red": ["Cherry"],
                "Yellow": ["Quince"],
                "Orange": ["clementine"],
                "Green": ["Strawberry"],
                "Pink": ["Pears"],
            },
            id="distinct-on",
        ),
    ],
)
@pytest.mark.snapshot
async def test_nested_query_hook_order_by_joined_alias(
    arguments: str,
    expected: dict[str, list[str]],
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Test that a relation query hook ordering by an alias it joins orders by that joined alias."""
    result = await maybe_async(
        any_async_query(f"{{ colorsWithColorNameOrderedFruits {{ name fruits({arguments}) {{ name }} }} }}")
    )
    assert not result.errors
    assert result.data

    fruits = {
        color["name"]: [fruit["name"] for fruit in color["fruits"]]
        for color in result.data["colorsWithColorNameOrderedFruits"]
    }
    assert fruits == expected
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot
