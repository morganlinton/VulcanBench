from __future__ import annotations

from typing import TYPE_CHECKING, Any

import pytest
from sqlalchemy import Insert, MetaData, insert

from tests.integration.models import JSONModel, json_metadata
from tests.integration.types import mysql as mysql_types
from tests.integration.types import postgres as postgres_types
from tests.integration.types import sqlite as sqlite_types
from tests.integration.utils import graphql_input
from tests.utils import maybe_async

if TYPE_CHECKING:
    from syrupy.assertion import SnapshotAssertion

    from strawchemy.config.databases import DatabaseFeatures
    from strawchemy.typing import SupportedDialect
    from tests.integration.fixtures import QueryTracker
    from tests.integration.typing import RawRecordData
    from tests.typing import AnyQueryExecutor


@pytest.fixture
def metadata() -> MetaData:
    return json_metadata


@pytest.fixture
def seed_insert_statements(raw_json: RawRecordData) -> list[Insert]:
    return [insert(JSONModel).values(raw_json)]


@pytest.fixture
def async_query(dialect: SupportedDialect) -> type[Any]:
    if dialect == "postgresql":
        return postgres_types.JSONAsyncQuery
    if dialect == "mysql":
        return mysql_types.JSONAsyncQuery
    if dialect == "sqlite":
        return sqlite_types.JSONAsyncQuery
    pytest.skip(f"JSON tests can't be run on this dialect: {dialect}")


@pytest.fixture
def sync_query(dialect: SupportedDialect) -> type[Any]:
    if dialect == "postgresql":
        return postgres_types.JSONSyncQuery
    if dialect == "mysql":
        return mysql_types.JSONSyncQuery
    if dialect == "sqlite":
        return sqlite_types.JSONSyncQuery
    pytest.skip(f"JSON tests can't be run on this dialect: {dialect}")


# Tests for JSON-specific filters
@pytest.mark.parametrize(
    ("filter_name", "value", "expected_ids"),
    [
        pytest.param("contains", {"key1": "value1"}, [0], id="contains"),
        pytest.param(
            "containedIn",
            {"key1": "value1", "key2": 2, "key3": 3, "key4": None, "nested": {"inner": "value"}, "extra": "value"},
            [0, 2],
            id="containedIn",
        ),
        pytest.param("hasKey", "key1", [0], id="hasKey"),
        pytest.param("hasKeyAll", ["key1", "key2"], [0], id="hasKeyAll"),
        pytest.param("hasKeyAny", ["key1", "status"], [0, 1], id="hasKeyAny"),
        pytest.param("hasKey", "key4", [0, 1, 2], id="hasKey-json-null"),
        pytest.param("hasKeyAll", ["key3", "key4"], [0, 1, 2], id="hasKeyAll-json-null"),
        pytest.param("hasKeyAny", ["key4", "missing"], [0, 1, 2], id="hasKeyAny-json-null"),
    ],
)
@pytest.mark.snapshot
async def test_json_filters(
    filter_name: str,
    value: Any,
    expected_ids: list[int],
    any_async_query: AnyQueryExecutor,
    raw_json: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    db_features: DatabaseFeatures,
) -> None:
    if db_features.dialect == "sqlite" and filter_name in {"contains", "containedIn"}:
        pytest.skip(f"contains/containedIn not supported on {db_features.dialect}")
    value_repr = graphql_input(value)
    query = f"""
        {{
            json(filter: {{ dictCol: {{ {filter_name}: {value_repr} }} }}) {{
                id
                dictCol
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert len(result.data["json"]) == len(expected_ids)

    for i, expected_id in enumerate(expected_ids):
        assert result.data["json"][i]["id"] == raw_json[expected_id]["id"]

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    ("filter_name", "value", "expected_ids"),
    [
        pytest.param("hasKey", "key1", [1, 2], id="hasKey"),
        pytest.param("hasKey", "key4", [], id="hasKey-json-null"),
        pytest.param("hasKeyAll", ["key1", "key4"], [1, 2], id="hasKeyAll-json-null"),
        pytest.param("hasKeyAny", ["key4", "missing"], [], id="hasKeyAny-json-null"),
    ],
)
@pytest.mark.snapshot
async def test_json_not_filters(
    filter_name: str,
    value: Any,
    expected_ids: list[int],
    any_async_query: AnyQueryExecutor,
    raw_json: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    query = f"""
        {{
            json(filter: {{ _not: {{ dictCol: {{ {filter_name}: {graphql_input(value)} }} }} }}) {{
                id
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert [row["id"] for row in result.data["json"]] == [raw_json[i]["id"] for i in expected_ids]

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


_KEY_ROWS: RawRecordData = [
    {"id": 1, "dict_col": {"a.b": 1}},
    {"id": 2, "dict_col": {"a": {"b": 1}}},
    {"id": 3, "dict_col": {"x y": 1}},
    {"id": 4, "dict_col": {"": 1}},
    {"id": 5, "dict_col": ["k"]},
    {"id": 6, "dict_col": {'q"t': 1}},
    {"id": 7, "dict_col": {"a[0]": 1}},
    {"id": 8, "dict_col": {"arr": [1]}},
    {"id": 9, "dict_col": {"b\\s": 1}},
    {"id": 10, "dict_col": {"k": None}},
    {"id": 11, "dict_col": {"*": 1}},
    {"id": 12, "dict_col": "k"},
    {"id": 13, "dict_col": None},
    {"id": 14, "dict_col": {"None": 1}},
    {"id": 15, "dict_col": {"é": 1}},
    {"id": 16, "dict_col": {"😀": 1}},
]
_KEY_ROW_IDS = [row["id"] for row in _KEY_ROWS]


@pytest.mark.parametrize("raw_json", [pytest.param(_KEY_ROWS, id="keys")])
@pytest.mark.parametrize(
    ("filter_name", "value", "expected_ids"),
    [
        pytest.param("hasKey", "a.b", [1], id="hasKey-dot"),
        pytest.param("hasKey", "a[0]", [7], id="hasKey-brackets"),
        pytest.param("hasKey", "arr[0]", [], id="hasKey-array-index"),
        pytest.param("hasKey", "*", [11], id="hasKey-wildcard"),
        pytest.param("hasKey", "x y", [3], id="hasKey-space"),
        pytest.param("hasKey", 'q"t', [6], id="hasKey-double-quote"),
        pytest.param("hasKey", "b\\s", [9], id="hasKey-backslash"),
        pytest.param("hasKey", "", [4], id="hasKey-empty"),
        pytest.param("hasKey", "k", [10], id="hasKey-objects-only"),
        pytest.param("hasKey", "é", [15], id="hasKey-non-ascii"),
        pytest.param("hasKeyAny", ["😀", "missing"], [16], id="hasKeyAny-non-ascii"),
        pytest.param("hasKeyAll", [], _KEY_ROW_IDS, id="hasKeyAll-empty"),
        pytest.param("hasKeyAny", [], [], id="hasKeyAny-empty"),
        pytest.param("hasKeyAll", ["a.b"], [1], id="hasKeyAll-dot"),
        pytest.param("hasKeyAll", ["k"], [10], id="hasKeyAll-objects-only"),
        pytest.param("hasKeyAny", ['q"t', "a[0]", "*"], [6, 7, 11], id="hasKeyAny-path-syntax"),
        pytest.param("hasKeyAny", ["k"], [10], id="hasKeyAny-objects-only"),
    ],
)
async def test_json_has_key_literal(
    filter_name: str,
    value: str | list[str],
    expected_ids: list[int],
    any_async_query: AnyQueryExecutor,
    raw_json: RawRecordData,  # noqa: ARG001
) -> None:
    """Test that ``hasKey*`` match literal top-level object keys, and that ``_not`` matches every other row."""
    variable_type = "String!" if isinstance(value, str) else "[String!]!"
    for dto_filter, ids in (
        (f"{{ dictCol: {{ {filter_name}: $value }} }}", expected_ids),
        (f"{{ _not: {{ dictCol: {{ {filter_name}: $value }} }} }}", [i for i in _KEY_ROW_IDS if i not in expected_ids]),
    ):
        query = f"query ($value: {variable_type}) {{ json(filter: {dto_filter}) {{ id }} }}"
        result = await maybe_async(any_async_query(query, {"value": value}))
        assert not result.errors
        assert result.data
        assert sorted(row["id"] for row in result.data["json"]) == ids


@pytest.mark.parametrize("raw_json", [pytest.param(_KEY_ROWS, id="keys")])
@pytest.mark.parametrize(
    "comparison",
    [
        pytest.param("hasKey: null", id="hasKey-null"),
        pytest.param("hasKeyAll: null", id="hasKeyAll-null"),
        pytest.param("hasKeyAny: null", id="hasKeyAny-null"),
        pytest.param("contains: null", id="contains-null"),
        pytest.param("containedIn: null", id="containedIn-null"),
    ],
)
@pytest.mark.parametrize("negated", [pytest.param(False, id="plain"), pytest.param(True, id="not")])
async def test_json_null_comparison_ignored(
    comparison: str,
    negated: bool,
    any_async_query: AnyQueryExecutor,
    raw_json: RawRecordData,  # noqa: ARG001
    db_features: DatabaseFeatures,
) -> None:
    """Test that a null JSON comparison is ignored, directly and under ``_not``."""
    if db_features.dialect == "sqlite" and comparison.startswith(("contains", "containedIn")):
        pytest.skip(f"contains/containedIn not supported on {db_features.dialect}")
    dto_filter = f"{{ dictCol: {{ {comparison} }} }}"
    if negated:
        dto_filter = f"{{ _not: {dto_filter} }}"
    result = await maybe_async(any_async_query(f"{{ json(filter: {dto_filter}) {{ id }} }}"))
    assert not result.errors
    assert result.data
    assert sorted(row["id"] for row in result.data["json"]) == _KEY_ROW_IDS


@pytest.mark.snapshot
async def test_json_output(
    any_async_query: AnyQueryExecutor,
    raw_json: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    query = """
        {
            json {
                id
                dictCol
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    for interval in result.data["json"]:
        expected_interval = next(f for f in raw_json if f["id"] == interval["id"])
        assert interval["dictCol"] == expected_interval["dict_col"]

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    "path",
    [
        pytest.param("$.key1", id="key1"),
        pytest.param("$.key3", id="key3"),
        pytest.param("$.key4", id="key4"),
        pytest.param("$.nested", id="nested"),
    ],
)
@pytest.mark.snapshot
async def test_json_extract_path(
    path: str,
    any_async_query: AnyQueryExecutor,
    raw_json: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    query = f"""
        {{
            json {{
                id
                dictCol(path: "{path}")
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    for json in result.data["json"]:
        expected_dict_col = next(f for f in raw_json if f["id"] == json["id"])
        assert json["dictCol"] == expected_dict_col["dict_col"].get(path.strip("$."), {})

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_json_extract_inner_path(
    any_async_query: AnyQueryExecutor,
    raw_json: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    query = """
        {
            json {
                id
                dictCol(path: "$.nested.inner")
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    for json in result.data["json"]:
        expected_dict_col = next(f for f in raw_json if f["id"] == json["id"])
        assert json["dictCol"] == expected_dict_col["dict_col"].get("nested", {}).get("inner", {})

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


async def test_json_extract_aliased_paths(
    any_async_query: AnyQueryExecutor, raw_json: RawRecordData, query_tracker: QueryTracker
) -> None:
    """Test that aliases of a JSON column extracting different paths each get their own path."""
    query = """
        {
            json {
                id
                key1: dictCol(path: "$.key1")
                nested: dictCol(path: "$.nested")
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    for json in result.data["json"]:
        expected_dict_col = next(f for f in raw_json if f["id"] == json["id"])["dict_col"]
        assert json["key1"] == expected_dict_col.get("key1", {})
        assert json["nested"] == expected_dict_col.get("nested", {})

    assert query_tracker.query_count == 1
