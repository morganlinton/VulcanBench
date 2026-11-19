from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

from tests.integration.fixtures import QueryTracker
from tests.typing import AnyQueryExecutor
from tests.utils import maybe_async

if TYPE_CHECKING:
    from syrupy.assertion import SnapshotAssertion

    from strawchemy.typing import SupportedDialect

pytestmark = [pytest.mark.integration]


@pytest.mark.snapshot
async def test_upsert_one_new(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    """Test upserting a single new fruit record.

    This test verifies that the upsertFruit mutation correctly creates a new fruit
    record when no existing record matches. The test expects:
    - A successful GraphQL mutation response
    - The new fruit data to be returned correctly
    - Exactly 2 SQL queries: 1 INSERT and 1 SELECT
    """
    query = """
        mutation {
            upsertFruit(
                data: {
                    name: "Grape",
                    sweetness: 7,
                    waterPercent: 0.85
                }
            ) {
                name
                sweetness
                waterPercent
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert result.data["upsertFruit"] == {
        "name": "Grape",
        "sweetness": 7,
        "waterPercent": 0.85,
    }

    assert query_tracker.query_count == 2
    query_tracker.assert_statements(1, "insert", sql_snapshot)
    query_tracker.assert_statements(1, "select", sql_snapshot)


@pytest.mark.parametrize(
    ("query", "name", "sweetness", "water_percent"),
    [
        pytest.param(
            """
            mutation {
                upsertFruit(
                    data: {
                        name: "Apple",
                        sweetness: 0,
                        waterPercent: 0
                    },
                    conflictFields: name
                ) {
                    id
                    name
                    sweetness
                    waterPercent
                }
            }
            """,
            "Apple",
            0,
            0,
            id="unique-constraint",
        ),
        pytest.param(
            """
            mutation {
                upsertFruit(
                    data: {
                        name: "Almost Apple",
                        sweetness: 4,
                        waterPercent: 0.84
                    },
                    conflictFields: sweetnessAndWaterPercent
                ) {
                    id
                    name
                    sweetness
                    waterPercent
                }
            }
            """,
            "Almost Apple",
            4,
            0.84,
            id="unique-constraint-multi-columns",
        ),
        pytest.param(
            """
            mutation {
                upsertFruit(
                    data: {
                        id: 1
                        name: "Blueberries",
                        sweetness: 0,
                        waterPercent: 0
                    },
                    conflictFields: id
                ) {
                    id
                    name
                    sweetness
                    waterPercent
                }
            }
            """,
            "Blueberries",
            0,
            0,
            id="pk-constraint",
        ),
        pytest.param(
            """
            mutation {
                upsertFruit(
                    data: {
                        id: 1
                        name: "Blueberries",
                        sweetness: 0,
                        waterPercent: 0
                    },
                    conflictFields: id
                    updateFields: [ name ]
                ) {
                    id
                    name
                    sweetness
                    waterPercent
                }
            }
            """,
            "Blueberries",
            4,
            0.84,
            id="pk-constraint-update-fields",
        ),
    ],
)
@pytest.mark.snapshot
async def test_upsert_one_existing(
    query: str,
    name: str,
    sweetness: int,
    water_percent: float,
    any_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Test upserting a single existing fruit record with conflict resolution.

    This test verifies that the upsertFruit mutation correctly updates an existing
    fruit record when a conflict is detected on the specified conflict field (name).
    The test expects:
    - A successful GraphQL mutation response with conflictFields specified
    - The existing fruit record to be updated with new values
    - The original ID to be preserved in the response
    - Exactly 2 SQL queries: 1 INSERT (with conflict resolution) and 1 SELECT
    """
    result = await maybe_async(any_query(query))
    assert not result.errors
    assert result.data
    assert result.data["upsertFruit"] == {"id": 1, "name": name, "sweetness": sweetness, "waterPercent": water_percent}

    assert query_tracker.query_count == 2
    query_tracker.assert_statements(1, "insert", sql_snapshot)
    query_tracker.assert_statements(1, "select", sql_snapshot)


@pytest.mark.snapshot
async def test_upsert_many_new(
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    dialect: SupportedDialect,
) -> None:
    """Test upserting multiple new fruit records.

    This test verifies that the upsertFruits mutation correctly creates multiple new
    fruit records when no existing records match. The test expects:
    - A successful GraphQL mutation response
    - All new fruit data to be returned in the correct order
    - Different SQL query patterns based on database dialect:
      * PostgreSQL/MySQL: 3 queries (2 INSERTs + 1 SELECT)
      * SQLite: 2 queries (1 batch INSERT + 1 SELECT)
    """
    query = """
        mutation {
            upsertFruits(
                data: [
                    {
                        name: "Grape",
                        sweetness: 7,
                        waterPercent: 0.85
                    },
                    {
                        name: "Blueberries",
                        sweetness: 6,
                        waterPercent: 0.93
                    },
                ]
            ) {
                name
                sweetness
                waterPercent
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert result.data["upsertFruits"] == [
        {"name": "Grape", "sweetness": 7, "waterPercent": 0.85},
        {"name": "Blueberries", "sweetness": 6, "waterPercent": 0.93},
    ]

    if dialect in {"postgresql", "mysql"}:
        assert query_tracker.query_count == 3
        query_tracker.assert_statements(2, "insert", sql_snapshot)
    else:
        assert query_tracker.query_count == 2
        query_tracker.assert_statements(1, "insert", sql_snapshot)
    query_tracker.assert_statements(1, "select", sql_snapshot)


@pytest.mark.snapshot
async def test_upsert_many_new_and_existing(
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    dialect: SupportedDialect,
) -> None:
    """Test upserting multiple fruits with mixed new and existing records.

    This test verifies that the upsertFruits mutation correctly handles a batch
    operation containing both new records (to be created) and existing records
    (to be updated) based on conflict resolution using the name field. The test expects:
    - A successful GraphQL mutation response with conflictFields specified
    - Mixed create/update operations to be handled correctly
    - All fruit data to be returned with updated values
    - Different SQL query patterns based on database dialect:
      * PostgreSQL/MySQL: 3 queries (2 INSERTs with conflict resolution + 1 SELECT)
      * SQLite: 2 queries (1 batch INSERT with conflict resolution + 1 SELECT)
    """
    query = """
        mutation {
            upsertFruits(
                data: [
                    {
                        name: "Apple",
                        sweetness: 7,
                        waterPercent: 0.85
                    },
                    {
                        name: "Blueberries",
                        sweetness: 6,
                        waterPercent: 0.93
                    },
                ],
                conflictFields: name
            ) {
                name
                sweetness
                waterPercent
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert result.data["upsertFruits"] == [
        {"name": "Apple", "sweetness": 7, "waterPercent": 0.85},
        {"name": "Blueberries", "sweetness": 6, "waterPercent": 0.93},
    ]

    if dialect in {"postgresql", "mysql"}:
        assert query_tracker.query_count == 3
        query_tracker.assert_statements(2, "insert", sql_snapshot)
    else:
        assert query_tracker.query_count == 2
        query_tracker.assert_statements(1, "insert", sql_snapshot)
    query_tracker.assert_statements(1, "select", sql_snapshot)


# To-One Upsert Tests


@pytest.mark.snapshot
async def test_to_one_upsert_create_new(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    """Tests upsert mutation that creates a new record when no match is found."""
    query = """
        mutation {
            updateFruit(
                data: {
                    id: 1,
                    name: "Grape",
                    sweetness: 7,
                    waterPercent: 0.85,
                    color: {
                        upsert: {
                            create: { name: "Purple" }
                        }
                    }
                }
            ) {
                id
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
    assert result.data["updateFruit"] == {
        "id": 1,
        "name": "Grape",
        "color": {"name": "Purple"},
    }

    query_tracker.assert_statements(1, "insert", sql_snapshot)  # Insert new color
    query_tracker.assert_statements(1, "update", sql_snapshot)  # Update fruit's color_id
    query_tracker.assert_statements(1, "select", sql_snapshot)  # Fetch updated fruit + new color


@pytest.mark.snapshot
async def test_to_one_upsert_update_existing(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    """Tests upsert mutation that updates an existing record when a match is found."""
    query = """
        mutation {
            updateFruit(
                data: {
                    id: 1,
                    name: "Apple - Granny smith",
                    color: {
                        upsert: {
                            create: { name: "Green" }
                            conflictFields: name
                        }
                    }
                }
            ) {
                id
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
    assert result.data["updateFruit"] == {
        "id": 1,
        "name": "Apple - Granny smith",
        "color": {"name": "Green"},
    }

    query_tracker.assert_statements(1, "update", sql_snapshot)
    query_tracker.assert_statements(1, "update", sql_snapshot)
    query_tracker.assert_statements(1, "select", sql_snapshot)


# To-Many Upsert Tests


@pytest.mark.snapshot
async def test_to_many_upsert_create_new(
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    dialect: SupportedDialect,
) -> None:
    """Tests to-many upsert mutation that creates new records."""
    query = """
        mutation {
            updateColor(
                data: {
                    id: 1,
                    name: "Bright Red",
                    fruits: {
                        upsert: {
                            create: [
                                { name: "Pomegranate", sweetness: 6, waterPercent: 0.82 },
                                { name: "Plums", sweetness: 7, waterPercent: 0.87 }
                            ]
                            conflictFields: name
                        }
                    }
                }
            ) {
                id
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

    assert result.data["updateColor"] == {
        "id": 1,
        "name": "Bright Red",
        "fruits": [{"name": "Apple"}, {"name": "Cherry"}, {"name": "Pomegranate"}, {"name": "Plums"}],
    }

    if dialect == "sqlite":
        query_tracker.assert_statements(1, "insert", sql_snapshot)
    else:
        query_tracker.assert_statements(2, "insert", sql_snapshot)

    query_tracker.assert_statements(1, "update", sql_snapshot)  # Update color name
    query_tracker.assert_statements(1, "select", sql_snapshot)  # Fetch updated color + new fruits


@pytest.mark.snapshot
async def test_to_many_upsert_update_existing(
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    dialect: SupportedDialect,
) -> None:
    """Tests to-many upsert mutation that updates existing records."""
    query = """
        mutation {
            updateColor(
                data: {
                    id: 1,
                    name: "Bright Red",
                    fruits: {
                        upsert: {
                            create: [
                                { name: "Apple", sweetness: 6, waterPercent: 0.82 },
                                { name: "Cherry", sweetness: 7, waterPercent: 0.87 }
                            ]
                            conflictFields: name
                        }
                    }
                }
            ) {
                id
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

    assert result.data["updateColor"] == {
        "id": 1,
        "name": "Bright Red",
        "fruits": [{"name": "Apple"}, {"name": "Cherry"}],
    }

    if dialect == "sqlite":
        query_tracker.assert_statements(1, "insert", sql_snapshot)
    else:
        query_tracker.assert_statements(2, "insert", sql_snapshot)

    query_tracker.assert_statements(1, "update", sql_snapshot)  # Update color name
    query_tracker.assert_statements(1, "select", sql_snapshot)  # Fetch updated color + new fruits


@pytest.mark.snapshot
async def test_to_many_upsert_mixed_create_and_update(
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    dialect: SupportedDialect,
) -> None:
    """Tests to-many upsert mutation that both creates new records and updates existing ones."""
    query = """
        mutation {
            updateColor(
                data: {
                    id: 1,
                    name: "Bright Red",
                    fruits: {
                        upsert: {
                            create: [
                                { name: "Plums", sweetness: 6, waterPercent: 0.82 },
                                { name: "Cherry", sweetness: 7, waterPercent: 0.87 }
                            ]
                            conflictFields: name
                        }
                    }
                }
            ) {
                id
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

    assert result.data["updateColor"] == {
        "id": 1,
        "name": "Bright Red",
        "fruits": [{"name": "Apple"}, {"name": "Cherry"}, {"name": "Plums"}],
    }

    if dialect == "sqlite":
        query_tracker.assert_statements(1, "insert", sql_snapshot)
    else:
        query_tracker.assert_statements(2, "insert", sql_snapshot)

    query_tracker.assert_statements(1, "update", sql_snapshot)  # Update color name
    query_tracker.assert_statements(1, "select", sql_snapshot)  # Fetch updated color + new fruits
