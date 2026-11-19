from __future__ import annotations

from collections import Counter
from typing import TYPE_CHECKING, Any

import pytest
from sqlalchemy import insert, update

from tests.integration.fixtures import QueryTracker
from tests.integration.models import DerivedProduct, Fruit
from tests.integration.typing import RawRecordData
from tests.integration.utils import to_graphql_representation
from tests.typing import AnyQueryExecutor
from tests.utils import maybe_async

if TYPE_CHECKING:
    from strawchemy.repository.typing import AnySession

pytestmark = [pytest.mark.integration]


async def _data(any_async_query: AnyQueryExecutor, query: str) -> dict[str, Any]:
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    return result.data


def _fruits_of(
    raw_fruits: RawRecordData, color_id: int, *, by: str = "id", descending: bool = False
) -> list[dict[str, Any]]:
    fruits = [fruit for fruit in raw_fruits if fruit["color_id"] == color_id]
    return sorted(fruits, key=lambda fruit: fruit[by], reverse=descending)


def _ids_by_color(colors: list[dict[str, Any]], *aliases: str) -> dict[int, tuple[list[int], ...]]:
    return {color["id"]: tuple([fruit["id"] for fruit in color[alias]] for alias in aliases) for color in colors}


async def test_aliased_relations_with_different_order_by(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, raw_fruits: RawRecordData
) -> None:
    """Test that two aliases of a relation ordered differently are each ordered their own way, in one query."""
    data = await _data(
        any_async_query,
        """
        {
            colors {
                id
                a: fruits(orderBy: { sweetness: DESC }) { id }
                b: fruits(orderBy: { sweetness: ASC }) { id }
            }
        }
        """,
    )
    assert query_tracker.query_count == 1
    for color in data["colors"]:
        ascending = [{"id": fruit["id"]} for fruit in _fruits_of(raw_fruits, color["id"], by="sweetness")]
        assert color["a"] == ascending[::-1]
        assert color["b"] == ascending


async def test_aliased_relations_with_different_pagination(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, raw_fruits: RawRecordData
) -> None:
    """Test that two aliases of a relation paginated differently each get their own page, in one query."""
    data = await _data(
        any_async_query,
        """
        {
            colorsPaginated(limit: 3, offset: 1) {
                id
                a: fruits(limit: 1) { id }
                b: fruits(limit: 1, offset: 1) { id }
                c: fruits { id }
            }
        }
        """,
    )
    assert query_tracker.query_count == 1
    assert [color["id"] for color in data["colorsPaginated"]] == [2, 3, 4]
    for color in data["colorsPaginated"]:
        fruits = [{"id": fruit["id"]} for fruit in _fruits_of(raw_fruits, color["id"])]
        assert color["a"] == fruits[:1]
        assert color["b"] == fruits[1:2]
        assert color["c"] == fruits


async def test_aliased_relations_with_different_arguments_and_selections(
    any_async_query: AnyQueryExecutor, raw_fruits: RawRecordData, raw_colors: RawRecordData
) -> None:
    """Test that aliases with different arguments keep their own selection, sub-relations included."""
    data = await _data(
        any_async_query,
        """
        {
            colors {
                id
                a: fruits(orderBy: { sweetness: DESC }) { name }
                b: fruits(orderBy: { sweetness: ASC }) { id sweetness color { name } }
            }
        }
        """,
    )
    color_names = {color["id"]: color["name"] for color in raw_colors}
    for color in data["colors"]:
        ascending = _fruits_of(raw_fruits, color["id"], by="sweetness")
        assert color["a"] == [{"name": fruit["name"]} for fruit in reversed(ascending)]
        assert color["b"] == [
            {"id": fruit["id"], "sweetness": fruit["sweetness"], "color": {"name": color_names[color["id"]]}}
            for fruit in ascending
        ]


async def test_aliased_relations_with_and_without_arguments(
    any_async_query: AnyQueryExecutor, raw_fruits: RawRecordData
) -> None:
    """Test that an alias without arguments is unaffected by another alias of the same relation that has some."""
    data = await _data(
        any_async_query,
        """
        {
            colors {
                id
                a: fruits { id }
                b: fruits(orderBy: { sweetness: DESC }) { id }
            }
        }
        """,
    )
    for color in data["colors"]:
        assert color["a"] == [{"id": fruit["id"]} for fruit in _fruits_of(raw_fruits, color["id"])]
        assert color["b"] == [
            {"id": fruit["id"]} for fruit in _fruits_of(raw_fruits, color["id"], by="sweetness", descending=True)
        ]


@pytest.mark.allow_duplicate_reads(
    reason=(
        "the root reads fruit, and the shared read of aliases a and b of fruits.color.fruits reads the same fruit "
        "again through the color round trip; round trips are never detected as the same rows"
    ),
    dialects=("sqlite", "mysql"),
)
async def test_aliased_relations_nested_in_relation(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, raw_fruits: RawRecordData
) -> None:
    """Test that aliases of a relation with different arguments, below another relation, are each ordered."""
    data = await _data(
        any_async_query,
        """
        {
            fruits {
                id
                colorId
                color {
                    a: fruits(orderBy: { sweetness: DESC }) { id }
                    b: fruits(orderBy: { sweetness: ASC }) { id }
                }
            }
        }
        """,
    )
    assert query_tracker.query_count == 1
    for fruit in data["fruits"]:
        ascending = [{"id": related["id"]} for related in _fruits_of(raw_fruits, fruit["colorId"], by="sweetness")]
        assert fruit["color"]["a"] == ascending[::-1]
        assert fruit["color"]["b"] == ascending


async def test_aliased_relations_with_same_arguments_share_one_join(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, raw_fruits: RawRecordData
) -> None:
    """Test that aliases of a relation with the same arguments share one join and both get the merged selection."""
    data = await _data(
        any_async_query,
        """
        {
            colors {
                id
                a: fruits(orderBy: { sweetness: DESC }) { id }
                b: fruits(orderBy: { sweetness: DESC }) { id sweetness }
            }
        }
        """,
    )
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted.count("JOIN") == 1
    for color in data["colors"]:
        descending = _fruits_of(raw_fruits, color["id"], by="sweetness", descending=True)
        assert color["a"] == [{"id": fruit["id"]} for fruit in descending]
        assert color["b"] == [{"id": fruit["id"], "sweetness": fruit["sweetness"]} for fruit in descending]


async def test_aliased_relations_with_computed_values(
    any_async_query: AnyQueryExecutor, raw_fruits: RawRecordData, raw_farms: RawRecordData
) -> None:
    """Test that computed values under one alias still map to the right objects when another alias has arguments."""
    data = await _data(
        any_async_query,
        """
        {
            colors {
                id
                a: fruits { id farmsAggregate { count } }
                b: fruits(orderBy: { sweetness: DESC }) { id }
            }
        }
        """,
    )
    farm_counts = Counter(farm["fruit_id"] for farm in raw_farms)
    for color in data["colors"]:
        assert color["a"] == [
            {"id": fruit["id"], "farmsAggregate": {"count": farm_counts[fruit["id"]]}}
            for fruit in _fruits_of(raw_fruits, color["id"])
        ]
        assert color["b"] == [
            {"id": fruit["id"]} for fruit in _fruits_of(raw_fruits, color["id"], by="sweetness", descending=True)
        ]


async def test_aliased_relations_with_query_hook(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, raw_fruits: RawRecordData
) -> None:
    """Test that a relation query hook applies to every alias of the relation, each keeping its own page."""
    data = await _data(
        any_async_query,
        """
        {
            colorsWithPaginatedSweetFruits {
                id
                a: fruits(limit: 1) { id }
                b: fruits(limit: 1, offset: 1) { id }
            }
        }
        """,
    )
    assert query_tracker.query_count == 1
    for color in data["colorsWithPaginatedSweetFruits"]:
        sweet = [{"id": fruit["id"]} for fruit in _fruits_of(raw_fruits, color["id"]) if fruit["sweetness"] > 5]
        assert color["a"] == sweet[:1]
        assert color["b"] == sweet[1:2]


@pytest.mark.allow_duplicate_reads(
    reason=(
        "the root reads fruit, and aliases a and b of fruits.color.fruits, under the merged color aliases x and y, "
        "read the same fruit again through the color round trip; round trips are never detected as the same rows"
    ),
    dialects=("sqlite", "mysql"),
)
async def test_aliased_relations_under_to_one_relation_selected_twice(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, raw_fruits: RawRecordData
) -> None:
    """Test that aliases of a to-one relation merge, and the differently-argued relations below each keep theirs."""
    data = await _data(
        any_async_query,
        """
        {
            fruits {
                colorId
                x: color { a: fruits(orderBy: { sweetness: DESC }) { id } }
                y: color { b: fruits(orderBy: { sweetness: ASC }) { id } }
            }
        }
        """,
    )
    assert query_tracker.query_count == 1
    for fruit in data["fruits"]:
        ascending = [{"id": related["id"]} for related in _fruits_of(raw_fruits, fruit["colorId"], by="sweetness")]
        assert fruit["x"] == {"a": ascending[::-1]}
        assert fruit["y"] == {"b": ascending}


async def test_mutation_returning_aliased_relations(
    any_async_query: AnyQueryExecutor, raw_colors: RawRecordData, raw_fruits: RawRecordData
) -> None:
    """Test that a mutation result selecting aliases of a relation with different arguments returns each alias."""
    color_id = raw_colors[0]["id"]
    data = await _data(
        any_async_query,
        f"""
        mutation {{
            updateColor(data: {{ id: {to_graphql_representation(color_id, "input")}, name: "updated" }}) {{
                name
                a: fruits(orderBy: {{ sweetness: DESC }}) {{ id }}
                b: fruits(orderBy: {{ sweetness: ASC }}) {{ id }}
            }}
        }}
        """,
    )
    ascending = [{"id": fruit["id"]} for fruit in _fruits_of(raw_fruits, color_id, by="sweetness")]
    assert data["updateColor"] == {"name": "updated", "a": ascending[::-1], "b": ascending}


async def test_aliased_relations_with_same_query_hook_apply_it_once(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, raw_fruits: RawRecordData
) -> None:
    """Test that aliases of a hooked relation sharing one node run the relation's query hook only once."""
    data = await _data(
        any_async_query,
        """
        {
            colorsWithSweetFruits {
                id
                a: fruits { id }
                b: fruits { name }
            }
        }
        """,
    )
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted.count("> ") == 1
    for color in data["colorsWithSweetFruits"]:
        sweet = [fruit for fruit in _fruits_of(raw_fruits, color["id"]) if fruit["sweetness"] > 5]
        assert color["a"] == [{"id": fruit["id"]} for fruit in sweet]
        assert color["b"] == [{"name": fruit["name"]} for fruit in sweet]


async def test_aliased_relations_queried_twice(
    any_async_query: AnyQueryExecutor, raw_fruits: RawRecordData, raw_farms: RawRecordData
) -> None:
    """Test that an aliased query runs again once its compiled statement is cached."""
    query = """
        {
            colors {
                id
                a: fruits { id farmsAggregate { count } }
                b: fruits(orderBy: { sweetness: DESC }) { id }
            }
        }
    """
    farm_counts = Counter(farm["fruit_id"] for farm in raw_farms)
    for _ in range(2):
        data = await _data(any_async_query, query)
        for color in data["colors"]:
            assert color["a"] == [
                {"id": fruit["id"], "farmsAggregate": {"count": farm_counts[fruit["id"]]}}
                for fruit in _fruits_of(raw_fruits, color["id"])
            ]
            assert color["b"] == [
                {"id": fruit["id"]} for fruit in _fruits_of(raw_fruits, color["id"], by="sweetness", descending=True)
            ]


async def test_aliases_mixing_bounded_and_unbounded(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker
) -> None:
    """Test that two pages next to an unbounded alias each keep their own rows, in their own order."""
    data = await _data(
        any_async_query,
        """
        {
            colorsPaginated {
                id
                a: fruits(orderBy: { sweetness: DESC }, limit: 1) { id }
                b: fruits(orderBy: { sweetness: ASC }, limit: 1, offset: 1) { id }
                c: fruits(orderBy: { id: DESC }, limit: null) { id }
            }
        }
        """,
    )
    assert query_tracker.query_count == 1
    assert _ids_by_color(data["colorsPaginated"], "a", "b", "c") == {
        1: ([2], [2], [2, 1]),
        2: ([5], [3], [5, 4, 3]),
        3: ([7], [7], [7, 6]),
        4: ([8], [8], [9, 8]),
        5: ([11], [11], [11, 10]),
    }


async def test_alias_with_offset_page(any_async_query: AnyQueryExecutor, query_tracker: QueryTracker) -> None:
    """Test that an alias with an offset and no limit keeps every row after the offset."""
    data = await _data(
        any_async_query,
        """
        {
            colorsPaginated {
                id
                a: fruits(orderBy: { sweetness: ASC }, offset: 1, limit: null) { id }
                b: fruits(orderBy: { sweetness: DESC }, limit: 1) { id }
            }
        }
        """,
    )
    assert query_tracker.query_count == 1
    assert _ids_by_color(data["colorsPaginated"], "a", "b") == {
        1: ([2], [2]),
        2: ([3, 5], [5]),
        3: ([7], [7]),
        4: ([8], [8]),
        5: ([11], [11]),
    }


async def test_aliases_ordered_with_nulls(
    any_async_query: AnyQueryExecutor, any_async_session: AnySession, query_tracker: QueryTracker
) -> None:
    """Test that aliases ordered on a column holding NULL values place them as each alias asks."""
    products = [{"id": 1, "name": "Jam"}, {"id": 2, "name": "Juice"}]
    await maybe_async(any_async_session.execute(insert(DerivedProduct).values(products)))
    for fruit_id, product_id in ((1, 1), (3, 2), (5, 1)):
        statement = update(Fruit).where(Fruit.id == fruit_id).values(derived_product_id=product_id)
        await maybe_async(any_async_session.execute(statement))
    await maybe_async(any_async_session.flush())
    query_tracker.executions.clear()

    data = await _data(
        any_async_query,
        """
        {
            colors {
                id
                a: fruits(orderBy: { derivedProductId: ASC_NULLS_FIRST }) { id }
                b: fruits(orderBy: { derivedProductId: ASC_NULLS_LAST }) { id }
            }
        }
        """,
    )
    assert query_tracker.query_count == 1
    assert _ids_by_color(data["colors"], "a", "b") == {
        1: ([2, 1], [1, 2]),
        2: ([4, 5, 3], [5, 3, 4]),
        3: ([6, 7], [6, 7]),
        4: ([8, 9], [8, 9]),
        5: ([10, 11], [10, 11]),
    }


async def test_nested_to_one_under_one_alias(any_async_query: AnyQueryExecutor, query_tracker: QueryTracker) -> None:
    """Test that a to-one relation selected under one alias only reaches that alias's objects."""
    data = await _data(
        any_async_query,
        """
        {
            colors {
                id
                a: fruits(orderBy: { sweetness: ASC }) { id color { name } }
                b: fruits(orderBy: { sweetness: DESC }) { id }
            }
        }
        """,
    )
    assert query_tracker.query_count == 1
    names = {1: "Red", 2: "Yellow", 3: "Orange", 4: "Green", 5: "Pink"}
    ascending = {1: [1, 2], 2: [4, 3, 5], 3: [6, 7], 4: [9, 8], 5: [10, 11]}
    assert {color["id"]: (color["a"], color["b"]) for color in data["colors"]} == {
        color_id: (
            [{"id": fruit_id, "color": {"name": names[color_id]}} for fruit_id in ids],
            [{"id": fruit_id} for fruit_id in reversed(ids)],
        )
        for color_id, ids in ascending.items()
    }


async def test_row_in_one_alias_page_only(any_async_query: AnyQueryExecutor, query_tracker: QueryTracker) -> None:
    """Test that shared rows inside both pages or one page only appear in the collections of the pages holding them."""
    data = await _data(
        any_async_query,
        """
        {
            colorsPaginated {
                id
                a: fruits(orderBy: { sweetness: DESC }, limit: 1) { id }
                b: fruits(orderBy: { sweetness: DESC }, limit: 20) { id }
            }
        }
        """,
    )
    assert query_tracker.query_count == 1
    assert _ids_by_color(data["colorsPaginated"], "a", "b") == {
        1: ([2], [2, 1]),
        2: ([5], [5, 3, 4]),
        3: ([7], [7, 6]),
        4: ([8], [8, 9]),
        5: ([11], [11, 10]),
    }


async def test_hooked_aliases_with_an_unbounded_alias(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, raw_fruits: RawRecordData
) -> None:
    """Test that a hooked relation's bounded and unbounded aliases each keep the hook's rows and their own page."""
    data = await _data(
        any_async_query,
        """
        {
            colorsWithPaginatedSweetFruits {
                id
                a: fruits(limit: 1) { id }
                b: fruits(limit: null) { id }
            }
        }
        """,
    )
    assert query_tracker.query_count == 1
    for color in data["colorsWithPaginatedSweetFruits"]:
        sweet = [{"id": fruit["id"]} for fruit in _fruits_of(raw_fruits, color["id"]) if fruit["sweetness"] > 5]
        assert color["a"] == sweet[:1]
        assert color["b"] == sweet


@pytest.mark.allow_duplicate_reads(
    reason=(
        "aliases c and d of fruits.color.fruits read the fruits of the root color again through the color round trip; "
        "round trips are never detected as the same rows on LATERAL databases"
    ),
    dialects=("postgresql",),
)
async def test_aliases_on_both_paths(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, raw_fruits: RawRecordData
) -> None:
    """Test that aliases of a relation and aliases of the same relation through another path each keep their rows."""
    data = await _data(
        any_async_query,
        """
        {
            colors {
                id
                a: fruits(orderBy: { sweetness: DESC }) {
                    id
                    color { c: fruits(orderBy: { sweetness: ASC }) { id } d: fruits(orderBy: { id: DESC }) { id } }
                }
                b: fruits(orderBy: { id: ASC }) { id }
            }
        }
        """,
    )
    assert query_tracker.query_count == 1
    for color in data["colors"]:
        by_sweetness = [{"id": fruit["id"]} for fruit in _fruits_of(raw_fruits, color["id"], by="sweetness")]
        by_id = [{"id": fruit["id"]} for fruit in _fruits_of(raw_fruits, color["id"])]
        nested = {"c": by_sweetness, "d": by_id[::-1]}
        assert color["a"] == [{"id": fruit["id"], "color": nested} for fruit in by_sweetness[::-1]]
        assert color["b"] == by_id


async def test_aggregate_two_levels_under_aliases(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, raw_fruits: RawRecordData
) -> None:
    """Test that an aggregate two levels under both aliases of a relation counts the rows of each alias's objects."""
    data = await _data(
        any_async_query,
        """
        {
            colors {
                id
                a: fruits(orderBy: { sweetness: ASC }) { id color { fruitsAggregate { count } } }
                b: fruits(orderBy: { sweetness: DESC }) { id color { fruitsAggregate { count } } }
            }
        }
        """,
    )
    assert query_tracker.query_count == 1
    counts = Counter(fruit["color_id"] for fruit in raw_fruits)
    for color in data["colors"]:
        ascending = _fruits_of(raw_fruits, color["id"], by="sweetness")
        nested = {"color": {"fruitsAggregate": {"count": counts[color["id"]]}}}
        assert color["a"] == [{"id": fruit["id"], **nested} for fruit in ascending]
        assert color["b"] == [{"id": fruit["id"], **nested} for fruit in ascending[::-1]]


@pytest.mark.allow_duplicate_reads(
    reason=(
        "b's color.fruits keeps its own read next to the aliases x and y of a's color.fruits, and both read the fruits "
        "of the root color again through the color round trip"
    )
)
async def test_relation_aliased_under_one_shared_alias(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, raw_fruits: RawRecordData
) -> None:
    """Test that a relation aliased under one alias and selected once under another keeps each alias's rows."""
    data = await _data(
        any_async_query,
        """
        {
            colors {
                id
                a: fruits(orderBy: { id: ASC }) {
                    id
                    color { x: fruits { id } y: fruits(orderBy: { sweetness: DESC }) { id } }
                }
                b: fruits(orderBy: { id: DESC }) { id color { fruits { id } } }
            }
        }
        """,
    )
    assert query_tracker.query_count == 1
    for color in data["colors"]:
        by_id = [{"id": fruit["id"]} for fruit in _fruits_of(raw_fruits, color["id"])]
        by_sweetness = [{"id": fruit["id"]} for fruit in _fruits_of(raw_fruits, color["id"], by="sweetness")]
        a_color = {"x": by_id, "y": by_sweetness[::-1]}
        assert color["a"] == [{"id": fruit["id"], "color": a_color} for fruit in by_id]
        assert color["b"] == [{"id": fruit["id"], "color": {"fruits": by_id}} for fruit in by_id[::-1]]


async def test_argless_aliases_with_different_nested_selections(
    any_async_query: AnyQueryExecutor, raw_fruits: RawRecordData
) -> None:
    """Test that argument-free aliases of a relation each keep their own nested selection."""
    data = await _data(
        any_async_query,
        """
        {
            colors {
                id
                a: fruits { id color { fruits(orderBy: { id: ASC }) { id name } } }
                b: fruits { id color { fruits(orderBy: { id: DESC }) { id } } }
            }
        }
        """,
    )
    for color in data["colors"]:
        fruits = _fruits_of(raw_fruits, color["id"])
        a_color = {"fruits": [{"id": fruit["id"], "name": fruit["name"]} for fruit in fruits]}
        b_color = {"fruits": [{"id": fruit["id"]} for fruit in fruits[::-1]]}
        assert sorted(color["a"], key=lambda fruit: fruit["id"]) == [
            {"id": fruit["id"], "color": a_color} for fruit in fruits
        ]
        assert sorted(color["b"], key=lambda fruit: fruit["id"]) == [
            {"id": fruit["id"], "color": b_color} for fruit in fruits
        ]


async def test_argless_to_one_aliases_with_different_nested_selections(
    any_async_query: AnyQueryExecutor, raw_fruits: RawRecordData, raw_farms: RawRecordData
) -> None:
    """Test that argument-free aliases of a to-one relation each keep their own nested relations and selections."""
    data = await _data(
        any_async_query,
        """
        {
            fruits {
                colorId
                x: color { fruits(orderBy: { id: ASC }) { id farms { id } } }
                y: color { fruits(orderBy: { id: DESC }) { id } }
            }
        }
        """,
    )
    for fruit in data["fruits"]:
        siblings = _fruits_of(raw_fruits, fruit["colorId"])
        x_fruits = [
            {
                "id": sibling["id"],
                "farms": sorted(farm["id"] for farm in raw_farms if farm["fruit_id"] == sibling["id"]),
            }
            for sibling in siblings
        ]
        assert [
            {"id": sibling["id"], "farms": sorted(farm["id"] for farm in sibling["farms"])}
            for sibling in fruit["x"]["fruits"]
        ] == x_fruits
        assert fruit["y"] == {"fruits": [{"id": sibling["id"]} for sibling in siblings[::-1]]}


@pytest.mark.allow_duplicate_reads(
    reason=(
        "the aliases of fruits.color.fruits read the fruits of the root color again through the color round trip; "
        "round trips are never detected as the same rows on LATERAL databases"
    ),
    dialects=("postgresql",),
)
async def test_argless_aliases_with_partly_shared_nested_arguments(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, raw_fruits: RawRecordData
) -> None:
    """Test that argument-free aliases keep their own nested selections when only some share nested arguments."""
    data = await _data(
        any_async_query,
        """
        {
            colors {
                id
                a: fruits { id color { fruits(orderBy: { id: ASC }) { id } } }
                b: fruits { id name color { fruits(orderBy: { id: ASC }) { name } } }
                c: fruits { id color { fruits(orderBy: { id: DESC }) { id } } }
            }
        }
        """,
    )
    assert query_tracker.query_count == 1
    for color in data["colors"]:
        fruits = _fruits_of(raw_fruits, color["id"])
        a_color = {"fruits": [{"id": fruit["id"]} for fruit in fruits]}
        b_color = {"fruits": [{"name": fruit["name"]} for fruit in fruits]}
        c_color = {"fruits": [{"id": fruit["id"]} for fruit in fruits[::-1]]}
        assert sorted(color["a"], key=lambda fruit: fruit["id"]) == [
            {"id": fruit["id"], "color": a_color} for fruit in fruits
        ]
        assert sorted(color["b"], key=lambda fruit: fruit["id"]) == [
            {"id": fruit["id"], "name": fruit["name"], "color": b_color} for fruit in fruits
        ]
        assert sorted(color["c"], key=lambda fruit: fruit["id"]) == [
            {"id": fruit["id"], "color": c_color} for fruit in fruits
        ]
