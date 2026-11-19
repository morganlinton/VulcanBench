from __future__ import annotations

from typing import TYPE_CHECKING, Any

import pytest
from sqlalchemy import select
from sqlalchemy.orm import selectinload

from tests.integration.models import Color, Fruit
from tests.integration.utils import to_graphql_representation
from tests.utils import maybe_async

if TYPE_CHECKING:
    from collections.abc import Sequence

    from strawchemy.repository.typing import AnySession
    from tests.integration.fixtures import QueryTracker
    from tests.integration.typing import RawRecordData
    from tests.typing import AnyQueryExecutor

pytestmark = [pytest.mark.integration]


async def _data(any_async_query: AnyQueryExecutor, query: str) -> dict[str, Any]:
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    return result.data


def _sweetness_of(raw_fruits: RawRecordData, color_id: int, *, descending: bool = False) -> list[int]:
    return sorted((fruit["sweetness"] for fruit in raw_fruits if fruit["color_id"] == color_id), reverse=descending)


async def _load_colors_with_fruits(any_async_session: AnySession) -> Sequence[Color]:
    result = await maybe_async(any_async_session.execute(select(Color).options(selectinload(Color.fruits))))
    return result.scalars().all()


async def test_nested_order_by_ignores_collection_loaded_in_session(
    any_async_query: AnyQueryExecutor,
    any_async_session: AnySession,
    query_tracker: QueryTracker,
    raw_fruits: RawRecordData,
) -> None:
    """Test that a nested relation is ordered as asked when the session already holds that collection."""
    await _data(any_async_query, "{ colors { id } }")
    kept = await _load_colors_with_fruits(any_async_session)
    kept_fruits = {color.id: [fruit.id for fruit in color.fruits] for color in kept}
    query_tracker.executions.clear()

    data = await _data(any_async_query, "{ colors { id fruits(orderBy: { sweetness: DESC }) { sweetness } } }")

    assert query_tracker.query_count == 1
    for color in data["colors"]:
        expected = _sweetness_of(raw_fruits, color["id"], descending=True)
        assert [fruit["sweetness"] for fruit in color["fruits"]] == expected
    assert {color.id: [fruit.id for fruit in color.fruits] for color in kept} == kept_fruits


async def test_nested_pagination_ignores_collection_loaded_in_session(
    any_async_query: AnyQueryExecutor, any_async_session: AnySession, raw_fruits: RawRecordData
) -> None:
    """Test that a nested relation is paginated as asked when the session already holds that collection."""
    kept = await _load_colors_with_fruits(any_async_session)

    data = await _data(
        any_async_query,
        "{ colorsPaginated { id fruits(orderBy: { sweetness: DESC }, limit: 1, offset: 1) { sweetness } } }",
    )

    assert kept
    for color in data["colorsPaginated"]:
        expected = _sweetness_of(raw_fruits, color["id"], descending=True)[1:2]
        assert [fruit["sweetness"] for fruit in color["fruits"]] == expected


async def test_nested_filter_ignores_collection_loaded_in_session(
    any_async_query: AnyQueryExecutor, any_async_session: AnySession, raw_fruits: RawRecordData
) -> None:
    """Test that a nested relation is filtered as asked when the session already holds that collection."""
    kept = await _load_colors_with_fruits(any_async_session)

    data = await _data(any_async_query, "{ colorsWithSweetFruits { id fruits { sweetness } } }")

    assert kept
    for color in data["colorsWithSweetFruits"]:
        expected = [sweetness for sweetness in _sweetness_of(raw_fruits, color["id"]) if sweetness > 5]
        assert sorted(fruit["sweetness"] for fruit in color["fruits"]) == expected


async def test_to_one_relation_ignores_stale_attribute_loaded_in_session(
    any_query: AnyQueryExecutor,
    any_session: AnySession,
    raw_fruits: RawRecordData,
    raw_colors: RawRecordData,
) -> None:
    """Test that a to-one relation follows the foreign key when the session holds the previously related object."""
    fruit_id = raw_fruits[0]["id"]
    new_color_id = next(color["id"] for color in raw_colors if color["id"] != raw_fruits[0]["color_id"])
    result = await maybe_async(
        any_session.execute(select(Fruit).options(selectinload(Fruit.color)).where(Fruit.id == fruit_id))
    )
    fruit = result.scalar_one()
    fruit.color_id = new_color_id
    await maybe_async(any_session.flush())

    data = await _data(any_query, "{ fruits { id color { id } } }")

    assert next(item for item in data["fruits"] if item["id"] == fruit_id)["color"] == {"id": new_color_id}


@pytest.mark.allow_duplicate_reads(
    reason=(
        "on postgresql, colors.fruits (sweetness DESC) and colors.fruits.color.fruits (sweetness ASC) each read "
        "fruit through their own LATERAL, two-path selections staying separate there; the CTE databases merge "
        "their rank CTEs"
    ),
    dialects=("postgresql",),
)
async def test_same_object_reached_through_two_paths_keeps_each_path_arguments(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, raw_fruits: RawRecordData
) -> None:
    """Test that a model reached through two selection paths with different arguments gets each path's collection."""
    data = await _data(
        any_async_query,
        """
        {
            colors {
                id
                fruits(orderBy: { sweetness: DESC }) {
                    sweetness
                    color { id fruits(orderBy: { sweetness: ASC }) { sweetness } }
                }
            }
        }
        """,
    )

    assert query_tracker.query_count == 1
    for color in data["colors"]:
        assert [fruit["sweetness"] for fruit in color["fruits"]] == _sweetness_of(
            raw_fruits, color["id"], descending=True
        )
        for fruit in color["fruits"]:
            nested = fruit["color"]
            assert [related["sweetness"] for related in nested["fruits"]] == _sweetness_of(raw_fruits, nested["id"])


async def test_same_object_reached_through_two_paths_keeps_each_path_filter(
    any_async_query: AnyQueryExecutor, raw_fruits: RawRecordData
) -> None:
    """Test that a model reached through a filtered and an unfiltered selection path gets each path's collection."""
    data = await _data(
        any_async_query,
        """
        {
            colorsWithSweetFruits {
                id
                fruits { sweetness color { id fruits { sweetness } } }
            }
        }
        """,
    )

    for color in data["colorsWithSweetFruits"]:
        ascending = _sweetness_of(raw_fruits, color["id"])
        assert sorted(fruit["sweetness"] for fruit in color["fruits"]) == [value for value in ascending if value > 5]
        for fruit in color["fruits"]:
            assert sorted(related["sweetness"] for related in fruit["color"]["fruits"]) == ascending


async def test_relation_selected_and_loaded_by_query_hook(
    any_async_query: AnyQueryExecutor, raw_fruits: RawRecordData, raw_farms: RawRecordData, raw_colors: RawRecordData
) -> None:
    """Test that a relation both selected and loaded by a query hook gets the selection's rows and the hook's."""
    data = await _data(any_async_query, "{ colorsWithFilteredFruits { id fruits { name } farms } }")

    assert {color["id"] for color in data["colorsWithFilteredFruits"]} == {color["id"] for color in raw_colors}
    for color in data["colorsWithFilteredFruits"]:
        fruits = [fruit for fruit in raw_fruits if fruit["color_id"] == color["id"]]
        fruit_ids = {fruit["id"] for fruit in fruits}
        farm_names = sorted(farm["name"] for farm in raw_farms if farm["fruit_id"] in fruit_ids)
        assert color["fruits"] == [{"name": fruit["name"]} for fruit in fruits if fruit["name"] == "Apple"]
        assert sorted(color["farms"].removeprefix("Farms are: ").split(", ")) == farm_names


async def test_mutation_result_ignores_collection_loaded_in_session(
    any_async_query: AnyQueryExecutor,
    any_async_session: AnySession,
    raw_fruits: RawRecordData,
    raw_colors: RawRecordData,
) -> None:
    """Test that a mutation result lists the created object when the session already holds the parent collection."""
    color_id = raw_colors[0]["id"]
    kept = await _load_colors_with_fruits(any_async_session)

    data = await _data(
        any_async_query,
        f"""
        mutation {{
            createFruit(data: {{
                name: "new fruit",
                sweetness: 1,
                waterPercent: 0.8,
                color: {{ set: {{ id: {to_graphql_representation(color_id, "input")} }} }}
            }}) {{
                color {{ fruits {{ name }} }}
            }}
        }}
        """,
    )

    assert kept
    expected = sorted([*(fruit["name"] for fruit in raw_fruits if fruit["color_id"] == color_id), "new fruit"])
    assert sorted(fruit["name"] for fruit in data["createFruit"]["color"]["fruits"]) == expected
