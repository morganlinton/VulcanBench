from __future__ import annotations

import pytest

pytest.importorskip("geoalchemy2", reason="geoalchemy2 is not installed")
pytest.importorskip("pydantic", reason="pydantic is not installed")

from strawberry import Schema

from tests.integration.fixtures import scalar_overrides
from tests.integration.types import mysql, postgres

pytestmark = [pytest.mark.extras]


def test_schema() -> None:
    for types in (postgres, mysql):
        Schema(query=types.AsyncQuery, mutation=types.AsyncMutation, scalar_overrides=scalar_overrides)
