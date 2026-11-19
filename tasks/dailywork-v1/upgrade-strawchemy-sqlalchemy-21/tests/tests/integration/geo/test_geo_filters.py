from __future__ import annotations

import pytest

pytest.importorskip("geoalchemy2", reason="geoalchemy2 is not installed")


from typing import TYPE_CHECKING, Any

from sqlalchemy import Executable, Insert, MetaData, insert, text

from tests.integration.fixtures import QueryTracker
from tests.integration.geo.models import GeoModel, geo_metadata
from tests.integration.geo.types import mysql as mysql_types
from tests.integration.geo.types import postgres as postgres_types
from tests.integration.utils import to_graphql_representation
from tests.typing import AnyQueryExecutor
from tests.utils import maybe_async

if TYPE_CHECKING:
    from pytest_databases.docker.postgres import PostgresService
    from syrupy.assertion import SnapshotAssertion

    from strawchemy.typing import SupportedDialect
    from tests.integration.typing import RawRecordData


pytestmark = [pytest.mark.integration, pytest.mark.geo, pytest.mark.extras]


@pytest.fixture
def metadata() -> MetaData:
    return geo_metadata


@pytest.fixture
def postgres_database_service(namespaced_postgis_service: PostgresService) -> PostgresService:
    return namespaced_postgis_service


@pytest.fixture
def before_create_all_statements(dialect: SupportedDialect) -> list[Executable]:
    if dialect == "postgresql":
        return [text("CREATE EXTENSION IF NOT EXISTS postgis")]
    return []


@pytest.fixture
def async_query(dialect: SupportedDialect) -> type[Any]:
    if dialect == "postgresql":
        return postgres_types.AsyncGeoQuery
    if dialect == "mysql":
        return mysql_types.AsyncGeoQuery
    pytest.skip(f"Geo tests can't be run on this dialect: {dialect}")


@pytest.fixture
def sync_query(dialect: SupportedDialect) -> type[Any]:
    if dialect == "postgresql":
        return postgres_types.SyncGeoQuery
    if dialect == "mysql":
        return mysql_types.SyncGeoQuery
    pytest.skip(f"Geo tests can't be run on this dialect: {dialect}")


@pytest.fixture
def seed_insert_statements(raw_geo: RawRecordData) -> list[Insert]:
    return [insert(GeoModel).values(raw_geo)]


@pytest.mark.snapshot
async def test_no_filtering(
    any_async_query: AnyQueryExecutor,
    raw_geo: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Test that querying without filters returns all records."""
    result = await maybe_async(any_async_query("{ geoField { id } }"))
    assert not result.errors
    assert result.data
    assert len(result.data["geoField"]) == len(raw_geo)
    # Verify SQL query
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    ("field_name", "geometry", "expected_ids"),
    [
        pytest.param("polygon", {"type": "Point", "coordinates": [0.5, 0.5]}, [0], id="point-within-polygon"),
        pytest.param("multiPolygon", {"type": "Point", "coordinates": [2.5, 2.5]}, [0], id="point-within-multipolygon"),
        pytest.param("geometry", {"type": "Point", "coordinates": [5, 5]}, [0], id="point-equals-geometry-point"),
    ],
)
@pytest.mark.snapshot
async def test_contains_geometry(
    field_name: str,
    geometry: dict[str, Any],
    expected_ids: list[int],
    any_async_query: AnyQueryExecutor,
    raw_geo: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Test the contains_geometry filter.

    This tests that a geometry contains the provided geometry.
    For example, a polygon contains a point if the point is inside the polygon.
    """
    query = f"""
            {{
                geoField(filter: {{ {field_name}: {{ containsGeometry: {to_graphql_representation(geometry, "input")} }} }}) {{
                    id
                    {field_name}
                }}
            }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert len(result.data["geoField"]) == len(expected_ids)
    for i, expected_id in enumerate(expected_ids):
        assert result.data["geoField"][i]["id"] == raw_geo[expected_id]["id"]
    # Verify SQL query
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    ("field_name", "geometry", "expected_ids"),
    [
        pytest.param(
            "point",
            {"type": "Polygon", "coordinates": [[[0, 0], [0, 2], [2, 2], [2, 0], [0, 0]]]},
            [0],
            id="point-within-polygon",
        ),
        pytest.param(
            "lineString",
            {"type": "Polygon", "coordinates": [[[0, 0], [0, 3], [3, 3], [3, 0], [0, 0]]]},
            [0],
            id="linestring-within-polygon",
        ),
        pytest.param(
            "multiPoint",
            {
                "coordinates": [
                    [
                        [3.8530909369223423, 3.5077205177229587],
                        [-2.126498556883888, 3.5077205177229587],
                        [-2.126498556883888, -0.8070832671228061],
                        [3.8530909369223423, -0.8070832671228061],
                        [3.8530909369223423, 3.5077205177229587],
                    ]
                ],
                "type": "Polygon",
            },
            [0],
            id="multipoint-within-polygon",
        ),
    ],
)
@pytest.mark.snapshot
async def test_within_geometry(
    field_name: str,
    geometry: dict[str, Any],
    expected_ids: list[int],
    any_async_query: AnyQueryExecutor,
    raw_geo: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Test the within_geometry filter.

    This tests that a geometry is within the provided geometry.
    For example, a point is within a polygon if the point is inside the polygon.
    """
    query = f"""
            {{
                geoField(filter: {{ {field_name}: {{ withinGeometry: {to_graphql_representation(geometry, "input")} }} }}) {{
                    id
                    {field_name}
                }}
            }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert len(result.data["geoField"]) == len(expected_ids)
    for i, expected_id in enumerate(expected_ids):
        assert result.data["geoField"][i]["id"] == raw_geo[expected_id]["id"]
    # Verify SQL query
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_is_null(
    any_async_query: AnyQueryExecutor,
    raw_geo: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Test the isNull filter for geometry fields."""
    query = """
            {
                geoField(filter: { point: { isNull: true } }) {
                    id
                    point
                }
            }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert len(result.data["geoField"]) == 1
    assert result.data["geoField"][0]["id"] == raw_geo[1]["id"]
    assert result.data["geoField"][0]["point"] is None
    # Verify SQL query
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


async def test_not_is_null(any_async_query: AnyQueryExecutor, raw_geo: RawRecordData) -> None:
    result = await maybe_async(any_async_query("{ geoField(filter: { _not: { point: { isNull: true } } }) { id } }"))
    assert not result.errors
    assert result.data

    assert sorted(row["id"] for row in result.data["geoField"]) == sorted(
        to_graphql_representation(row["id"], "output") for row in raw_geo if row["point"] is not None
    )


@pytest.mark.parametrize("operator", ["isNull", "containsGeometry", "withinGeometry"])
@pytest.mark.parametrize("negated", [pytest.param(False, id="plain"), pytest.param(True, id="not")])
async def test_null_operator_is_ignored(
    operator: str, negated: bool, any_async_query: AnyQueryExecutor, raw_geo: RawRecordData
) -> None:
    """Test that a geo operator set to null is ignored, directly and under ``_not``."""
    dto_filter = f"{{ point: {{ {operator}: null }} }}"
    if negated:
        dto_filter = f"{{ _not: {dto_filter} }}"
    result = await maybe_async(any_async_query(f"{{ geoField(filter: {dto_filter}) {{ id }} }}"))
    assert not result.errors
    assert result.data

    assert sorted(row["id"] for row in result.data["geoField"]) == sorted(
        to_graphql_representation(row["id"], "output") for row in raw_geo
    )
