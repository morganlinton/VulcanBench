from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

from tests.integration.fixtures import QueryTracker
from tests.integration.typing import RawRecordData
from tests.integration.utils import to_graphql_representation
from tests.typing import AnyQueryExecutor
from tests.utils import maybe_async

if TYPE_CHECKING:
    from syrupy.assertion import SnapshotAssertion

    from strawchemy.repository.typing import AnySession
    from strawchemy.typing import SupportedDialect

pytestmark = [pytest.mark.integration]


# Create tests


@pytest.mark.parametrize(
    ("query_name", "query"),
    [
        pytest.param(
            "createColor",
            """
            mutation {
                createColor(data: {  name: "new color" }) {
                    name
                }
            }
            """,
            id="createColor",
        ),
        pytest.param(
            "createValidatedColor",
            """
            mutation {
                createValidatedColor(data: {  name: "new color" }) {
                    ... on ColorType {
                        name
                    }
                }
            }
            """,
            id="createValidatedColor",
        ),
        pytest.param(
            "createColorManualValidation",
            """
            mutation {
                createColorManualValidation(data: {  name: "new color" }) {
                    ... on ColorType {
                        name
                    }
                }
            }
            """,
            id="createValidatedColor-manual",
        ),
        pytest.param(
            "createValidatedColor",
            """
            mutation {
                createValidatedColor(data: {  name: "new color" }) {
                    ... on ColorType {
                        name
                    }
                    ... on ValidationErrorType {
                        id
                        errors {
                            id
                            loc
                            message
                            type
                        }
                    }
                }
            }
            """,
            id="createValidatedColorAllFragments",
        ),
        pytest.param(
            "createColorManualValidation",
            """
            mutation {
                createColorManualValidation(data: {  name: "new color" }) {
                    ... on ColorType {
                        name
                    }
                    ... on ValidationErrorType {
                        id
                        errors {
                            id
                            loc
                            message
                            type
                        }
                    }
                }
            }
            """,
            id="createValidatedColorAllFragments-manual",
        ),
    ],
)
@pytest.mark.snapshot
async def test_create(
    query_name: str,
    query: str,
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert result.data[query_name] == {"name": "new color"}

    insert_tracker, select_tracker = query_tracker.filter("insert"), query_tracker.filter("select")
    assert insert_tracker.query_count == 1
    assert select_tracker.query_count == 1
    assert insert_tracker[0].statement_formatted == sql_snapshot
    assert select_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_create_with_to_one_set(
    raw_colors: RawRecordData,
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    query = """
        mutation {{
            createFruit(data: {{
                name: "new fruit",
                sweetness: 1,
                waterPercent: 0.8,
                color: {{
                    set: {{ id: {color_id} }}
                }}
            }}) {{
                name
                color {{
                    id
                }}
            }}
        }}
    """
    result = await maybe_async(
        any_async_query(query.format(color_id=to_graphql_representation(raw_colors[0]["id"], "input")))
    )
    assert not result.errors
    assert result.data
    assert result.data["createFruit"] == {
        "name": "new fruit",
        "color": {"id": to_graphql_representation(raw_colors[0]["id"], "output")},
    }

    query_tracker.assert_statements(1, "select", sql_snapshot)
    query_tracker.assert_statements(1, "insert", sql_snapshot)


@pytest.mark.snapshot
async def test_create_with_to_one_set_null(
    raw_colors: RawRecordData,
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    query = """
        mutation {{
            createFruit(data: {{
                name: "new fruit",
                sweetness: 1,
                waterPercent: 0.8,
                color: {{ set: null }}
            }}) {{
                name
                color {{
                    id
                }}
            }}
        }}
    """
    result = await maybe_async(
        any_async_query(query.format(color_id=to_graphql_representation(raw_colors[0]["id"], "input")))
    )
    assert not result.errors
    assert result.data
    assert result.data["createFruit"] == {"name": "new fruit", "color": None}

    query_tracker.assert_statements(1, "select", sql_snapshot)
    query_tracker.assert_statements(1, "insert", sql_snapshot)


@pytest.mark.snapshot
async def test_create_with_to_one_create(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    query = """
            mutation {
                createFruit(data: {
                    name: "new color",
                    sweetness: 1,
                    waterPercent: 0.8,
                    color: {
                        create: { name: "new sub color" }
                    }
                }) {
                    name
                    color {
                        name
                    }
                }
            }
            """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert result.data["createFruit"] == {"name": "new color", "color": {"name": "new sub color"}}

    query_tracker.assert_statements(1, "select", sql_snapshot)
    query_tracker.assert_statements(2, "insert", sql_snapshot)


@pytest.mark.snapshot
async def test_create_with_to_one_create_and_nested_set(
    raw_topics: RawRecordData,
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    query = """
            mutation {{
                createUser(data: {{
                    name: "Bob",
                    group: {{
                        create: {{
                            name: "new group",
                            topics: {{ set: [ {{ id: {topic_id} }} ] }}
                        }}
                    }}
                }}) {{
                    name
                    group {{
                        name
                        topics {{
                            id
                        }}
                    }}
                }}
            }}
            """
    result = await maybe_async(
        any_async_query(query.format(topic_id=to_graphql_representation(raw_topics[0]["id"], "input")))
    )
    assert not result.errors
    assert result.data
    assert result.data["createUser"] == {
        "name": "Bob",
        "group": {"name": "new group", "topics": [{"id": to_graphql_representation(raw_topics[0]["id"], "output")}]},
    }

    query_tracker.assert_statements(1, "select", sql_snapshot)
    query_tracker.assert_statements(2, "insert", sql_snapshot)


@pytest.mark.parametrize(
    "query",
    [
        pytest.param(
            """
        mutation {{
            createColor(data: {{
                name: "new color",
                fruits: {{
                    set: [{{ id: {fruit_id} }}]
                }}
            }}) {{
                name
                fruits {{
                    id
                }}
            }}
        }}
        """,
            id="set",
        ),
        pytest.param(
            """
        mutation {{
            createColor(data: {{
                name: "new color",
                fruits: {{
                    add: [{{ id: {fruit_id} }}]
                }}
            }}) {{
                name
                fruits {{
                    id
                }}
            }}
        }}
        """,
            id="add",
        ),
    ],
)
@pytest.mark.snapshot
async def test_create_with_existing_to_many(
    query: str,
    raw_fruits: RawRecordData,
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    result = await maybe_async(
        any_async_query(query.format(fruit_id=to_graphql_representation(raw_fruits[0]["id"], "input")))
    )
    assert not result.errors
    assert result.data
    assert result.data["createColor"] == {
        "name": "new color",
        "fruits": [{"id": to_graphql_representation(raw_fruits[0]["id"], "output")}],
    }

    query_tracker.assert_statements(1, "select", sql_snapshot)
    query_tracker.assert_statements(1, "insert", sql_snapshot)
    query_tracker.assert_statements(1, "update", sql_snapshot)


@pytest.mark.snapshot
async def test_create_with_to_many_create(
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    dialect: SupportedDialect,
) -> None:
    query = """
            mutation {
                createColor(data: {
                    name: "new color",
                    fruits: {
                        create: [
                            { name: "new fruit 1", sweetness: 1, waterPercent: 0.8 },
                            { name: "new fruit 2", sweetness: 2, waterPercent: 0.9 }
                        ]
                    }
                }) {
                    name
                    fruits {
                        name
                    }
                }
            }
            """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert result.data["createColor"] == {
        "name": "new color",
        "fruits": [{"name": "new fruit 1"}, {"name": "new fruit 2"}],
    }
    if dialect in ("postgresql", "sqlite"):
        query_tracker.assert_statements(2, "insert", sql_snapshot)
    else:
        query_tracker.assert_statements(3, "insert", sql_snapshot)
    query_tracker.assert_statements(1, "select", sql_snapshot)


@pytest.mark.snapshot
async def test_create_with_to_many_create_and_nested_set(
    raw_farms: RawRecordData,
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    query = """
            mutation {{
                createColor(data: {{
                    name: "White",
                    fruits: {{
                        create: [
                            {{
                                name: "Grape",
                                sweetness: 1,
                                waterPercent: 0.8,
                                farms: {{ set: [ {{ id: {farm_id} }} ] }}
                            }},
                        ]
                    }}
                }}) {{
                    name
                    fruits {{
                        name
                        farms {{
                            id
                        }}
                    }}
                }}
            }}
            """
    result = await maybe_async(
        any_async_query(query.format(farm_id=to_graphql_representation(raw_farms[0]["id"], "input")))
    )
    assert not result.errors
    assert result.data
    assert result.data["createColor"] == {
        "name": "White",
        "fruits": [{"name": "Grape", "farms": [{"id": to_graphql_representation(raw_farms[0]["id"], "output")}]}],
    }

    query_tracker.assert_statements(1, "select", sql_snapshot)
    query_tracker.assert_statements(2, "insert", sql_snapshot)


async def test_create_validated_with_nested_relations_create(any_async_query: AnyQueryExecutor) -> None:
    query = """
            mutation {
                createValidatedColor(data: {
                    name: "white",
                    fruits: {
                        create: [
                            {
                                name: "Grape",
                                sweetness: 1,
                                waterPercent: 0.8,
                                product: { create: { name: "wine" } }
                            },
                            {
                                name: "Lychee",
                                sweetness: 1,
                                waterPercent: 0.7,
                                farms: { create: [ { name: "Bio farm" } ] }
                            },
                        ]
                    }
                }) {
                    ... on ColorType {
                        name
                        fruits {
                            name
                            product {
                                name
                            }
                            farms {
                                name
                            }
                        }
                    }
                }
            }
            """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert result.data["createValidatedColor"]["name"] == "white"
    expected_fruits = [
        {"name": "Lychee", "product": None, "farms": [{"name": "Bio farm"}]},
        {"name": "Grape", "product": {"name": "wine"}, "farms": []},
    ]
    for fruit in expected_fruits:
        assert fruit in result.data["createValidatedColor"]["fruits"]


async def test_create_validated_with_nested_upsert(any_async_query: AnyQueryExecutor) -> None:
    query = """
            mutation {
                createValidatedColor(data: {
                    name: "white",
                    fruits: {
                        upsert: {
                            create: [
                                {
                                    name: "Plum",
                                    sweetness: 7,
                                    waterPercent: 0.87,
                                    product: { create: { name: "jam" } }
                                }
                            ]
                            conflictFields: name
                        }
                    }
                }) {
                    ... on ColorType {
                        name
                        fruits {
                            name
                            product {
                                name
                            }
                        }
                    }
                }
            }
            """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert result.data["createValidatedColor"] == {
        "name": "white",
        "fruits": [{"name": "Plum", "product": {"name": "jam"}}],
    }


@pytest.mark.snapshot
async def test_create_with_nested_mixed_relations_create(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    query = """
            mutation {
                createColor(data: {
                    name: "White",
                    fruits: {
                        create: [
                            {
                                name: "Grape",
                                sweetness: 1,
                                waterPercent: 0.8,
                                product: { create: { name: "wine" } }
                            },
                            {
                                name: "Lychee",
                                sweetness: 1,
                                waterPercent: 0.7,
                                farms: { create: [ { name: "Bio farm" } ] }
                            },
                        ]
                    }
                }) {
                    name
                    fruits {
                        name
                        product {
                            name
                        }
                        farms {
                            name
                        }
                    }
                }
            }
            """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert result.data["createColor"]["name"] == "White"
    expected_fruits = [
        {"name": "Lychee", "product": None, "farms": [{"name": "Bio farm"}]},
        {"name": "Grape", "product": {"name": "wine"}, "farms": []},
    ]
    for fruit in expected_fruits:
        assert fruit in result.data["createColor"]["fruits"]

    # Heterogeneous params means inserts cannot be batched
    query_tracker.assert_statements(5, "insert", sql_snapshot)
    query_tracker.assert_statements(1, "select", sql_snapshot)


@pytest.mark.snapshot
async def test_create_many(
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    dialect: SupportedDialect,
) -> None:
    result = await maybe_async(
        any_async_query(
            """
                mutation {
                    createColors(
                        data: [
                            { name: "new color 1" }
                            { name: "new color 2" }
                        ]
                    ) {
                        name
                    }
                }
            """
        )
    )
    assert not result.errors
    assert result.data
    assert result.data["createColors"] == [{"name": "new color 1"}, {"name": "new color 2"}]
    if dialect in ("postgresql", "sqlite"):
        query_tracker.assert_statements(1, "insert", sql_snapshot)
    else:
        query_tracker.assert_statements(2, "insert", sql_snapshot)
    query_tracker.assert_statements(1, "select", sql_snapshot)


@pytest.mark.snapshot
async def test_create_init_defaults(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    result = await maybe_async(
        any_async_query(
            """
            mutation {
                createUser(data: { name: "Jeanne" }) {
                    name
                    bio
                }
            }
            """
        )
    )
    assert not result.errors
    assert result.data
    assert result.data["createUser"] == {
        "name": "Jeanne",
        "bio": "Lorem ipsum dolor sit amet, consectetur adipiscing elit",
    }

    query_tracker.assert_statements(1, "insert", sql_snapshot)


@pytest.mark.snapshot
async def test_create_with_secondary_table_set(
    any_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    query = """
            mutation {
                createUser(
                    data: {
                        name: "Astrid",
                        departments: { set: [ { id: 1 } ] }
                    }
                ) {
                    name
                    departments {
                        id
                        name
                    }
                }
            }
            """

    result = await maybe_async(any_query(query))
    assert not result.errors
    assert result.data

    assert result.data["createUser"] == {"name": "Astrid", "departments": [{"id": 1, "name": "IT"}]}

    insert_tracker, select_tracker = query_tracker.filter("insert"), query_tracker.filter("select")
    assert insert_tracker.query_count == 2
    assert select_tracker.query_count == 1
    assert insert_tracker[0].statement_formatted == sql_snapshot
    assert select_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_create_with_secondary_table_create(
    any_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    query = """
            mutation {
                createUser(
                    data: {
                        name: "Astrid",
                        departments: { create: [ { name: "Support" } ] }
                    }
                ) {
                    name
                    departments {
                        name
                    }
                }
            }
            """

    result = await maybe_async(any_query(query))
    assert not result.errors
    assert result.data
    assert result.data["createUser"] == {"name": "Astrid", "departments": [{"name": "Support"}]}

    query_tracker.assert_statements(3, "insert", sql_snapshot)
    query_tracker.assert_statements(1, "select", sql_snapshot)


# Update tests


@pytest.mark.parametrize(
    ("query", "query_name"),
    [
        pytest.param(
            """
                mutation {{
                    updateColor(
                        data: {{
                            id: {color_id},
                            name: "updated color"
                        }}
                    ) {{
                        id
                        name
                    }}
                }}
                """,
            "updateColor",
            id="no-validation",
        ),
        pytest.param(
            """
                mutation {{
                    updateValidatedColor(
                        data: {{
                            id: {color_id},
                            name: "updated color"
                        }}
                    ) {{
                        ... on ColorType {{
                            id
                            name
                        }}
                        ... on ValidationErrorType {{
                            errorId: id
                            errors {{
                                id
                                loc
                                message
                                type
                            }}
                        }}
                    }}
                }}
                """,
            "updateValidatedColor",
            id="validation-fragment",
        ),
        pytest.param(
            """
                mutation {{
                    updateValidatedColor(
                        data: {{
                            id: {color_id},
                            name: "updated color"
                        }}
                    ) {{
                        ... on ColorType {{
                            id
                            name
                        }}
                    }}
                }}
                """,
            "updateValidatedColor",
            id="validation-no-fragment",
        ),
    ],
)
@pytest.mark.snapshot
async def test_update(
    query_name: str,
    query: str,
    raw_colors: RawRecordData,
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Tests a simple update mutation."""
    result = await maybe_async(
        any_async_query(query.format(color_id=to_graphql_representation(raw_colors[0]["id"], "input")))
    )
    assert not result.errors
    assert result.data
    assert result.data[query_name] == {
        "id": to_graphql_representation(raw_colors[0]["id"], "output"),
        "name": "updated color",
    }

    query_tracker.assert_statements(1, "update", sql_snapshot)  # Update color name
    query_tracker.assert_statements(1, "select", sql_snapshot)  # Fetch id + name


@pytest.mark.snapshot
async def test_update_by_filter(
    raw_colors: RawRecordData,
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    dialect: SupportedDialect,
) -> None:
    """Tests a simple update mutation."""
    query = """
        mutation {
            updateColorsFilter(
                data: {
                    name: "updated color"
                },
                filter: {
                    name: { eq: "Red" }
                }
            ) {
                id
                name
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert result.data["updateColorsFilter"] == [
        {
            "id": to_graphql_representation(raw_colors[0]["id"], "output"),
            "name": "updated color",
        }
    ]

    query_tracker.assert_statements(1, "update", sql_snapshot)  # Update color name
    if dialect in ("postgresql", "sqlite"):
        query_tracker.assert_statements(1, "select", sql_snapshot)  # Fetch id + name
    else:
        query_tracker.assert_statements(2, "select", sql_snapshot)


@pytest.mark.snapshot
async def test_update_by_filter_only_return_affected_objects(
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    dialect: SupportedDialect,
) -> None:
    """Tests a simple update mutation."""
    query = """
        mutation {
            updateColorsFilter(
                data: {
                    name: "updated color"
                },
                filter: {
                    name: { eq: "unknown" }
                }
            ) {
                id
                name
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert result.data["updateColorsFilter"] == []

    query_tracker.assert_statements(1, "update", sql_snapshot)  # Update color name
    if dialect in ("postgresql", "sqlite"):
        query_tracker.assert_statements(1, "select", sql_snapshot)  # Fetch id + name
    else:
        query_tracker.assert_statements(2, "select", sql_snapshot)


@pytest.mark.snapshot
@pytest.mark.parametrize(
    ("dto_filter", "updated_names"),
    [
        pytest.param('{ color: { name: { eq: "Red" } } }', ["Apple", "Cherry"], id="relation"),
        pytest.param(
            '{ _and: [{ sweetness: { gt: 5 } }, { color: { name: { eq: "Red" } } }] }',
            ["Cherry"],
            id="column-and-relation",
        ),
    ],
)
async def test_update_by_filter_on_to_one_relation(
    dto_filter: str,
    updated_names: list[str],
    raw_fruits: RawRecordData,
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Test that an update filtered on a to-one relation updates only the rows whose relation matches."""
    query = f"""
        mutation {{
            updateFruitsFilter(data: {{ sweetness: 42 }}, filter: {dto_filter}) {{
                id
                sweetness
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    updated_ids = [fruit["id"] for fruit in raw_fruits if fruit["name"] in updated_names]
    assert sorted(result.data["updateFruitsFilter"], key=lambda fruit: fruit["id"]) == [
        {"id": to_graphql_representation(fruit_id, "output"), "sweetness": 42} for fruit_id in updated_ids
    ]
    query_tracker.assert_statements(1, "update", sql_snapshot)

    result = await maybe_async(any_async_query("{ fruits(filter: { sweetness: { eq: 42 } }) { id } }"))
    assert not result.errors
    assert result.data
    assert sorted(fruit["id"] for fruit in result.data["fruits"]) == [
        to_graphql_representation(fruit_id, "output") for fruit_id in updated_ids
    ]


@pytest.mark.snapshot
@pytest.mark.parametrize(
    ("dto_filter", "color_index"),
    [
        pytest.param('{ fruits: { name: { in: ["Apple", "Cherry"] } } }', 0, id="relation"),
        pytest.param("{ fruitsAggregate: { count: { arguments: [id], predicate: { gt: 2 } } } }", 1, id="aggregation"),
        pytest.param(
            '{ name: { eq: "Orange" }, fruitsAggregate: { count: { arguments: [id], predicate: { gt: 1 } } } }',
            2,
            id="column-and-aggregation",
        ),
    ],
)
async def test_update_by_filter_on_to_many_relation(
    dto_filter: str,
    color_index: int,
    raw_colors: RawRecordData,
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Test that an update filtered on a to-many relation updates each matching row once, and only those."""
    query = f"""
        mutation {{
            updateColorsFilter(data: {{ name: "updated color" }}, filter: {dto_filter}) {{
                id
                name
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    color_id = to_graphql_representation(raw_colors[color_index]["id"], "output")
    assert result.data["updateColorsFilter"] == [{"id": color_id, "name": "updated color"}]
    query_tracker.assert_statements(1, "update", sql_snapshot)

    result = await maybe_async(any_async_query('{ colors(filter: { name: { eq: "updated color" } }) { id } }'))
    assert not result.errors
    assert result.data
    assert result.data["colors"] == [{"id": color_id}]


@pytest.mark.snapshot
@pytest.mark.parametrize(
    ("dto_filter", "updated_names"),
    [
        pytest.param("{ departments: { name: { isNull: true } } }", [], id="to-many-null"),
        pytest.param(
            '{ name: { neq: "Zed" }, departments: { name: { isNull: true } } }', [], id="column-and-to-many-null"
        ),
        pytest.param(
            '{ name: { neq: "Zed" }, departments: { _or: [{ name: { isNull: true } }, { name: { eq: "IT" } }] } }',
            ["Alice", "Charlie"],
            id="column-and-to-many-or-null",
        ),
        pytest.param(
            '{ group: { name: { isNull: true } }, departments: { name: { eq: "IT" } } }',
            [],
            id="to-one-null-and-to-many",
        ),
        pytest.param("{ group: { name: { isNull: true } } }", [], id="to-one-null"),
        pytest.param('{ name: { neq: "Zed" }, group: { name: { isNull: true } } }', [], id="column-and-to-one-null"),
        pytest.param(
            '{ _or: [{ name: { eq: "Bob" } }, { group: { name: { isNull: true } } }] }', ["Bob"], id="or-to-one-null"
        ),
        pytest.param(
            '{ name: { neq: "Zed" }, _not: { group: { name: { isNull: true } } } }',
            ["Alice", "Bob", "Charlie", "Tango"],
            id="column-and-not-to-one-null",
        ),
        pytest.param('{ group: { _not: { name: { eq: "Group 1" } } } }', [], id="to-one-not"),
        pytest.param('{ departments: { _not: { name: { eq: "IT" } } } }', ["Bob", "Charlie"], id="to-many-not"),
        pytest.param(
            '{ departments: { _or: [{ name: { eq: "Sales" } }, { _not: { name: { isNull: false } } }] } }',
            ["Bob"],
            id="to-many-or-not",
        ),
        pytest.param(
            "{ departments: { usersAggregate: { count: { arguments: [id], predicate: { lt: 2 } } } } }",
            ["Bob", "Charlie"],
            id="to-many-aggregation",
        ),
        pytest.param(
            "{ group: { topicsAggregate: { count: { arguments: [id], predicate: { eq: 0 } } } } }",
            [],
            id="to-one-aggregation",
        ),
        pytest.param(
            '{ _or: [{ name: { eq: "Bob" } }, '
            "{ group: { topicsAggregate: { count: { arguments: [id], predicate: { eq: 0 } } } } }] }",
            ["Bob"],
            id="or-to-one-aggregation",
        ),
        pytest.param(
            '{ _or: [{ group: { name: { eq: "Group 1" } } }, { departments: { name: { eq: "Sales" } } }] }',
            ["Alice", "Bob"],
            id="or-to-one-and-to-many",
        ),
        pytest.param(
            "{ _or: [{ departmentsAggregate: { count: { arguments: [id], predicate: { eq: 0 } } } }, "
            "{ group: { name: { isNull: true } } }] }",
            ["Tango"],
            id="or-aggregation-and-to-one-null",
        ),
        pytest.param(
            "{ _or: [{ departmentsAggregate: { count: { arguments: [id], predicate: { eq: 0 } } } }, "
            '{ group: { name: { eq: "Group 1" } } }] }',
            ["Alice", "Tango"],
            id="or-aggregation-and-to-one",
        ),
        pytest.param('{ _not: { departments: { name: { eq: "IT" } } } }', ["Bob", "Tango"], id="not-to-many"),
        pytest.param(
            '{ _not: { departments: { users: { name: { eq: "Alice" } } } } }', ["Bob", "Tango"], id="not-nested-to-many"
        ),
        pytest.param(
            '{ name: { neq: "Tango" }, _not: { departments: { name: { eq: "IT" } } } }',
            ["Bob"],
            id="column-and-not-to-many",
        ),
        pytest.param(
            '{ departments: { _not: { users: { name: { eq: "Alice" } } } } }',
            ["Bob", "Charlie"],
            id="to-many-not-nested-to-many",
        ),
        pytest.param("{ group: { name: {} } }", ["Alice", "Bob", "Charlie", "Tango"], id="to-one-empty"),
        pytest.param("{ departments: { name: {} } }", ["Alice", "Bob", "Charlie", "Tango"], id="to-many-empty"),
        pytest.param("{ _not: { departments: {} } }", ["Alice", "Bob", "Charlie", "Tango"], id="not-to-many-empty"),
        pytest.param(
            "{ departmentsAggregate: { count: { arguments: [id], predicate: {} } } }",
            ["Alice", "Bob", "Charlie", "Tango"],
            id="aggregation-empty",
        ),
        pytest.param(
            '{ _or: [{ departments: { name: {} } }, { name: { eq: "Bob" } }] }', ["Bob"], id="or-to-many-empty"
        ),
        pytest.param("{ group: null }", ["Alice", "Bob", "Charlie", "Tango"], id="to-one-null-field"),
        pytest.param("{ departments: null }", ["Alice", "Bob", "Charlie", "Tango"], id="to-many-null-field"),
        pytest.param(
            "{ departmentsAggregate: { count: null } }",
            ["Alice", "Bob", "Charlie", "Tango"],
            id="aggregation-null-field",
        ),
        pytest.param('{ _not: null, name: { eq: "Bob" } }', ["Bob"], id="not-null-field"),
        pytest.param(
            "{ departmentsAggregate: { count: { arguments: null, predicate: { gt: 0 } } } }",
            ["Alice", "Bob", "Charlie"],
            id="aggregation-arguments-null-field",
        ),
        pytest.param('{ _or: [{ departments: null }, { name: { eq: "Bob" } }] }', ["Bob"], id="or-to-many-null-field"),
        pytest.param(
            '{ _and: [{ departments: { name: { eq: "IT" } } }, { departments: { name: { eq: "Platform" } } }] }',
            ["Charlie"],
            id="and-same-to-many",
        ),
        pytest.param(
            '{ _not: { _and: [{ departments: { name: { eq: "IT" } } }, '
            '{ departments: { name: { eq: "Platform" } } }] } }',
            ["Alice", "Bob", "Tango"],
            id="not-and-same-to-many",
        ),
    ],
)
async def test_update_users_by_split_filter(
    dto_filter: str,
    updated_names: list[str],
    raw_users: RawRecordData,
    any_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Test that an update split between the WHERE and an EXISTS subquery updates and returns the same rows."""
    query = f"""
        mutation {{
            updateUsersFilter(data: {{ name: "updated" }}, filter: {dto_filter}) {{
                id
                name
            }}
        }}
    """
    result = await maybe_async(any_query(query))
    assert not result.errors
    assert result.data
    updated_ids = sorted(user["id"] for user in raw_users if user["name"] in updated_names)
    assert sorted(result.data["updateUsersFilter"], key=lambda user: user["id"]) == [
        {"id": user_id, "name": "updated"} for user_id in updated_ids
    ]
    query_tracker.assert_statements(1, "update", sql_snapshot)

    result = await maybe_async(any_query('{ users(filter: { name: { eq: "updated" } }) { id } }'))
    assert not result.errors
    assert result.data
    assert sorted(user["id"] for user in result.data["users"]) == updated_ids


@pytest.mark.parametrize(
    ("dto_filter", "updated_names"),
    [
        pytest.param("{ name: { in: [] } }", [], id="in"),
        pytest.param("{ bio: { nin: [] } }", ["Alice", "Bob", "Charlie", "Tango"], id="nin"),
        pytest.param(
            "{ departments: { name: { in: [] } } }",
            [],
            id="to-many-in",
        ),
        pytest.param("{ bio: { eq: null } }", ["Alice", "Bob", "Charlie", "Tango"], id="eq-null"),
        pytest.param(
            "{ _not: { bio: { isNull: null, gt: null } } }", ["Alice", "Bob", "Charlie", "Tango"], id="not-null"
        ),
        pytest.param("{ group: { name: { neq: null } } }", ["Alice", "Bob", "Charlie", "Tango"], id="to-one-null"),
    ],
)
async def test_update_users_by_empty_list_filter(
    dto_filter: str, updated_names: list[str], raw_users: RawRecordData, any_async_query: AnyQueryExecutor
) -> None:
    query = f"""
        mutation {{
            updateUsersFilter(data: {{ name: "updated" }}, filter: {dto_filter}) {{
                id
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    updated_ids = sorted(user["id"] for user in raw_users if user["name"] in updated_names)
    assert sorted(user["id"] for user in result.data["updateUsersFilter"]) == updated_ids

    result = await maybe_async(any_async_query('{ users(filter: { name: { eq: "updated" } }) { id } }'))
    assert not result.errors
    assert result.data
    assert sorted(user["id"] for user in result.data["users"]) == updated_ids


@pytest.mark.snapshot
async def test_update_with_to_one_set(
    raw_fruits: RawRecordData,
    raw_colors: RawRecordData,
    any_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Tests updating a record and setting a to-one relationship."""
    fruit_id_gql = to_graphql_representation(raw_fruits[0]["id"], "input")
    # Use a different color to test the update
    color_id_gql = to_graphql_representation(raw_colors[1]["id"], "input")
    query = f"""
        mutation {{
            updateFruit(
                data: {{
                    id: {fruit_id_gql},
                    name: "updated fruit name",
                    color: {{ set: {{ id: {color_id_gql} }} }}
                }}
            ) {{
                id
                name
                color {{
                    id
                }}
            }}
        }}
    """
    result = await maybe_async(any_query(query))
    assert not result.errors
    assert result.data
    assert result.data["updateFruit"] == {
        "id": to_graphql_representation(raw_fruits[0]["id"], "output"),
        "name": "updated fruit name",
        "color": {"id": to_graphql_representation(raw_colors[1]["id"], "output")},
    }

    query_tracker.assert_statements(1, "update", sql_snapshot)  # Update fruit's color_id
    query_tracker.assert_statements(1, "select", sql_snapshot)  # Fetch updated fruit + color


@pytest.mark.snapshot
async def test_update_with_to_one_set_null(
    raw_fruits: RawRecordData, any_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    """Tests updating a record and setting a to-one relationship."""
    fruit_id_gql = to_graphql_representation(raw_fruits[0]["id"], "input")
    query = f"""
        mutation {{
            updateFruit(
                data: {{
                    id: {fruit_id_gql},
                    name: "updated fruit name",
                    color: {{ set: null }}
                }}
            ) {{
                id
                name
                color {{
                    id
                }}
            }}
        }}
    """
    result = await maybe_async(any_query(query))
    assert not result.errors
    assert result.data
    assert result.data["updateFruit"] == {
        "id": to_graphql_representation(raw_fruits[0]["id"], "output"),
        "name": "updated fruit name",
        "color": None,
    }

    query_tracker.assert_statements(1, "update", sql_snapshot)  # Update fruit's color_id
    query_tracker.assert_statements(1, "select", sql_snapshot)  # Fetch updated fruit + color


@pytest.mark.snapshot
async def test_update_with_to_one_create(
    raw_fruits: RawRecordData,
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Tests updating a record and creating a new related record for a to-one relationship."""
    fruit_id_gql = to_graphql_representation(raw_fruits[0]["id"], "input")
    query = f"""
        mutation {{
            updateFruit(
                data: {{
                    id: {fruit_id_gql},
                    name: "updated fruit name 2",
                    color: {{ create: {{ name: "newly created color during update" }} }}
                }}
            ) {{
                id
                name
                color {{
                    name
                }}
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert result.data["updateFruit"] == {
        "id": to_graphql_representation(raw_fruits[0]["id"], "output"),
        "name": "updated fruit name 2",
        "color": {"name": "newly created color during update"},
    }

    query_tracker.assert_statements(1, "insert", sql_snapshot)  # Insert new color
    query_tracker.assert_statements(1, "update", sql_snapshot)  # Update fruit's color_id
    query_tracker.assert_statements(1, "select", sql_snapshot)  # Fetch updated fruit + new color


@pytest.mark.snapshot
async def test_update_with_to_one_create_and_nested_set(
    raw_users: RawRecordData,
    raw_topics: RawRecordData,
    any_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Tests updating a record and creating a nested related record which itself sets a relationship."""
    user_id_gql = to_graphql_representation(raw_users[0]["id"], "input")
    # Use a different topic to test the update/create
    topic_id_gql = to_graphql_representation(raw_topics[1]["id"], "input")
    query = f"""
        mutation {{
            updateUser(
                data: {{
                    id: {user_id_gql},
                    name: "Updated Bob",
                    group: {{
                        create: {{
                            name: "new group during update",
                            topics: {{ set: [ {{ id: {topic_id_gql} }} ] }}
                        }}
                    }}
                }}
            ) {{
                id
                name
                group {{
                    name
                    topics {{
                        id
                    }}
                }}
            }}
        }}
    """
    result = await maybe_async(any_query(query))
    assert not result.errors
    assert result.data
    assert result.data["updateUser"] == {
        "id": to_graphql_representation(raw_users[0]["id"], "output"),
        "name": "Updated Bob",
        "group": {
            "name": "new group during update",
            "topics": [{"id": to_graphql_representation(raw_topics[1]["id"], "output")}],
        },
    }

    query_tracker.assert_statements(1, "insert", sql_snapshot)  # Insert new group
    query_tracker.assert_statements(2, "update", sql_snapshot)  # Update user's group_id and topic's group id
    query_tracker.assert_statements(1, "select", sql_snapshot)  # Fetch updated user + new group + topic


@pytest.mark.snapshot
async def test_update_with_to_many_set(
    raw_colors: RawRecordData,
    raw_fruits: RawRecordData,
    any_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Tests updating a record and setting (replacing) a to-many relationship."""
    color_id_gql = to_graphql_representation(raw_colors[0]["id"], "input")
    # Use a different fruit to test the update
    fruit_id_gql = to_graphql_representation(raw_fruits[1]["id"], "input")
    query = f"""
        mutation {{
            updateColor(
                data: {{
                    id: {color_id_gql},
                    name: "updated color name",
                    fruits: {{ set: [{{ id: {fruit_id_gql} }}] }}
                }}
            ) {{
                id
                name
                fruits {{
                    id
                }}
            }}
        }}
    """
    result = await maybe_async(any_query(query))
    assert not result.errors
    assert result.data
    assert result.data["updateColor"] == {
        "id": to_graphql_representation(raw_colors[0]["id"], "output"),
        "name": "updated color name",
        "fruits": [{"id": to_graphql_representation(raw_fruits[1]["id"], "output")}],
    }

    # 1. Disconnect previous fruits
    # 2. Update specified fruit's color_id
    # 3. Update color's name
    query_tracker.assert_statements(3, "update", sql_snapshot)
    # Fetch updated color + fruit
    query_tracker.assert_statements(1, "select", sql_snapshot)


@pytest.mark.snapshot
async def test_update_with_to_many_remove(
    raw_colors: RawRecordData,
    raw_fruits: RawRecordData,
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Tests updating a record and setting (replacing) a to-many relationship."""
    color_id_gql = to_graphql_representation(raw_colors[0]["id"], "input")
    # Remove the existing fruits
    fruit_ids_gql = [
        to_graphql_representation(fruit["id"], "input")
        for fruit in raw_fruits
        if fruit["color_id"] == raw_colors[0]["id"]
    ]
    query = f"""
        mutation {{
            updateColor(
                data: {{
                    id: {color_id_gql},
                    name: "updated color name",
                    fruits: {{ remove: [ {", ".join(f"{{ id: {fruit_id} }}" for fruit_id in fruit_ids_gql)} ] }}
                }}
            ) {{
                id
                name
                fruits {{
                    id
                }}
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert result.data["updateColor"] == {
        "id": to_graphql_representation(raw_colors[0]["id"], "output"),
        "name": "updated color name",
        "fruits": [],
    }

    # 1. Update specified fruit's color_id
    # 2. Update color's name
    query_tracker.assert_statements(2, "update", sql_snapshot)
    # Fetch updated color + fruit
    query_tracker.assert_statements(1, "select", sql_snapshot)


@pytest.mark.snapshot
async def test_update_with_to_many_create(
    raw_fruits: RawRecordData,
    raw_colors: RawRecordData,
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    dialect: SupportedDialect,
) -> None:
    """Tests updating a record and creating new related records for a to-many relationship."""
    color_id = raw_colors[0]["id"]
    color_id_gql = to_graphql_representation(color_id, "input")
    query = f"""
        mutation {{
            updateColor(
                data: {{
                    id: {color_id_gql},
                    name: "updated color name 2",
                    fruits: {{
                        create: [
                            {{ name: "new fruit 3 during update", sweetness: 1, waterPercent: 0.7 }},
                            {{ name: "new fruit 4 during update", sweetness: 1, waterPercent: 0.6 }}
                        ]
                    }}
                }}
            ) {{
                id
                name
                fruits {{
                    name # Check names of newly created fruits
                }}
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    # The order might vary, sort for assertion stability
    fruits_data = sorted(result.data["updateColor"]["fruits"], key=lambda x: x["name"])
    assert result.data["updateColor"]["id"] == to_graphql_representation(color_id, "output")
    assert result.data["updateColor"]["name"] == "updated color name 2"
    assert fruits_data == [
        *[{"name": fruit["name"]} for fruit in raw_fruits if fruit["color_id"] == color_id],
        {"name": "new fruit 3 during update"},
        {"name": "new fruit 4 during update"},
    ]

    # Insert 2 new fruits, in a single batched query
    if dialect in ("postgresql", "sqlite"):
        query_tracker.assert_statements(1, "insert", sql_snapshot)
    else:
        query_tracker.assert_statements(2, "insert", sql_snapshot)
    query_tracker.assert_statements(1, "update", sql_snapshot)  # Update color name
    query_tracker.assert_statements(1, "select", sql_snapshot)  # Fetch updated color + new fruits


@pytest.mark.snapshot
async def test_update_with_to_many_create_and_nested_set(
    raw_colors: RawRecordData,
    raw_farms: RawRecordData,
    raw_fruits: RawRecordData,
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Tests updating a record and creating a nested related record which itself sets a to-many relationship."""
    color_id = raw_colors[0]["id"]
    color_id_gql = to_graphql_representation(color_id, "input")
    # Use a different farm
    farm_id = raw_farms[-1]["id"]
    farm_id_gql = to_graphql_representation(farm_id, "input")
    query = f"""
        mutation {{
            updateColor(
                data: {{
                    id: {color_id_gql},
                    name: "Updated White",
                    fruits: {{
                        create: [
                            {{
                                name: "New Grape during update",
                                sweetness: 1,
                                waterPercent: 0.8,
                                farms: {{ set: [ {{ id: {farm_id_gql} }} ] }}
                            }}
                        ]
                    }}
                }}
            ) {{
                id
                name
                fruits {{
                    name
                    farms {{
                        id
                    }}
                }}
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    expected_fruits = [
        # Existing fruits
        *[
            {
                "name": fruit["name"],
                "farms": [
                    {"id": to_graphql_representation(farm["id"], "output")}
                    for farm in raw_farms
                    if farm["fruit_id"] == fruit["id"]
                ],
            }
            for fruit in raw_fruits
            if fruit["color_id"] == color_id
        ],
        # New one
        {
            "name": "New Grape during update",
            "farms": [{"id": to_graphql_representation(farm_id, "output")}],
        },
    ]
    for fruit in expected_fruits:
        assert fruit in result.data["updateColor"]["fruits"]

    query_tracker.assert_statements(1, "insert", sql_snapshot)  # Insert new fruit
    query_tracker.assert_statements(2, "update", sql_snapshot)  # Update color name + fruit's farm_id
    query_tracker.assert_statements(1, "select", sql_snapshot)  # Fetch updated color + new fruit + farm


@pytest.mark.snapshot
async def test_update_with_to_many_add(
    raw_colors: RawRecordData,
    raw_fruits: RawRecordData,
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Tests updating a record and setting (replacing) a to-many relationship."""
    color_id_gql = to_graphql_representation(raw_colors[0]["id"], "input")
    # Use a different fruit to test the update
    fruit_id_gql = to_graphql_representation(raw_fruits[1]["id"], "input")
    query = f"""
        mutation {{
            updateColor(
                data: {{
                    id: {color_id_gql},
                    name: "updated color name",
                    fruits: {{ add: [{{ id: {fruit_id_gql} }}] }}
                }}
            ) {{
                id
                name
                fruits {{
                    id
                }}
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert result.data["updateColor"] == {
        "id": to_graphql_representation(raw_colors[0]["id"], "output"),
        "name": "updated color name",
        "fruits": [
            {"id": to_graphql_representation(raw_fruits[0]["id"], "output")},
            {"id": to_graphql_representation(raw_fruits[1]["id"], "output")},
        ],
    }

    # 1. Update specified fruit's color_id
    # 2. Update color's name
    query_tracker.assert_statements(2, "update", sql_snapshot)
    # Fetch updated color + fruit
    query_tracker.assert_statements(1, "select", sql_snapshot)


@pytest.mark.snapshot
async def test_update_with_to_many_add_and_create(
    raw_colors: RawRecordData,
    raw_fruits: RawRecordData,
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Tests updating a record and setting (replacing) a to-many relationship."""
    color_id_gql = to_graphql_representation(raw_colors[0]["id"], "input")
    # Use a different fruit to test the update
    fruit_id_gql = to_graphql_representation(raw_fruits[1]["id"], "input")
    query = f"""
        mutation {{
            updateColor(
                data: {{
                    id: {color_id_gql},
                    name: "updated color name",
                    fruits: {{
                        add: [{{ id: {fruit_id_gql} }}],
                        create: [{{ name: "new fruit 3 during update", sweetness: 1, waterPercent: 0.8 }}]
                    }}
                }}
            ) {{
                id
                name
                fruits {{
                    name
                }}
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert result.data["updateColor"]["id"] == to_graphql_representation(raw_colors[0]["id"], "output")
    assert result.data["updateColor"]["name"] == "updated color name"
    expected = [
        {"name": to_graphql_representation(raw_fruits[0]["name"], "output")},
        {"name": "new fruit 3 during update"},
        {"name": to_graphql_representation(raw_fruits[1]["name"], "output")},
    ]
    for fruit in expected:
        assert fruit in result.data["updateColor"]["fruits"]

    query_tracker.assert_statements(1, "insert", sql_snapshot)
    # 1. Update specified fruit's color_id
    # 2. Update color's name
    query_tracker.assert_statements(2, "update", sql_snapshot)
    # Fetch updated color + fruit
    query_tracker.assert_statements(1, "select", sql_snapshot)


@pytest.mark.snapshot
async def test_update_with_nested_mixed_relations_create(
    raw_farms: RawRecordData,
    raw_fruits: RawRecordData,
    raw_colors: RawRecordData,
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Tests updating a record and creating multiple nested relations with different structures."""
    color_id = raw_colors[0]["id"]
    color_id_gql = to_graphql_representation(color_id, "input")
    query = f"""
        mutation {{
            updateColor(
                data: {{
                    id: {color_id_gql},
                    name: "Updated White 2",
                    fruits: {{
                        create: [
                            {{
                                name: "New Grape 2",
                                sweetness: 1,
                                waterPercent: 0.8,
                                product: {{ create: {{ name: "juice" }} }}
                            }},
                            {{
                                name: "New Lychee 2",
                                sweetness: 1,
                                waterPercent: 0.7,
                                farms: {{ create: [ {{ name: "Organic farm" }} ] }}
                            }},
                        ]
                    }}
                }}
            ) {{
                id
                name
                fruits {{
                    name
                    product {{ name }}
                    farms {{ name }}
                }}
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    # Sort fruits for assertion stability
    fruits_data = sorted(result.data["updateColor"]["fruits"], key=lambda x: x["name"])
    assert result.data["updateColor"]["id"] == to_graphql_representation(color_id, "output")
    assert result.data["updateColor"]["name"] == "Updated White 2"
    expected_fruits = [
        # Existing fruits
        *[
            {
                "name": fruit["name"],
                "farms": [
                    {"name": to_graphql_representation(farm["name"], "output")}
                    for farm in raw_farms
                    if farm["fruit_id"] == fruit["id"]
                ],
                "product": None,
            }
            for fruit in raw_fruits
            if fruit["color_id"] == color_id
        ],
        # New ones
        {"name": "New Grape 2", "product": {"name": "juice"}, "farms": []},
        {"name": "New Lychee 2", "product": None, "farms": [{"name": "Organic farm"}]},
    ]
    for fruit in expected_fruits:
        assert fruit in fruits_data

    # Heterogeneous params means inserts cannot be batched
    query_tracker.assert_statements(4, "insert", sql_snapshot)  # product, farm, fruit1, fruit2
    query_tracker.assert_statements(1, "update", sql_snapshot)  # update color name
    query_tracker.assert_statements(1, "select", sql_snapshot)  # fetch updated color + new relations


@pytest.mark.snapshot
async def test_update_many(
    raw_colors: RawRecordData,
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Tests updating multiple records in a single mutation."""
    color_id1_gql = to_graphql_representation(raw_colors[0]["id"], "input")
    color_id2_gql = to_graphql_representation(raw_colors[1]["id"], "input")
    query = f"""
        mutation {{
            updateColors(
                data: [
                    {{ id: {color_id1_gql}, name: "batch updated color 1" }},
                    {{ id: {color_id2_gql}, name: "batch updated color 2" }}
                ]
            ) {{
                id
                name
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    # Order might not be guaranteed, sort by ID
    updated_colors = sorted(result.data["updateColors"], key=lambda x: x["id"])
    expected_colors = sorted(
        [
            {"id": to_graphql_representation(raw_colors[0]["id"], "output"), "name": "batch updated color 1"},
            {"id": to_graphql_representation(raw_colors[1]["id"], "output"), "name": "batch updated color 2"},
        ],
        key=lambda x: x["id"],
    )
    assert updated_colors == expected_colors

    query_tracker.assert_statements(1, "update", sql_snapshot)  # Update colors in a single query
    query_tracker.assert_statements(1, "select", sql_snapshot)  # Fetch updated records


@pytest.mark.snapshot
async def test_update_no_init_defaults(
    raw_users: RawRecordData,
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    result = await maybe_async(
        any_async_query(
            f"""
            mutation {{
                updateUser(data: {{ id: {raw_users[3]["id"]}, name: "Jeanne" }}) {{
                    name
                    bio
                }}
            }}
            """
        )
    )
    assert not result.errors
    assert result.data
    assert result.data["updateUser"] == {"name": "Jeanne", "bio": raw_users[3]["bio"]}

    query_tracker.assert_statements(1, "update", sql_snapshot)


@pytest.mark.snapshot
async def test_update_with_secondary_table_set(
    raw_users: RawRecordData, any_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    query = f"""
    mutation {{
        updateUser(data: {{ id: {raw_users[1]["id"]}, departments: {{ set: [ {{ id: 1 }} ] }} }}) {{
            name
            departments {{
                id
                name
            }}
        }}
    }}
    """

    result = await maybe_async(any_query(query))
    assert not result.errors
    assert result.data

    assert result.data["updateUser"] == {"name": "Bob", "departments": [{"id": 1, "name": "IT"}]}

    assert query_tracker.query_count == 3
    # Insert new m2m relation
    query_tracker.assert_statements(1, "insert", sql_snapshot)
    # Delete old ones
    query_tracker.assert_statements(1, "delete", sql_snapshot)
    # Select result
    query_tracker.assert_statements(1, "select", sql_snapshot)


# Delete


@pytest.mark.parametrize(
    ("dto_filter", "deleted_names"),
    [
        pytest.param("{ name: { in: [] } }", [], id="in"),
        pytest.param("{ _not: { bio: { in: [] } } }", ["Alice", "Bob", "Charlie", "Tango"], id="not-in"),
        pytest.param("{ bio: { isNull: null } }", ["Alice", "Bob", "Charlie", "Tango"], id="is-null-null"),
        pytest.param(
            "{ _not: { name: { like: null, regexp: null } } }", ["Alice", "Bob", "Charlie", "Tango"], id="not-null"
        ),
        pytest.param(
            "{ departments: { name: { eq: null } } }", ["Alice", "Bob", "Charlie", "Tango"], id="to-many-null"
        ),
    ],
)
async def test_delete_users_by_empty_list_filter(
    dto_filter: str, deleted_names: list[str], raw_users: RawRecordData, any_async_query: AnyQueryExecutor
) -> None:
    result = await maybe_async(any_async_query(f"mutation {{ deleteUsersFilter(filter: {dto_filter}) {{ name }} }}"))
    assert not result.errors
    assert result.data
    assert sorted(user["name"] for user in result.data["deleteUsersFilter"]) == deleted_names

    result = await maybe_async(any_async_query("{ users { name } }"))
    assert not result.errors
    assert result.data
    assert sorted(user["name"] for user in result.data["users"]) == sorted(
        user["name"] for user in raw_users if user["name"] not in deleted_names
    )


@pytest.mark.snapshot
async def test_delete_filter(
    raw_users: RawRecordData,
    raw_groups: RawRecordData,
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    dialect: SupportedDialect,
) -> None:
    query = """
        mutation {
            deleteUsersFilter(
                filter: {
                    name: { eq: "Alice" }
                }
            ) {
                id
                name
                group {
                    name
                }
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert len(result.data["deleteUsersFilter"]) == 1
    apple_fruit = next(fruit for fruit in raw_users if fruit["name"] == "Alice")
    assert result.data["deleteUsersFilter"] == [
        {"id": apple_fruit["id"], "name": "Alice", "group": {"name": raw_groups[0]["name"]}}
    ]

    query_tracker.assert_statements(1, "delete", sql_snapshot)
    if dialect in ("postgresql", "sqlite"):
        query_tracker.assert_statements(1, "select", sql_snapshot)
    else:
        query_tracker.assert_statements(2, "select", sql_snapshot)


async def test_update_and_delete_filter_without_returning(
    raw_colors: RawRecordData,
    raw_users: RawRecordData,
    any_query: AnyQueryExecutor,
    any_session: AnySession,
    query_tracker: QueryTracker,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """On a dialect without UPDATE/DELETE RETURNING, filtered mutations select the affected rows before running."""
    dialect = any_session.get_bind().dialect
    monkeypatch.setattr(dialect, "update_returning", False)
    monkeypatch.setattr(dialect, "delete_returning", False)

    result = await maybe_async(
        any_query('mutation { updateColorsFilter(data: { name: "updated" }, filter: { name: { eq: "Red" } }) { id } }')
    )
    assert not result.errors
    assert result.data
    assert result.data["updateColorsFilter"] == [{"id": to_graphql_representation(raw_colors[0]["id"], "output")}]

    result = await maybe_async(any_query('mutation { deleteUsersFilter(filter: { name: { eq: "Alice" } }) { id } }'))
    assert not result.errors
    assert result.data
    alice = next(user for user in raw_users if user["name"] == "Alice")
    assert result.data["deleteUsersFilter"] == [{"id": to_graphql_representation(alice["id"], "output")}]

    assert query_tracker.filter("update").query_count == 1
    assert query_tracker.filter("delete").query_count == 1
    assert all("RETURNING" not in query.statement_str for query in query_tracker)


@pytest.mark.parametrize(
    ("first", "second", "above", "updated_names"),
    [
        pytest.param("Banana", "Lemon", 2, ["Yellow"], id="count-passes"),
        pytest.param("Banana", "Lemon", 3, [], id="count-fails"),
        pytest.param("Apple", "Lemon", 1, [], id="fruits-of-two-colors"),
    ],
)
async def test_update_filter_and_branch_counting_a_shared_to_many(
    first: str,
    second: str,
    above: int,
    updated_names: list[str],
    raw_colors: RawRecordData,
    any_async_query: AnyQueryExecutor,
) -> None:
    """An update filtered on a fruit and an ``_and`` branch counting fruits and testing another updates the matches."""
    count = f"fruitsAggregate: {{ count: {{ predicate: {{ gt: {above} }} }} }}"
    dto_filter = f'{{ fruits: {{ name: {{ eq: "{first}" }} }}, _and: [{{ {count}, fruits: {{ name: {{ eq: "{second}" }} }} }}] }}'

    result = await maybe_async(
        any_async_query(
            f'mutation {{ updateColorsFilter(data: {{ name: "updated" }}, filter: {dto_filter}) {{ id }} }}'
        )
    )

    assert not result.errors
    assert result.data
    assert result.data["updateColorsFilter"] == [
        {"id": to_graphql_representation(color["id"], "output")}
        for color in raw_colors
        if color["name"] in updated_names
    ]


@pytest.mark.parametrize(
    ("raw_users", "raw_user_departments"),
    [
        pytest.param(
            [
                {"id": 1, "name": "Alice", "group_id": 1, "bio": None},
                {"id": 2, "name": "Bob", "group_id": 1, "bio": None},
                {"id": 3, "name": "Charlie", "group_id": 2, "bio": None},
                {"id": 4, "name": "Dave", "group_id": 1, "bio": None},
                {"id": 5, "name": "Eve", "group_id": None, "bio": None},
            ],
            [
                {"user_id": 1, "department_id": 1},
                {"user_id": 2, "department_id": 2},
                {"user_id": 3, "department_id": 1},
                {"user_id": 4, "department_id": 1},
                {"user_id": 4, "department_id": 3},
                {"user_id": 5, "department_id": 3},
            ],
            id="dave-in-two-departments",
        )
    ],
)
async def test_update_filter_counting_the_table_next_to_two_to_many_paths(
    any_async_query: AnyQueryExecutor,
    raw_users: RawRecordData,  # noqa: ARG001
    raw_user_departments: RawRecordData,  # noqa: ARG001
) -> None:
    """An update filtered on a department, a topic of the group and a department count updates the users passing all."""
    dto_filter = (
        '{ departments: { name: { eq: "IT" } }, group: { topics: { name: { eq: "Hello!" } } }, '
        "departmentsAggregate: { count: { predicate: { gt: 1 } } } }"
    )

    result = await maybe_async(
        any_async_query(f'mutation {{ updateUsersFilter(data: {{ name: "updated" }}, filter: {dto_filter}) {{ id }} }}')
    )

    assert not result.errors
    assert result.data
    assert result.data["updateUsersFilter"] == [{"id": to_graphql_representation(4, "output")}]


@pytest.mark.parametrize(
    ("dto_filter", "deleted_names"),
    [
        pytest.param('{ name: { eq: "Alice" } }', ["Alice"], id="name"),
        pytest.param("{ bio: { isNull: false } }", ["Tango"], id="bio"),
    ],
)
async def test_delete_filter_on_unselected_column(
    dto_filter: str, deleted_names: list[str], raw_users: RawRecordData, any_async_query: AnyQueryExecutor
) -> None:
    """Deleting by a filter on a column the selection does not read returns the ids of the deleted rows."""
    result = await maybe_async(any_async_query(f"mutation {{ deleteUsersFilter(filter: {dto_filter}) {{ id }} }}"))
    assert not result.errors
    assert result.data
    assert result.data["deleteUsersFilter"] == [
        {"id": to_graphql_representation(user["id"], "output")} for user in raw_users if user["name"] in deleted_names
    ]

    result = await maybe_async(any_async_query("{ users { name } }"))
    assert not result.errors
    assert result.data
    assert sorted(user["name"] for user in result.data["users"]) == sorted(
        user["name"] for user in raw_users if user["name"] not in deleted_names
    )


async def test_delete_restricted_by_repository_statement(any_async_query: AnyQueryExecutor) -> None:
    """A delete whose repository statement keeps only Bob deletes Bob alone, whatever rows the filter matches."""
    result = await maybe_async(
        any_async_query('mutation { deleteUsersRestricted(filter: { name: { neq: "zzz" } }) { name } }')
    )
    assert not result.errors
    assert result.data
    assert result.data["deleteUsersRestricted"] == [{"name": "Bob"}]

    result = await maybe_async(any_async_query("{ users { name } }"))
    assert not result.errors
    assert result.data
    assert sorted(user["name"] for user in result.data["users"]) == ["Alice", "Charlie", "Tango"]


async def test_update_restricted_by_repository_statement(any_async_query: AnyQueryExecutor) -> None:
    """An update whose repository statement keeps only Bob updates Bob alone, whatever rows the filter matches."""
    result = await maybe_async(
        any_async_query(
            'mutation { updateUsersRestricted(data: { name: "Updated" }, filter: { name: { neq: "zzz" } }) { name } }'
        )
    )
    assert not result.errors
    assert result.data
    assert result.data["updateUsersRestricted"] == [{"name": "Updated"}]

    result = await maybe_async(any_async_query("{ users { name } }"))
    assert not result.errors
    assert result.data
    assert sorted(user["name"] for user in result.data["users"]) == ["Alice", "Charlie", "Tango", "Updated"]


@pytest.mark.snapshot
@pytest.mark.parametrize(
    ("dto_filter", "deleted_names"),
    [
        pytest.param('{ group: { name: { eq: "Group 1" } } }', ["Alice"], id="to-one"),
        pytest.param('{ departments: { name: { eq: "IT" } } }', ["Alice", "Charlie"], id="to-many"),
        pytest.param(
            '{ _or: [{ name: { eq: "Bob" } }, { group: { name: { eq: "Group 1" } } }] }',
            ["Alice", "Bob"],
            id="or-column-relation",
        ),
        pytest.param('{ _not: { group: { name: { eq: "Group 1" } } } }', ["Bob", "Charlie", "Tango"], id="not-to-one"),
        pytest.param(
            '{ departments: { users: { name: { eq: "Alice" } } } }', ["Alice", "Charlie"], id="nested-to-many"
        ),
        pytest.param('{ name: { eq: "Bob" } }', ["Bob"], id="column"),
        pytest.param(
            '{ name: { neq: "Alice" }, departments: { name: { eq: "IT" } } }', ["Charlie"], id="column-and-to-many"
        ),
        pytest.param(
            '{ _and: [{ name: { eq: "Alice" } }, { departments: { name: { eq: "IT" } } }] }',
            ["Alice"],
            id="and-column-relation",
        ),
        pytest.param(
            '{ _and: [{ _and: [{ name: { neq: "Alice" } }, { name: { neq: "Bob" } }] }, '
            '{ departments: { name: { in: ["IT", "Sales"] } } }] }',
            ["Charlie"],
            id="nested-and-column-relation",
        ),
        pytest.param(
            '{ name: { neq: "Alice" }, _or: [{ name: { eq: "Bob" } }, { group: { name: { eq: "Group 1" } } }] }',
            ["Bob"],
            id="column-and-or-column-relation",
        ),
        pytest.param(
            '{ _not: { name: { eq: "Bob" } }, departments: { name: { in: ["IT", "Sales"] } } }',
            ["Alice", "Charlie"],
            id="not-column-and-relation",
        ),
        pytest.param(
            '{ name: { neq: "Bob" }, _not: { group: { name: { eq: "Group 1" } } } }',
            ["Charlie", "Tango"],
            id="column-and-not-to-one",
        ),
        pytest.param(
            '{ name: { neq: "Alice" }, departmentsAggregate: { count: { arguments: [id], predicate: { gt: 1 } } } }',
            ["Charlie"],
            id="column-and-aggregation",
        ),
        pytest.param("{ departments: { name: { isNull: true } } }", [], id="to-many-null"),
        pytest.param(
            '{ name: { neq: "Zed" }, departments: { name: { isNull: true } } }', [], id="column-and-to-many-null"
        ),
        pytest.param(
            '{ name: { neq: "Zed" }, departments: { _or: [{ name: { isNull: true } }, { name: { eq: "IT" } }] } }',
            ["Alice", "Charlie"],
            id="column-and-to-many-or-null",
        ),
        pytest.param(
            '{ group: { name: { isNull: true } }, departments: { name: { eq: "IT" } } }',
            [],
            id="to-one-null-and-to-many",
        ),
        pytest.param("{ group: { name: { isNull: true } } }", [], id="to-one-null"),
        pytest.param('{ name: { neq: "Zed" }, group: { name: { isNull: true } } }', [], id="column-and-to-one-null"),
        pytest.param(
            '{ _or: [{ name: { eq: "Bob" } }, { group: { name: { isNull: true } } }] }', ["Bob"], id="or-to-one-null"
        ),
        pytest.param(
            '{ name: { neq: "Zed" }, _not: { group: { name: { isNull: true } } } }',
            ["Alice", "Bob", "Charlie", "Tango"],
            id="column-and-not-to-one-null",
        ),
        pytest.param('{ group: { _not: { name: { eq: "Group 1" } } } }', [], id="to-one-not"),
        pytest.param('{ departments: { _not: { name: { eq: "IT" } } } }', ["Bob", "Charlie"], id="to-many-not"),
        pytest.param(
            '{ departments: { _or: [{ name: { eq: "Sales" } }, { _not: { name: { isNull: false } } }] } }',
            ["Bob"],
            id="to-many-or-not",
        ),
        pytest.param(
            "{ departments: { usersAggregate: { count: { arguments: [id], predicate: { lt: 2 } } } } }",
            ["Bob", "Charlie"],
            id="to-many-aggregation",
        ),
        pytest.param(
            "{ group: { topicsAggregate: { count: { arguments: [id], predicate: { eq: 0 } } } } }",
            [],
            id="to-one-aggregation",
        ),
        pytest.param(
            '{ _or: [{ name: { eq: "Bob" } }, '
            "{ group: { topicsAggregate: { count: { arguments: [id], predicate: { eq: 0 } } } } }] }",
            ["Bob"],
            id="or-to-one-aggregation",
        ),
        pytest.param(
            '{ _or: [{ group: { name: { eq: "Group 1" } } }, { departments: { name: { eq: "Sales" } } }] }',
            ["Alice", "Bob"],
            id="or-to-one-and-to-many",
        ),
        pytest.param(
            "{ _or: [{ departmentsAggregate: { count: { arguments: [id], predicate: { eq: 0 } } } }, "
            "{ group: { name: { isNull: true } } }] }",
            ["Tango"],
            id="or-aggregation-and-to-one-null",
        ),
        pytest.param(
            "{ _or: [{ departmentsAggregate: { count: { arguments: [id], predicate: { eq: 0 } } } }, "
            '{ group: { name: { eq: "Group 1" } } }] }',
            ["Alice", "Tango"],
            id="or-aggregation-and-to-one",
        ),
        pytest.param('{ _not: { departments: { name: { eq: "IT" } } } }', ["Bob", "Tango"], id="not-to-many"),
        pytest.param(
            '{ _not: { departments: { users: { name: { eq: "Alice" } } } } }', ["Bob", "Tango"], id="not-nested-to-many"
        ),
        pytest.param(
            '{ name: { neq: "Tango" }, _not: { departments: { name: { eq: "IT" } } } }',
            ["Bob"],
            id="column-and-not-to-many",
        ),
        pytest.param(
            '{ departments: { _not: { users: { name: { eq: "Alice" } } } } }',
            ["Bob", "Charlie"],
            id="to-many-not-nested-to-many",
        ),
        pytest.param("{ group: { name: {} } }", ["Alice", "Bob", "Charlie", "Tango"], id="to-one-empty"),
        pytest.param("{ departments: { name: {} } }", ["Alice", "Bob", "Charlie", "Tango"], id="to-many-empty"),
        pytest.param("{ _not: { departments: {} } }", ["Alice", "Bob", "Charlie", "Tango"], id="not-to-many-empty"),
        pytest.param(
            "{ departmentsAggregate: { count: { arguments: [id], predicate: {} } } }",
            ["Alice", "Bob", "Charlie", "Tango"],
            id="aggregation-empty",
        ),
        pytest.param(
            '{ _or: [{ departments: { name: {} } }, { name: { eq: "Bob" } }] }', ["Bob"], id="or-to-many-empty"
        ),
        pytest.param("{ group: null }", ["Alice", "Bob", "Charlie", "Tango"], id="to-one-null-field"),
        pytest.param("{ departments: null }", ["Alice", "Bob", "Charlie", "Tango"], id="to-many-null-field"),
        pytest.param(
            "{ departmentsAggregate: { count: null } }",
            ["Alice", "Bob", "Charlie", "Tango"],
            id="aggregation-null-field",
        ),
        pytest.param('{ _not: null, name: { eq: "Bob" } }', ["Bob"], id="not-null-field"),
        pytest.param(
            "{ departmentsAggregate: { count: { arguments: null, predicate: { gt: 0 } } } }",
            ["Alice", "Bob", "Charlie"],
            id="aggregation-arguments-null-field",
        ),
        pytest.param('{ _or: [{ departments: null }, { name: { eq: "Bob" } }] }', ["Bob"], id="or-to-many-null-field"),
    ],
)
async def test_delete_filter_on_relation(
    dto_filter: str,
    deleted_names: list[str],
    raw_users: RawRecordData,
    any_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Test that a delete filtered on a relation deletes and returns only the rows whose relation matches."""
    query = f"""
        mutation {{
            deleteUsersFilter(filter: {dto_filter}) {{
                id
                name
            }}
        }}
    """
    result = await maybe_async(any_query(query))
    assert not result.errors
    assert result.data
    assert sorted(result.data["deleteUsersFilter"], key=lambda user: user["id"]) == [
        {"id": user["id"], "name": user["name"]} for user in raw_users if user["name"] in deleted_names
    ]
    query_tracker.assert_statements(1, "delete", sql_snapshot)

    result = await maybe_async(any_query("{ users { name } }"))
    assert not result.errors
    assert result.data
    assert sorted(user["name"] for user in result.data["users"]) == sorted(
        user["name"] for user in raw_users if user["name"] not in deleted_names
    )


@pytest.mark.snapshot
async def test_delete_all(
    raw_users: RawRecordData,
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    dialect: SupportedDialect,
) -> None:
    query = """
        mutation {
            deleteUsers {
                id
                name
                group {
                    name
                }
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert len(result.data["deleteUsers"]) == len(raw_users)
    query_tracker.assert_statements(1, "delete", sql_snapshot)
    if dialect in ("postgresql", "sqlite"):
        query_tracker.assert_statements(1, "select", sql_snapshot)
    else:
        query_tracker.assert_statements(2, "select", sql_snapshot)


# Custom mutations


@pytest.mark.snapshot
async def test_column_override(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    query = """
        mutation {
            createBlueColor(data: { name: "New Green" }) {
                name
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert result.data["createBlueColor"] == {"name": "New Blue"}
    query_tracker.assert_statements(1, "insert", sql_snapshot)
    query_tracker.assert_statements(1, "select", sql_snapshot)


@pytest.mark.parametrize(
    ("query_name", "query"),
    [
        pytest.param(
            "createValidatedRankedUser",
            """
        mutation {
            createValidatedRankedUser(data: {  name: "batman" }) {
                ... on RankedUserType {
                    name
                    rank
                }
                ... on ValidationErrorType {
                    id
                    errors {
                        id
                        loc
                        message
                        type
                    }
                }
            }
        }
        """,
            id="validation",
        ),
        pytest.param(
            "createRankedUser",
            """
        mutation {
            createRankedUser(data: {  name: "batman" }) {
                name
                rank
            }
        }
        """,
            id="validation",
        ),
    ],
)
async def test_read_only_column_override(query_name: str, query: str, any_async_query: AnyQueryExecutor) -> None:
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert result.data[query_name] == {"name": "batman", "rank": 1}


@pytest.mark.parametrize(
    ("query_name", "query"),
    [
        pytest.param(
            "createAppleColor",
            """
                mutation {
                    createAppleColor(data: { name: "Apple Red" }) {
                        name
                        fruits {
                            name
                        }
                    }
                }
                """,
            id="not-existing",
        ),
        pytest.param(
            "createColorForExistingFruits",
            """
                mutation {
                    createColorForExistingFruits(data: { name: "Apple Red" }) {
                        name
                        fruits {
                            name
                        }
                    }
                }
                """,
            id="existing",
        ),
    ],
)
async def test_relationship_to_many_override(query_name: str, query: str, any_query: AnyQueryExecutor) -> None:
    result = await maybe_async(any_query(query))
    assert not result.errors
    assert result.data
    assert result.data[query_name] == {
        "name": "Apple Red",
        "fruits": [{"name": "New Apple"}, {"name": "New Strawberry"}],
    }


@pytest.mark.parametrize(
    ("query_name", "query"),
    [
        pytest.param(
            "createRedFruit",
            """
            mutation {
                createRedFruit(data: { name: "New Apple", sweetness: 1, waterPercent: 0.1 }) {
                    name
                    color {
                        name
                    }
                }
            }
            """,
            id="not-existing",
        ),
        pytest.param(
            "createFruitForExistingColor",
            """
            mutation {
                createFruitForExistingColor(data: { name: "New Apple", sweetness: 1, waterPercent: 0.1 }) {
                    name
                    color {
                        name
                    }
                }
            }
            """,
            id="existing",
        ),
    ],
)
async def test_relationship_to_one_override(query_name: str, query: str, any_query: AnyQueryExecutor) -> None:
    result = await maybe_async(any_query(query))
    assert not result.errors
    assert result.data
    assert result.data[query_name] == {"name": "New Apple", "color": {"name": "New Red"}}


@pytest.mark.expire_on_commit
async def test_relationship_to_one_override_expired(any_query: AnyQueryExecutor) -> None:
    """Test that a to-one override connects when the session expired the related instance."""
    result = await maybe_async(
        any_query(
            """
            mutation {
                createFruitForExistingColor(data: { name: "New Apple", sweetness: 1, waterPercent: 0.1 }) {
                    name
                    color {
                        name
                    }
                }
            }
            """
        )
    )
    assert not result.errors
    assert result.data
    assert result.data["createFruitForExistingColor"] == {"name": "New Apple", "color": {"name": "New Red"}}
