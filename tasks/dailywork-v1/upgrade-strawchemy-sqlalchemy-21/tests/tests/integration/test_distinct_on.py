from __future__ import annotations

from datetime import datetime
from typing import TYPE_CHECKING, Any

import pytest

from tests.integration.fixtures import QueryTracker
from tests.integration.typing import RawRecordData
from tests.typing import AnyQueryExecutor
from tests.utils import maybe_async

if TYPE_CHECKING:
    from syrupy.assertion import SnapshotAssertion

    from strawchemy import StrawchemyConfig


@pytest.fixture
def raw_users() -> RawRecordData:
    return [
        {"id": 1, "name": "Alice", "group_id": None, "bio": None},
        {"id": 2, "name": "Alice", "group_id": None, "bio": None},
        {"id": 3, "name": "Charlie", "group_id": None, "bio": None},
        {"id": 4, "name": "Charlie", "group_id": None, "bio": None},
        {"id": 5, "name": "Bob", "group_id": None, "bio": None},
    ]


@pytest.fixture
def raw_fruits(raw_colors: RawRecordData) -> RawRecordData:
    red, yellow, green = raw_colors[0]["id"], raw_colors[1]["id"], raw_colors[3]["id"]
    fruits = [
        ("Apple", 4, 0.84, red),
        ("Cherry", 4, 0.93, red),
        ("Plum", 9, 0.8, red),
        ("Banana", 2, 0.75, yellow),
        ("Lemon", 2, 0.88, yellow),
        ("Strawberry", 5, 0.91, green),
    ]
    return [
        {
            "id": index,
            "created_at": datetime.now().replace(second=index, microsecond=0),  # noqa: DTZ005
            "name": name,
            "sweetness": sweetness,
            "water_percent": water_percent,
            "color_id": color_id,
        }
        for index, (name, sweetness, water_percent, color_id) in enumerate(fruits, start=1)
    ]


def _fruits_by_color(data: dict[str, Any]) -> dict[str, list[Any]]:
    return {color["name"]: color["fruits"] for color in data["colorsNestedDistinct"]}


@pytest.mark.parametrize(
    "deterministic_ordering",
    [pytest.param(True, id="deterministic-ordering"), pytest.param(False, id="non-deterministic-ordering")],
)
@pytest.mark.snapshot
async def test_distinct_on(
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    config: StrawchemyConfig,
    deterministic_ordering: bool,
) -> None:
    config.deterministic_ordering = deterministic_ordering
    result = await maybe_async(any_async_query("{ users(distinctOn: [name]) { id name } }"))
    assert not result.errors
    assert result.data

    expected = [{"id": 1, "name": "Alice"}, {"id": 3, "name": "Charlie"}, {"id": 5, "name": "Bob"}]
    assert len(result.data["users"]) == len(expected)
    assert all(user in result.data["users"] for user in expected)

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_distinct_and_order_by(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    result = await maybe_async(
        any_async_query("{ users(distinctOn: [name], orderBy: [{name: ASC}, {id: DESC}]) { id name } }")
    )
    assert not result.errors
    assert result.data

    assert result.data["users"] == [{"id": 2, "name": "Alice"}, {"id": 5, "name": "Bob"}, {"id": 4, "name": "Charlie"}]

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_distinct_on_projects_every_aggregation_order_by_column(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    """Test that an aggregation ORDER BY column is selected even when another aggregation of the node is."""
    result = await maybe_async(
        any_async_query(
            """
            {
                colors(distinctOn: [name], orderBy: [{ name: ASC }, { fruitsAggregate: { sum: { sweetness: ASC } } }]) {
                    name
                    fruitsAggregate { max { sweetness } }
                }
            }
            """
        )
    )
    assert not result.errors
    assert result.data

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    ("arguments", "deterministic_ordering", "expected"),
    [
        pytest.param(
            "orderBy: [{name: ASC}, {id: DESC}], limit: 2",
            False,
            [{"id": 2, "name": "Alice"}, {"id": 5, "name": "Bob"}],
            id="order-by-distinct-columns",
        ),
        pytest.param(
            "limit: 2",
            True,
            [{"id": 1, "name": "Alice"}, {"id": 3, "name": "Charlie"}],
            id="deterministic-ordering",
        ),
        pytest.param(
            "limit: 2, offset: 1",
            True,
            [{"id": 3, "name": "Charlie"}, {"id": 5, "name": "Bob"}],
            id="deterministic-ordering-offset",
        ),
    ],
)
@pytest.mark.snapshot
async def test_distinct_on_paginates_distinct_rows(
    arguments: str,
    deterministic_ordering: bool,
    expected: list[dict[str, object]],
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    config: StrawchemyConfig,
) -> None:
    config.deterministic_ordering = deterministic_ordering
    result = await maybe_async(any_async_query(f"{{ usersPaginated(distinctOn: [name], {arguments}) {{ id name }} }}"))
    assert not result.errors
    assert result.data

    assert result.data["usersPaginated"] == expected

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    "deterministic_ordering",
    [pytest.param(True, id="deterministic-ordering"), pytest.param(False, id="non-deterministic-ordering")],
)
async def test_nested_distinct_on(
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    config: StrawchemyConfig,
    deterministic_ordering: bool,
) -> None:
    config.deterministic_ordering = deterministic_ordering
    result = await maybe_async(
        any_async_query("{ colorsNestedDistinct { name fruits(distinctOn: [sweetness]) { sweetness } } }")
    )
    assert not result.errors
    assert result.data

    fruits = _fruits_by_color(result.data)
    assert {name: sorted(fruit["sweetness"] for fruit in value) for name, value in fruits.items()} == {
        "Red": [4, 9],
        "Yellow": [2],
        "Orange": [],
        "Green": [5],
        "Pink": [],
    }
    assert query_tracker.query_count == 1


@pytest.mark.snapshot
async def test_nested_distinct_on_and_order_by(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    result = await maybe_async(
        any_async_query(
            """
            {
                colorsNestedDistinct {
                    name
                    fruits(distinctOn: [sweetness], orderBy: [{ sweetness: ASC }, { name: DESC }]) { name }
                }
            }
            """
        )
    )
    assert not result.errors
    assert result.data

    fruits = _fruits_by_color(result.data)
    assert fruits["Red"] == [{"name": "Cherry"}, {"name": "Plum"}]
    assert fruits["Yellow"] == [{"name": "Lemon"}]
    assert fruits["Green"] == [{"name": "Strawberry"}]
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


async def test_nested_distinct_on_on_one_alias(any_async_query: AnyQueryExecutor, query_tracker: QueryTracker) -> None:
    """Test that distinct on applied to one alias of a relation leaves another alias of it unaffected."""
    result = await maybe_async(
        any_async_query(
            """
            {
                colorsNestedDistinct {
                    name
                    a: fruits(distinctOn: [sweetness], orderBy: [{ sweetness: ASC }, { name: DESC }]) { name }
                    b: fruits(orderBy: [{ name: ASC }]) { name }
                }
            }
            """
        )
    )
    assert not result.errors
    assert result.data

    colors = {color["name"]: color for color in result.data["colorsNestedDistinct"]}
    assert colors["Red"]["a"] == [{"name": "Cherry"}, {"name": "Plum"}]
    assert colors["Red"]["b"] == [{"name": "Apple"}, {"name": "Cherry"}, {"name": "Plum"}]
    assert colors["Yellow"]["a"] == [{"name": "Lemon"}]
    assert colors["Yellow"]["b"] == [{"name": "Banana"}, {"name": "Lemon"}]
    assert query_tracker.query_count == 1


@pytest.mark.parametrize(
    ("pagination", "expected_red", "expected_yellow"),
    [
        pytest.param("limit: 2", ["Apple", "Plum"], ["Banana"], id="limit"),
        pytest.param("limit: 1, offset: 1", ["Plum"], [], id="limit-offset"),
    ],
)
@pytest.mark.snapshot
async def test_nested_distinct_on_paginated(
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    pagination: str,
    expected_red: list[str],
    expected_yellow: list[str],
) -> None:
    result = await maybe_async(
        any_async_query(
            f"""
            {{
                colorsNestedDistinct {{
                    name
                    fruits(distinctOn: [sweetness], orderBy: [{{ sweetness: ASC }}, {{ name: ASC }}], {pagination}) {{
                        name
                    }}
                }}
            }}
            """
        )
    )
    assert not result.errors
    assert result.data

    fruits = _fruits_by_color(result.data)
    assert [fruit["name"] for fruit in fruits["Red"]] == expected_red
    assert [fruit["name"] for fruit in fruits["Yellow"]] == expected_yellow
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    ("pagination", "expected_red", "expected_yellow"),
    [
        pytest.param("limit: 1", ["Apple"], ["Banana"], id="limit"),
        pytest.param("limit: 1, offset: 1", ["Plum"], [], id="limit-offset"),
    ],
)
async def test_nested_distinct_on_paginated_by_primary_key(
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    config: StrawchemyConfig,
    pagination: str,
    expected_red: list[str],
    expected_yellow: list[str],
) -> None:
    config.deterministic_ordering = True
    result = await maybe_async(
        any_async_query(
            f"{{ colorsNestedDistinct {{ name fruits(distinctOn: [sweetness], {pagination}) {{ name }} }} }}"
        )
    )
    assert not result.errors
    assert result.data

    fruits = _fruits_by_color(result.data)
    assert [fruit["name"] for fruit in fruits["Red"]] == expected_red
    assert [fruit["name"] for fruit in fruits["Yellow"]] == expected_yellow
    assert query_tracker.query_count == 1


@pytest.mark.parametrize(
    ("query", "field", "expected"),
    [
        pytest.param(
            "{ colors(distinctOn: [name], orderBy: [{ name: ASC }]) { name fruits { name } } }",
            "colors",
            {
                "Green": ["Strawberry"],
                "Orange": [],
                "Pink": [],
                "Red": ["Apple", "Cherry", "Plum"],
                "Yellow": ["Banana", "Lemon"],
            },
            id="to-many",
        ),
        pytest.param(
            """
            {
                colorsFilteredDistinct(distinctOn: [name], orderBy: [{ name: ASC }], limit: 2, offset: 1) {
                    name
                    fruits { name }
                }
            }
            """,
            "colorsFilteredDistinct",
            {"Pink": [], "Red": ["Apple", "Cherry", "Plum"]},
            id="paginated",
        ),
    ],
)
@pytest.mark.snapshot
async def test_distinct_on_keeps_every_child_of_a_to_many_relation(
    query: str,
    field: str,
    expected: dict[str, list[str]],
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Test that a root distinctOn deduplicates the root rows only, keeping every child of a to-many relation."""
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    children = {color["name"]: sorted(fruit["name"] for fruit in color["fruits"]) for color in result.data[field]}
    assert children == expected
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_distinct_on_keeps_every_child_of_a_to_many_relation_behind_a_to_one(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    """Test that a root distinctOn keeps every child of a to-many relation selected through a to-one relation."""
    result = await maybe_async(
        any_async_query(
            "{ fruitsDefaultOrderDistinct(distinctOn: [name], orderBy: [{ name: ASC }]) { name color { fruits { name } } } }"
        )
    )
    assert not result.errors
    assert result.data

    children = {
        fruit["name"]: sorted(child["name"] for child in fruit["color"]["fruits"])
        for fruit in result.data["fruitsDefaultOrderDistinct"]
    }
    assert children == {
        "Apple": ["Apple", "Cherry", "Plum"],
        "Banana": ["Banana", "Lemon"],
        "Cherry": ["Apple", "Cherry", "Plum"],
        "Lemon": ["Banana", "Lemon"],
        "Plum": ["Apple", "Cherry", "Plum"],
        "Strawberry": ["Strawberry"],
    }
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_distinct_on_deduplicates_root_rows_selecting_a_to_many_relation(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    """Test that a root distinctOn drops duplicate root rows and keeps every child of the selected to-many relation."""
    result = await maybe_async(
        any_async_query(
            """
            {
                fruitsDefaultOrderDistinct(distinctOn: [sweetness], orderBy: [{ sweetness: ASC }, { name: ASC }]) {
                    name
                    color { fruits { name } }
                }
            }
            """
        )
    )
    assert not result.errors
    assert result.data

    fruits = result.data["fruitsDefaultOrderDistinct"]
    assert [fruit["name"] for fruit in fruits] == ["Banana", "Apple", "Strawberry", "Plum"]
    assert {fruit["name"]: sorted(child["name"] for child in fruit["color"]["fruits"]) for fruit in fruits} == {
        "Banana": ["Banana", "Lemon"],
        "Apple": ["Apple", "Cherry", "Plum"],
        "Strawberry": ["Strawberry"],
        "Plum": ["Apple", "Cherry", "Plum"],
    }
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_distinct_on_ordered_by_an_unselected_to_many_relation(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    """Test that a root distinctOn ordered by a to-many relation it does not select returns one row per group."""
    result = await maybe_async(
        any_async_query(
            "{ colors(distinctOn: [name], orderBy: [{ name: ASC }, { fruits: { name: DESC } }]) { id name } }"
        )
    )
    assert not result.errors
    assert result.data

    assert [color["name"] for color in result.data["colors"]] == ["Green", "Orange", "Pink", "Red", "Yellow"]
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    ("query", "expected"),
    [
        pytest.param(
            "{ users(distinctOn: [name], orderBy: [{ id: DESC }]) { id name } }",
            [{"id": 5, "name": "Bob"}, {"id": 4, "name": "Charlie"}, {"id": 2, "name": "Alice"}],
            id="root",
        ),
        pytest.param(
            "{ users(distinctOn: [name, groupId], orderBy: [{ name: ASC }, { id: DESC }]) { id name } }",
            [{"id": 2, "name": "Alice"}, {"id": 5, "name": "Bob"}, {"id": 4, "name": "Charlie"}],
            id="partial-prefix",
        ),
        pytest.param(
            "{ usersPaginated(distinctOn: [name], orderBy: [{ id: DESC }], limit: 2) { id name } }",
            [{"id": 5, "name": "Bob"}, {"id": 4, "name": "Charlie"}],
            id="paginated",
        ),
        pytest.param(
            "{ usersPaginated(distinctOn: [name], orderBy: [{ id: DESC }], limit: 2, offset: 1) { id name } }",
            [{"id": 4, "name": "Charlie"}, {"id": 2, "name": "Alice"}],
            id="paginated-offset",
        ),
    ],
)
@pytest.mark.snapshot
async def test_distinct_on_ordered_by_other_fields(
    query: str,
    expected: list[dict[str, Any]],
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Test that distinctOn keeps the first row of each group by an order by not starting with the distinct fields."""
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    assert next(iter(result.data.values())) == expected
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


async def test_distinct_on_ordered_by_tied_fields(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker
) -> None:
    """Test that distinctOn keeps one row per group when the order by ties within the group."""
    result = await maybe_async(any_async_query("{ users(distinctOn: [name], orderBy: [{ bio: ASC }]) { id name } }"))
    assert not result.errors
    assert result.data

    users = result.data["users"]
    assert sorted(user["name"] for user in users) == ["Alice", "Bob", "Charlie"]
    ids_by_name = {"Alice": {1, 2}, "Bob": {5}, "Charlie": {3, 4}}
    assert all(user["id"] in ids_by_name[user["name"]] for user in users)
    assert query_tracker.query_count == 1


@pytest.mark.snapshot
async def test_distinct_on_ordered_by_other_fields_selecting_a_to_many_relation(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    """Test that a root distinctOn ordered by other fields keeps every child of a to-many relation behind a to-one."""
    result = await maybe_async(
        any_async_query(
            """
            {
                fruitsDefaultOrderDistinct(distinctOn: [colorId], orderBy: [{ name: DESC }]) {
                    name
                    color { fruits { name } }
                }
            }
            """
        )
    )
    assert not result.errors
    assert result.data

    fruits = result.data["fruitsDefaultOrderDistinct"]
    assert [fruit["name"] for fruit in fruits] == ["Strawberry", "Plum", "Lemon"]
    assert {fruit["name"]: sorted(child["name"] for child in fruit["color"]["fruits"]) for fruit in fruits} == {
        "Strawberry": ["Strawberry"],
        "Plum": ["Apple", "Cherry", "Plum"],
        "Lemon": ["Banana", "Lemon"],
    }
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_distinct_on_ordered_by_an_aggregation(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    """Test that distinctOn keeps the first row of each group by an order by starting with an aggregation."""
    result = await maybe_async(
        any_async_query(
            """
            {
                fruitsDefaultOrderDistinct(
                    distinctOn: [colorId], orderBy: [{ farmsAggregate: { count: DESC } }, { name: ASC }]
                ) {
                    name
                }
            }
            """
        )
    )
    assert not result.errors
    assert result.data

    assert [fruit["name"] for fruit in result.data["fruitsDefaultOrderDistinct"]] == ["Apple", "Banana", "Strawberry"]
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    ("arguments", "expected_red", "expected_yellow"),
    [
        pytest.param("orderBy: [{ name: DESC }]", ["Plum", "Cherry"], ["Lemon"], id="order-by"),
        pytest.param("orderBy: [{ waterPercent: ASC }]", ["Plum", "Apple"], ["Banana"], id="order-by-other-column"),
        pytest.param("orderBy: [{ name: DESC }], limit: 1", ["Plum"], ["Lemon"], id="limit"),
        pytest.param("orderBy: [{ name: DESC }], limit: 1, offset: 1", ["Cherry"], [], id="limit-offset"),
    ],
)
@pytest.mark.snapshot
async def test_nested_distinct_on_ordered_by_other_fields(
    arguments: str,
    expected_red: list[str],
    expected_yellow: list[str],
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Test that a nested distinctOn keeps the first row of each group by an order by not starting with it."""
    result = await maybe_async(
        any_async_query(
            f"{{ colorsNestedDistinct {{ name fruits(distinctOn: [sweetness], {arguments}) {{ name }} }} }}"
        )
    )
    assert not result.errors
    assert result.data

    fruits = _fruits_by_color(result.data)
    assert [fruit["name"] for fruit in fruits["Red"]] == expected_red
    assert [fruit["name"] for fruit in fruits["Yellow"]] == expected_yellow
    assert fruits["Orange"] == []
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    "arguments",
    [
        pytest.param("distinctOn: [sweetness]", id="no-order-by"),
        pytest.param("distinctOn: [sweetness], orderBy: [{ sweetness: ASC }]", id="order-by-prefix"),
        pytest.param("distinctOn: [sweetness], orderBy: [{ name: DESC }]", id="order-by-other-fields"),
    ],
)
@pytest.mark.snapshot
async def test_nested_distinct_on_with_hook_order_by(
    arguments: str, any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    """Test that a nested distinctOn keeps the first row of each group by the hook ordering, then the client's."""
    result = await maybe_async(
        any_async_query(f"{{ colorsWithOrderedFruits {{ name fruits({arguments}) {{ name }} }} }}")
    )
    assert not result.errors
    assert result.data

    fruits = {
        color["name"]: [fruit["name"] for fruit in color["fruits"]] for color in result.data["colorsWithOrderedFruits"]
    }
    assert fruits == {"Red": ["Plum", "Apple"], "Yellow": ["Banana"], "Orange": [], "Green": ["Strawberry"], "Pink": []}
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    ("query", "expected"),
    [
        pytest.param("orderedFruits(distinctOn: [colorId])", ["Banana", "Plum", "Strawberry"], id="no-order-by"),
        pytest.param(
            "orderedFruits(distinctOn: [colorId], orderBy: [{ colorId: ASC }])",
            ["Banana", "Plum", "Strawberry"],
            id="order-by-prefix",
        ),
        pytest.param(
            "orderedFruits(distinctOn: [colorId], orderBy: [{ name: DESC }])",
            ["Banana", "Plum", "Strawberry"],
            id="order-by-other-fields",
        ),
        pytest.param(
            "orderedFruitsPaginated(distinctOn: [colorId], limit: 2)", ["Banana", "Plum"], id="paginated-no-order-by"
        ),
        pytest.param(
            "orderedFruitsPaginated(distinctOn: [colorId], orderBy: [{ colorId: ASC }], limit: 2)",
            ["Banana", "Plum"],
            id="paginated-order-by-prefix",
        ),
        pytest.param(
            "orderedFruitsPaginated(distinctOn: [colorId], orderBy: [{ name: DESC }], limit: 2, offset: 1)",
            ["Plum", "Strawberry"],
            id="paginated-order-by-other-fields",
        ),
    ],
)
@pytest.mark.snapshot
async def test_distinct_on_with_hook_order_by(
    query: str,
    expected: list[str],
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Test that a root distinctOn keeps the first row of each group by the hook ordering, then the client's."""
    result = await maybe_async(any_async_query(f"{{ {query} {{ name }} }}"))
    assert not result.errors
    assert result.data

    assert [fruit["name"] for fruit in next(iter(result.data.values()))] == expected
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot
