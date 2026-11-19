from __future__ import annotations

import textwrap
from typing import TYPE_CHECKING

import pytest
from examples.testapp.testapp.app import create_app

if TYPE_CHECKING:
    from syrupy.assertion import SnapshotAssertion


def test_asgi() -> None:
    create_app()


@pytest.mark.snapshot
def test_graphql_schema(graphql_snapshot: SnapshotAssertion) -> None:
    from examples.testapp.testapp.schema import schema

    assert textwrap.dedent(str(schema)).strip() == graphql_snapshot
