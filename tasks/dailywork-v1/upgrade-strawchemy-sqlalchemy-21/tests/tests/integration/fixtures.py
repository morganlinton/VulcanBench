# ruff: noqa: DTZ005

from __future__ import annotations

import dataclasses
import os
import platform
import re
from copy import copy
from dataclasses import dataclass
from datetime import date, datetime, time, timedelta
from time import sleep
from typing import TYPE_CHECKING, Any, Literal, TypeAlias, cast

import psycopg
import pytest
import sqlparse
from psycopg.sql import SQL, Identifier
from pytest_databases.docker.postgres import _provide_postgres_service
from pytest_lazy_fixtures import lf
from sqlalchemy import (
    URL,
    ClauseElement,
    Compiled,
    Connection,
    CursorResult,
    Delete,
    Dialect,
    Engine,
    Executable,
    Insert,
    MetaData,
    NullPool,
    Select,
    Update,
    create_engine,
    insert,
)
from sqlalchemy.event import listens_for
from sqlalchemy.ext.asyncio import AsyncConnection, AsyncEngine, AsyncSession, async_sessionmaker, create_async_engine
from sqlalchemy.orm import Session, sessionmaker
from sqlalchemy.util import immutabledict
from strawberry.scalars import JSON
from strawberry.schema.types.scalar import DEFAULT_SCALAR_REGISTRY
from typing_extensions import Self

from strawchemy.config.databases import DatabaseFeatures
from strawchemy.constants import GEO_INSTALLED
from strawchemy.schema.scalars import Date, DateTime, Interval, Time
from tests.duplicate_reads import assert_no_duplicate_reads
from tests.fixtures import DefaultQuery
from tests.integration.models import (
    Color,
    Department,
    Fruit,
    FruitFarm,
    Group,
    Topic,
    User,
    UserDepartmentJoinTable,
    metadata,
)
from tests.integration.types import AnyAsyncMutationType, AnyAsyncQueryType, AnySyncQueryType
from tests.integration.types import mysql as mysql_types
from tests.integration.types import postgres as postgres_types
from tests.integration.types import sqlite as sqlite_types
from tests.typing import AnyQueryExecutor
from tests.utils import generate_query

if TYPE_CHECKING:
    from collections.abc import AsyncGenerator, Generator, Iterator
    from pathlib import Path

    from pytest import FixtureRequest
    from pytest_databases._service import DockerService
    from pytest_databases.docker.mysql import MySQLService
    from pytest_databases.docker.postgres import PostgresService
    from pytest_databases.types import XdistIsolationLevel
    from syrupy.assertion import SnapshotAssertion

    from strawchemy import Strawchemy, StrawchemyConfig
    from strawchemy.repository.typing import AnySession
    from strawchemy.typing import SupportedDialect
    from tests.integration.typing import RawRecordData

__all__ = (
    "ASYNCPG_ENGINE_PARAM",
    "ASYNC_ENGINE_PARAMS",
    "QueryTracker",
    "any_async_query",
    "any_async_session",
    "any_query",
    "async_engine",
    "async_session",
    "asyncpg_engine",
    "engine",
    "expire_on_commit",
    "psycopg_async_engine",
    "psycopg_engine",
    "raw_colors",
    "raw_fruits",
    "raw_users",
    "seed_db_async",
    "seed_db_sync",
    "session",
)

DATABASE_NAMESPACE_ENV_VAR = "STRAWCHEMY_TEST_DB_NAMESPACE"

_DATABASE_NAMESPACE_PATTERN = re.compile(r"[A-Za-z0-9_]+")

FilterableStatement: TypeAlias = Literal["insert", "update", "select", "delete"]
scalar_overrides: dict[object, Any] = {
    dict[str, Any]: DEFAULT_SCALAR_REGISTRY[JSON],
    timedelta: Interval,
    time: Time,
    date: Date,
    datetime: DateTime,
}
engine_plugins: list[str] = []

if GEO_INSTALLED:
    from strawchemy.schema.scalars.geo import GEO_SCALAR_OVERRIDES

    engine_plugins = ["geoalchemy2"]
    scalar_overrides |= GEO_SCALAR_OVERRIDES

# Mock data


GEO_DATA = [
    # Complete record with all geometry types
    {
        "id": 1,
        "point_required": "POINT(0 0)",  # Origin point
        "point": "POINT(1 1)",  # Simple point
        "line_string": "LINESTRING(0 0, 1 1, 2 2)",  # Simple line with 3 points
        "polygon": "POLYGON((0 0, 0 1, 1 1, 1 0, 0 0))",  # Simple square
        "multi_point": "MULTIPOINT((0 0), (1 1), (2 2))",  # 3 points
        "multi_line_string": "MULTILINESTRING((0 0, 1 1), (2 2, 3 3))",  # 2 lines
        "multi_polygon": "MULTIPOLYGON(((0 0, 0 1, 1 1, 1 0, 0 0)), ((2 2, 2 3, 3 3, 3 2, 2 2)))",  # 2 squares
        "geometry": "POINT(5 5)",  # Using point as generic geometry
    },
    # Record with only required fields
    {
        "id": 2,
        "point_required": "POINT(10 20)",  # Required point
        "point": None,
        "line_string": None,
        "polygon": None,
        "multi_point": None,
        "multi_line_string": None,
        "multi_polygon": None,
        "geometry": None,
    },
    # Record with complex geometries
    {
        "id": 3,
        "point_required": "POINT(-122.6 45.5)",  # Real-world coordinates (Portland, OR)
        "point": "POINT(-74.0060 40.7128)",  # New York City
        "line_string": "LINESTRING(-122.4194 37.7749, -118.2437 34.0522, -74.0060 40.7128)",  # SF to LA to NYC
        "polygon": "POLYGON((-122.4194 37.7749, -122.4194 37.8, -122.4 37.8, -122.4 37.7749, -122.4194 37.7749))",
        # Area in SF
        "multi_point": "MULTIPOINT((-122.4194 37.7749), (-118.2437 34.0522), (-74.0060 40.7128))",  # Major US cities
        "multi_line_string": "MULTILINESTRING((-122.4194 37.7749, -118.2437 34.0522), (-118.2437 34.0522, -74.0060 40.7128))",
        # Route segments
        "multi_polygon": "MULTIPOLYGON(((-122.42 37.78, -122.42 37.8, -122.4 37.8, -122.4 37.78, -122.42 37.78)), ((-118.25 34.05, -118.25 34.06, -118.24 34.06, -118.24 34.05, -118.25 34.05)))",
        # Areas in SF and LA
        "geometry": "LINESTRING(-122.4194 37.7749, -74.0060 40.7128)",  # Direct SF to NYC
    },
]


ASYNC_ENGINE_PARAMS = (
    pytest.param(
        "aiosqlite_engine",
        marks=[
            pytest.mark.aiosqlite,
            pytest.mark.integration,
            pytest.mark.xdist_group("sqlite"),
        ],
    ),
    pytest.param(
        "psycopg_async_engine",
        marks=[
            pytest.mark.psycopg_async,
            pytest.mark.integration,
            pytest.mark.xdist_group("postgres"),
        ],
    ),
    pytest.param(
        "asyncmy_engine",
        marks=[
            pytest.mark.asyncmy,
            pytest.mark.integration,
            pytest.mark.xdist_group("mysql"),
        ],
    ),
)
ASYNCPG_ENGINE_PARAM = pytest.param(
    "asyncpg_engine",
    marks=[
        pytest.mark.asyncpg,
        pytest.mark.integration,
        pytest.mark.xdist_group("postgres"),
    ],
)


def _requested_session(request: FixtureRequest) -> AnySession:
    return request.getfixturevalue("any_session" if "any_session" in request.fixturenames else "any_async_session")


def _query_executor(
    session: Session | AsyncSession,
    request: FixtureRequest,
    sync_query: type[Any],
    async_query: type[Any],
    sync_mutation: type[Any] | None,
    async_mutation: type[Any] | None,
) -> AnyQueryExecutor:
    if isinstance(session, AsyncSession):
        request.getfixturevalue("seed_db_async")
        return generate_query(
            session=session, query=async_query, mutation=async_mutation, scalar_overrides=scalar_overrides
        )
    request.getfixturevalue("seed_db_sync")

    return generate_query(session=session, query=sync_query, mutation=sync_mutation, scalar_overrides=scalar_overrides)


@pytest.fixture
def raw_topics(raw_groups: RawRecordData) -> RawRecordData:
    return [
        {"id": 1, "name": "Hello!", "group_id": raw_groups[0]["id"]},
        {"id": 2, "name": "Problems", "group_id": raw_groups[1]["id"]},
        {"id": 3, "name": "Solution", "group_id": raw_groups[2]["id"]},
        {"id": 4, "name": "How bake bread?", "group_id": raw_groups[3]["id"]},
        {"id": 5, "name": "My new basement!", "group_id": raw_groups[4]["id"]},
    ]


@pytest.fixture
def raw_farms(raw_fruits: RawRecordData) -> RawRecordData:
    # Apple and Cherry share a color, so holding a different number of farms each makes an
    # aggregate over farms differ between two siblings of the same fruit collection.
    farm_counts = {1: 3, 2: 2}
    farms: RawRecordData = []
    for fruit in raw_fruits:
        for index in range(1, farm_counts.get(fruit["id"], 1) + 1):
            suffix = "" if index == 1 else f" {index}"
            farms.append({"id": len(farms) + 1, "name": f"{fruit['name']} farm{suffix}", "fruit_id": fruit["id"]})
    return farms


@pytest.fixture
def raw_groups() -> RawRecordData:
    return [
        {"id": 1, "name": "Group 1"},
        {"id": 2, "name": "Group 2"},
        {"id": 3, "name": "Group 3"},
        {"id": 4, "name": "Group 4"},
        {"id": 5, "name": "Group 5"},
    ]


@pytest.fixture
def raw_colors() -> RawRecordData:
    return [
        {"id": 1, "name": "Red"},
        {"id": 2, "name": "Yellow"},
        {"id": 3, "name": "Orange"},
        {"id": 4, "name": "Green"},
        {"id": 5, "name": "Pink"},
    ]


@pytest.fixture
def raw_fruits(raw_colors: RawRecordData) -> RawRecordData:
    return [
        {
            "id": 1,
            "created_at": datetime.now().replace(second=1, microsecond=0),
            "name": "Apple",
            "sweetness": 4,
            "water_percent": 0.84,
            "best_time_to_pick": time(hour=9),
            "color_id": raw_colors[0]["id"],
        },
        {
            "id": 2,
            "created_at": datetime.now().replace(second=2, microsecond=0),
            "name": "Cherry",
            "sweetness": 9,
            "water_percent": 0.93,
            "best_time_to_pick": time(hour=10, minute=30),
            "color_id": raw_colors[0]["id"],
        },
        {
            "id": 3,
            "created_at": datetime.now().replace(second=3, microsecond=0),
            "name": "Banana",
            "sweetness": 2,
            "water_percent": 0.75,
            "best_time_to_pick": time(hour=17, minute=15),
            "color_id": raw_colors[1]["id"],
        },
        {
            "id": 4,
            "created_at": datetime.now().replace(second=4, microsecond=0),
            "name": "Lemon",
            "sweetness": 1,
            "water_percent": 0.88,
            "best_time_to_pick": time(hour=20),
            "color_id": raw_colors[1]["id"],
        },
        {
            "id": 5,
            "created_at": datetime.now().replace(second=5, microsecond=0),
            "name": "Quince",
            "sweetness": 3,
            "water_percent": 0.81,
            "best_time_to_pick": time(hour=13),
            "color_id": raw_colors[1]["id"],
        },
        {
            "id": 6,
            "created_at": datetime.now().replace(second=6, microsecond=0),
            "name": "Orange",
            "sweetness": 8,
            "water_percent": 0.86,
            "best_time_to_pick": time(hour=12, minute=12),
            "color_id": raw_colors[2]["id"],
        },
        {
            "id": 7,
            "created_at": datetime.now().replace(second=7, microsecond=0),
            "name": "clementine",
            "sweetness": 14,
            "water_percent": 0.9,
            "best_time_to_pick": time(hour=0, minute=0),
            "color_id": raw_colors[2]["id"],
        },
        {
            "id": 8,
            "created_at": datetime.now().replace(second=8, microsecond=0),
            "name": "Strawberry",
            "sweetness": 5,
            "water_percent": 0.91,
            "best_time_to_pick": time(hour=9),
            "color_id": raw_colors[3]["id"],
        },
        {
            "id": 9,
            "created_at": datetime.now().replace(second=9, microsecond=0),
            "name": "Cantaloupe",
            "sweetness": 0,
            "water_percent": 0.51,
            "best_time_to_pick": time(hour=20, minute=45, second=15, microsecond=10),
            "color_id": raw_colors[3]["id"],
        },
        {
            "id": 10,
            "created_at": datetime.now().replace(second=10, microsecond=0),
            "name": "Watermelon",
            "sweetness": 7,
            "water_percent": 0.92,
            "best_time_to_pick": time(hour=19, microsecond=55),
            "color_id": raw_colors[4]["id"],
        },
        {
            "id": 11,
            "created_at": datetime.now().replace(second=11, microsecond=0),
            "name": "Pears",
            "sweetness": 11,
            "water_percent": 0.15,
            "best_time_to_pick": time(hour=2),
            "color_id": raw_colors[4]["id"],
        },
    ]


@pytest.fixture
def raw_users(raw_groups: RawRecordData) -> RawRecordData:
    return [
        {"id": 1, "name": "Alice", "group_id": raw_groups[0]["id"], "bio": None},
        {"id": 2, "name": "Bob", "group_id": None, "bio": None},
        {"id": 3, "name": "Charlie", "group_id": None, "bio": None},
        {"id": 4, "name": "Tango", "group_id": None, "bio": "Tango's bio"},
    ]


@pytest.fixture
def raw_user_departments() -> RawRecordData:
    return [
        {"user_id": 1, "department_id": 1},
        {"user_id": 2, "department_id": 2},
        {"user_id": 3, "department_id": 3},
        {"user_id": 3, "department_id": 1},
    ]


@pytest.fixture
def raw_arrays() -> RawRecordData:
    return [
        # Standard case with typical values
        {"id": 1, "array_str_col": ["one", "two", "three"]},
        # Case with negative numbers and different values
        {"id": 2, "array_str_col": ["apple", "banana", "cherry", "date"]},
        # empty array
        {"id": 3, "array_str_col": []},
    ]


@pytest.fixture
def raw_intervals() -> RawRecordData:
    return [
        # Standard case with typical values
        {"id": 1, "time_delta_col": timedelta(days=2, hours=23, minutes=59, seconds=59)},
        # Case with negative numbers and different values
        {"id": 2, "time_delta_col": timedelta(weeks=1, days=3, hours=12)},
        # empty array
        {"id": 3, "time_delta_col": timedelta(seconds=1)},
    ]


@pytest.fixture
def raw_json() -> RawRecordData:
    return [
        # Standard case with typical values
        {"id": 1, "dict_col": {"key1": "value1", "key2": 2, "nested": {"inner": "value"}, "key3": 3, "key4": None}},
        # Case with negative numbers and different values
        {"id": 2, "dict_col": {"status": "pending", "count": 0, "key3": 3, "key4": None}},
        {"id": 3, "dict_col": {"key3": 3, "key4": None}},
    ]


@pytest.fixture
def raw_date_times() -> RawRecordData:
    return [
        # Standard case with typical values
        {
            "id": 1,
            "date_col": date(2023, 1, 15),
            "time_col": time(14, 30, 45),
            "datetime_col": datetime(2023, 1, 15, 14, 30, 45),  # noqa: DTZ001
        },
        # Case with negative numbers and different values
        {
            "id": 2,
            "date_col": date(2022, 12, 31),
            "time_col": time(8, 15, 0),
            "datetime_col": datetime(2022, 12, 31, 23, 59, 59),  # noqa: DTZ001
        },
        # empty array
        {
            "id": 3,
            "date_col": date(2024, 2, 29),  # leap year
            "time_col": time(0, 0, 0),
            "datetime_col": datetime(2024, 2, 29, 0, 0, 0),  # noqa: DTZ001
        },
    ]


@pytest.fixture
def raw_departments() -> RawRecordData:
    return [
        {"id": 1, "name": "IT"},
        {"id": 2, "name": "Sales"},
        {"id": 3, "name": "Platform"},
    ]


@pytest.fixture
def raw_geo_flipped() -> RawRecordData:
    from tests.integration.geo.utils import invert_wkt_coordinates

    # Create GEO_DATA_INVERTED by inverting coordinates in GEO_DATA
    geo_data_flipped = []
    for item in GEO_DATA:
        inverted_item = item.copy()
        for key, value in item.items():
            if key != "id" and value is not None:
                inverted_item[key] = invert_wkt_coordinates(str(value))
        geo_data_flipped.append(inverted_item)
    return geo_data_flipped


@pytest.fixture
def raw_geo(dialect: SupportedDialect, raw_geo_flipped: RawRecordData) -> RawRecordData:
    if dialect == "mysql":
        return raw_geo_flipped
    return GEO_DATA


def _namespaced_postgres_service(service: PostgresService, namespace: str) -> PostgresService:
    if not namespace:
        return service
    database = f"{service.database}_{namespace}"
    with psycopg.connect(
        dbname="postgres",
        user=service.user,
        password=service.password,
        host=service.host,
        port=service.port,
        autocommit=True,
    ) as connection:
        if connection.execute("SELECT 1 FROM pg_database WHERE datname = %s", (database,)).fetchone() is None:
            connection.execute(SQL("CREATE DATABASE {}").format(Identifier(database)))
    return dataclasses.replace(service, database=database)


def _namespaced_mysql_service(service: MySQLService, namespace: str) -> MySQLService:
    if not namespace:
        return service
    database = f"{service.db}_{namespace}"
    command = [
        "mysql",
        f"--user={service.user}",
        f"--password={service.password}",
        "-e",
        f"CREATE DATABASE IF NOT EXISTS {database}",
    ]
    # mysqld restarts once it is done initialising, so an exec can land while the server is down
    # even though the service fixture has just reached it.
    last_output: object = b""
    for attempt in range(15):
        result = service.container.exec_run(command)
        if result.exit_code == 0:
            return dataclasses.replace(service, db=database)
        last_output = result.output
        sleep(1 + attempt * 0.5)
    msg = f"Could not create MySQL database {database!r}: {last_output!r}"
    raise RuntimeError(msg)


@pytest.fixture(autouse=False, scope="session")
def postgis_image() -> str:
    repo = "imresamu/postgis-arm64" if "arm" in platform.processor().lower() else "postgis/postgis"
    return f"{repo}:17-3.5"


@pytest.fixture(scope="session")
def postgis_service(
    docker_service: DockerService,
    xdist_postgres_isolation_level: XdistIsolationLevel,
    postgres_user: str,
    postgres_password: str,
    postgres_host: str,
    postgis_image: str,
) -> Generator[PostgresService]:
    with _provide_postgres_service(
        docker_service,
        image=postgis_image,
        name="postgis-17",
        host=postgres_host,
        user=postgres_user,
        password=postgres_password,
        xdist_postgres_isolate=xdist_postgres_isolation_level,
    ) as service:
        yield service


@pytest.fixture(scope="session")
def database_namespace() -> str:
    """Suffix isolating this checkout's databases from those of any run sharing the containers.

    Raises:
        pytest.UsageError: If the namespace can't be used in a database name.
    """
    namespace = os.environ.get(DATABASE_NAMESPACE_ENV_VAR, "")
    if namespace and not _DATABASE_NAMESPACE_PATTERN.fullmatch(namespace):
        msg = f"{DATABASE_NAMESPACE_ENV_VAR} must only contain letters, digits and underscores, got {namespace!r}"
        raise pytest.UsageError(msg)
    return namespace


@pytest.fixture(scope="session")
def namespaced_postgres_service(postgres_service: PostgresService, database_namespace: str) -> PostgresService:
    return _namespaced_postgres_service(postgres_service, database_namespace)


@pytest.fixture(scope="session")
def namespaced_postgis_service(postgis_service: PostgresService, database_namespace: str) -> PostgresService:
    return _namespaced_postgres_service(postgis_service, database_namespace)


@pytest.fixture(scope="session")
def mysql_service(mysql_84_service: MySQLService, database_namespace: str) -> MySQLService:
    return _namespaced_mysql_service(mysql_84_service, database_namespace)


@pytest.fixture
def postgres_database_service(namespaced_postgres_service: PostgresService) -> PostgresService:
    return namespaced_postgres_service


# Sync engines


@pytest.fixture
def psycopg_engine(postgres_database_service: PostgresService) -> Generator[Engine]:
    """Postgresql instance for end-to-end testing."""
    engine = create_engine(
        URL(
            drivername="postgresql+psycopg",
            username="postgres",
            password=postgres_database_service.password,
            host=postgres_database_service.host,
            port=postgres_database_service.port,
            database=postgres_database_service.database,
            query=immutabledict(),
        ),
        poolclass=NullPool,
        plugins=engine_plugins,
    )
    try:
        yield engine
    finally:
        engine.dispose()


@pytest.fixture
def sqlite_engine(tmp_path: Path) -> Generator[Engine]:
    db_path = tmp_path / "test.db"
    db_path.unlink(missing_ok=True)
    engine = create_engine(f"sqlite:///{db_path}", poolclass=NullPool)
    try:
        yield engine
    finally:
        engine.dispose()


@pytest.fixture(
    name="engine",
    params=[
        pytest.param(
            "psycopg_engine",
            marks=[
                pytest.mark.psycopg_sync,
                pytest.mark.integration,
                pytest.mark.xdist_group("postgres"),
            ],
        ),
        pytest.param(
            "sqlite_engine",
            marks=[
                pytest.mark.sqlite,
                pytest.mark.integration,
                pytest.mark.xdist_group("sqlite"),
            ],
        ),
    ],
)
def engine(request: FixtureRequest) -> Engine:
    return cast("Engine", request.getfixturevalue(request.param))


@pytest.fixture
def expire_on_commit(request: FixtureRequest) -> bool:
    """Whether the session fixtures keep the SQLAlchemy default expiry, set by the marker of the same name."""
    return request.node.get_closest_marker("expire_on_commit") is not None


@pytest.fixture
def session(engine: Engine, expire_on_commit: bool) -> Generator[Session]:
    session = sessionmaker(bind=engine, expire_on_commit=expire_on_commit)()
    try:
        yield session
    finally:
        session.rollback()
        session.close()


# Async engines


@pytest.fixture
async def asyncmy_engine(mysql_service: MySQLService) -> AsyncGenerator[AsyncEngine]:
    engine = create_async_engine(
        URL(
            drivername="mysql+asyncmy",
            username=mysql_service.user,
            password=mysql_service.password,
            host=mysql_service.host,
            port=mysql_service.port,
            database=mysql_service.db,
            query=immutabledict(),
        ),
        poolclass=NullPool,
        plugins=engine_plugins,
    )
    try:
        yield engine
    finally:
        await engine.dispose()


@pytest.fixture
async def aiosqlite_engine(tmp_path: Path) -> AsyncGenerator[AsyncEngine]:
    engine = create_async_engine(f"sqlite+aiosqlite:///{tmp_path}/test.db", poolclass=NullPool)
    try:
        yield engine
    finally:
        await engine.dispose()


@pytest.fixture
async def asyncpg_engine(postgres_database_service: PostgresService) -> AsyncGenerator[AsyncEngine]:
    """Postgresql instance for end-to-end testing."""
    engine = create_async_engine(
        URL(
            drivername="postgresql+asyncpg",
            username="postgres",
            password=postgres_database_service.password,
            host=postgres_database_service.host,
            port=postgres_database_service.port,
            database=postgres_database_service.database,
            query=immutabledict(),
        ),
        poolclass=NullPool,
        plugins=engine_plugins,
    )
    try:
        yield engine
    finally:
        await engine.dispose()


@pytest.fixture
async def psycopg_async_engine(postgres_database_service: PostgresService) -> AsyncGenerator[AsyncEngine]:
    """Postgresql instance for end-to-end testing."""
    engine = create_async_engine(
        URL(
            drivername="postgresql+psycopg",
            username="postgres",
            password=postgres_database_service.password,
            host=postgres_database_service.host,
            port=postgres_database_service.port,
            database=postgres_database_service.database,
            query=immutabledict(),
        ),
        poolclass=NullPool,
        plugins=engine_plugins,
    )
    try:
        yield engine
    finally:
        await engine.dispose()


@pytest.fixture(name="async_engine", params=ASYNC_ENGINE_PARAMS)
def async_engine(request: FixtureRequest) -> AsyncEngine:
    return cast("AsyncEngine", request.getfixturevalue(request.param))


@pytest.fixture
async def async_session(async_engine: AsyncEngine, expire_on_commit: bool) -> AsyncGenerator[AsyncSession]:
    session = async_sessionmaker(bind=async_engine, expire_on_commit=expire_on_commit)()
    try:
        yield session
    finally:
        await session.rollback()
        await session.close()


# DB Seeding


@pytest.fixture
def seed_insert_statements(
    raw_fruits: RawRecordData,
    raw_colors: RawRecordData,
    raw_users: RawRecordData,
    raw_farms: RawRecordData,
    raw_groups: RawRecordData,
    raw_topics: RawRecordData,
    raw_departments: RawRecordData,
    raw_user_departments: RawRecordData,
) -> list[Insert]:
    return [
        insert(Group).values(raw_groups),
        insert(Topic).values(raw_topics),
        insert(Color).values(raw_colors),
        insert(Fruit).values(raw_fruits),
        insert(FruitFarm).values(raw_farms),
        insert(User).values(raw_users),
        insert(Department).values(raw_departments),
        insert(UserDepartmentJoinTable).values(raw_user_departments),
    ]


@pytest.fixture
def before_create_all_statements() -> list[Executable]:
    return []


@pytest.fixture(name="metadata")
def fx_metadata() -> MetaData:
    return metadata


@pytest.fixture
def seed_db_sync(
    engine: Engine,
    metadata: MetaData,
    seed_insert_statements: list[Insert],
    before_create_all_statements: list[Executable],
) -> None:
    with engine.begin() as conn:
        for statement in before_create_all_statements:
            conn.execute(statement)
        metadata.drop_all(conn)
        metadata.create_all(conn)
        for statement in seed_insert_statements:
            conn.execute(statement)


@pytest.fixture
async def seed_db_async(
    async_engine: AsyncEngine,
    metadata: MetaData,
    seed_insert_statements: list[Insert],
    before_create_all_statements: list[Executable],
) -> None:
    async with async_engine.begin() as conn:
        for statement in before_create_all_statements:
            await conn.execute(statement)
        await conn.run_sync(metadata.drop_all)
        await conn.run_sync(metadata.create_all)
        for statement in seed_insert_statements:
            await conn.execute(statement)


# Utilities


@pytest.fixture
def dialect(request: FixtureRequest) -> SupportedDialect:
    return cast("SupportedDialect", _requested_session(request).get_bind().dialect.name)


@pytest.fixture
def db_features(dialect: SupportedDialect) -> DatabaseFeatures:
    return DatabaseFeatures.new(dialect)


@pytest.fixture
def mapper(dialect: SupportedDialect) -> Strawchemy:
    if dialect == "postgresql":
        return postgres_types.strawchemy
    if dialect == "mysql":
        return mysql_types.strawchemy
    if dialect == "sqlite":
        return sqlite_types.strawchemy
    msg = f"Unknown dialect: {dialect}"
    raise ValueError(msg)


@pytest.fixture
def config(mapper: Strawchemy) -> Generator[StrawchemyConfig]:
    original_config = copy(mapper.config)
    yield mapper.config
    for field in dataclasses.fields(original_config):
        setattr(mapper.config, field.name, getattr(original_config, field.name))


@pytest.fixture
def async_query(dialect: SupportedDialect) -> type[AnyAsyncQueryType | DefaultQuery]:
    if dialect == "postgresql":
        return postgres_types.AsyncQuery
    if dialect == "mysql":
        return mysql_types.AsyncQuery
    if dialect == "sqlite":
        return sqlite_types.AsyncQuery
    return DefaultQuery


@pytest.fixture
def sync_query(dialect: SupportedDialect) -> type[AnySyncQueryType | DefaultQuery]:
    if dialect == "postgresql":
        return postgres_types.SyncQuery
    if dialect == "mysql":
        return mysql_types.SyncQuery
    if dialect == "sqlite":
        return sqlite_types.SyncQuery
    return DefaultQuery


@pytest.fixture
def async_mutation(dialect: SupportedDialect) -> type[AnyAsyncMutationType] | None:
    if dialect == "postgresql":
        return postgres_types.AsyncMutation
    if dialect == "mysql":
        return mysql_types.AsyncMutation
    if dialect == "sqlite":
        return sqlite_types.AsyncMutation
    return None


@pytest.fixture
def sync_mutation(dialect: SupportedDialect) -> type[Any] | None:
    if dialect == "postgresql":
        return postgres_types.SyncMutation
    if dialect == "mysql":
        return mysql_types.SyncMutation
    if dialect == "sqlite":
        return sqlite_types.SyncMutation
    return None


@pytest.fixture(params=[lf("async_session"), lf("session")], ids=["async", "sync"])
def any_session(request: FixtureRequest) -> AnySession:
    return request.param


@pytest.fixture(params=[lf("async_session")], ids=["async"])
def any_async_session(request: FixtureRequest) -> AsyncSession:
    return request.param


@pytest.fixture(params=[None], ids=["tracked"])
def query_tracker(request: FixtureRequest) -> QueryTracker:
    return QueryTracker(_requested_session(request))


@pytest.fixture(params=[None], ids=["session"])
def any_query(
    any_session: Session | AsyncSession,
    sync_query: type[Any],
    async_query: type[Any],
    async_mutation: type[Any] | None,
    sync_mutation: type[Any] | None,
    request: FixtureRequest,
) -> AnyQueryExecutor:
    return _query_executor(any_session, request, sync_query, async_query, sync_mutation, async_mutation)


@pytest.fixture(params=[None], ids=["session"])
def any_async_query(
    any_async_session: AsyncSession,
    sync_query: type[Any],
    async_query: type[Any],
    async_mutation: type[Any] | None,
    sync_mutation: type[Any] | None,
    request: FixtureRequest,
) -> AnyQueryExecutor:
    return _query_executor(any_async_session, request, sync_query, async_query, sync_mutation, async_mutation)


@dataclass
class QueryInspector:
    clause_element: Compiled | str | ClauseElement
    dialect: Dialect
    multiparams: list[dict[str, Any]] = dataclasses.field(default_factory=list)
    params: dict[str, Any] = dataclasses.field(default_factory=dict)

    @property
    def statement_str(self) -> str:
        compiled = self.clause_element
        if isinstance(self.clause_element, ClauseElement):
            column_keys = list(self.params) or list(self.multiparams[0] if self.multiparams else []) or None
            compiled = self.clause_element.compile(dialect=self.dialect, column_keys=column_keys)
        return str(compiled)

    @property
    def statement_formatted(self) -> str:
        return sqlparse.format(
            self.statement_str, reindent_aligned=True, use_space_around_operators=True, keyword_case="upper"
        )


@dataclass
class QueryTracker:
    session: AnySession

    executions: list[QueryInspector] = dataclasses.field(default_factory=list)

    def __post_init__(self) -> None:
        self._clause_map = {"insert": Insert, "select": Select, "update": Update, "delete": Delete}
        listens_for(self.session.get_bind(), "after_execute")(self._event_listener)

    def _event_listener(
        self,
        conn: Connection | AsyncConnection,
        clauseelement: Compiled | str | ClauseElement,
        multiparams: list[dict[str, Any]],
        params: dict[str, Any],
        execution_options: dict[str, Any],
        result: CursorResult[Any],
    ) -> None:
        if isinstance(clauseelement, Select):
            assert_no_duplicate_reads(clauseelement, conn.dialect)
        self.executions.append(QueryInspector(clauseelement, conn.dialect, multiparams, params))

    def filter(self, statement: FilterableStatement) -> Self:
        return dataclasses.replace(
            self,
            executions=[
                execution
                for execution in self.executions
                if isinstance(execution.clause_element, self._clause_map[statement])
            ],
        )

    @property
    def query_count(self) -> int:
        return len(
            [
                execution
                for execution in self.executions
                if isinstance(execution.clause_element, tuple(self._clause_map.values()))
            ]
        )

    def __getitem__(self, index: int) -> QueryInspector:
        return self.executions[index]

    def __iter__(self) -> Iterator[QueryInspector]:
        return iter(self.executions)

    def assert_statements(
        self,
        count: int,
        statement_type: FilterableStatement | None = None,
        snapshot: SnapshotAssertion | None = None,
    ) -> None:
        filtered = self.filter(statement_type) if statement_type is not None else self
        assert filtered.query_count == count
        if snapshot is None:
            return
        for query in filtered:
            assert query.statement_formatted == snapshot
