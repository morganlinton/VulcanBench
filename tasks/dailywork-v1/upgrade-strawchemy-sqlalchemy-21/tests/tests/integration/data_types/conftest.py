from __future__ import annotations

from typing import TYPE_CHECKING, cast

import pytest

from tests.integration.fixtures import ASYNC_ENGINE_PARAMS, ASYNCPG_ENGINE_PARAM

if TYPE_CHECKING:
    from pytest import FixtureRequest
    from sqlalchemy.ext.asyncio import AsyncEngine


# asyncpg and psycopg decode these column types differently, which only shows in returned values.
@pytest.fixture(name="async_engine", params=(*ASYNC_ENGINE_PARAMS, ASYNCPG_ENGINE_PARAM))
def async_engine(request: FixtureRequest) -> AsyncEngine:
    return cast("AsyncEngine", request.getfixturevalue(request.param))
