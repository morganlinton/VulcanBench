from __future__ import annotations

from uuid import uuid4

import strawberry
from strawberry import auto

from strawchemy import Strawchemy
from tests.unit.models import Container, SQLDataTypes

strawchemy = Strawchemy("postgresql")


@strawchemy.type(SQLDataTypes, include=["id", "dict_col"])
class OverriddenJSONType:
    """Class-body resolver must win over the auto-derived JSON path projection."""

    @strawberry.field(description="Class-body override for dict_col.")
    def dict_col(self) -> str:
        return "OVERRIDE"


@strawchemy.type(SQLDataTypes, include=["id", "dict_col"])
class AutoJSONType:
    """No override: the JSON column keeps its auto-derived `(path: String): JSON` projection."""

    id: auto
    dict_col: auto


@strawchemy.type(Container, include=["name", "fruits"])
class ContainerType:
    """Class-body resolver must win over the auto-derived relation arguments."""

    @strawberry.field(description="Class-body override for fruits.")
    def fruits(self) -> list[str]:
        return []


@strawberry.type
class Query:
    overridden_json: OverriddenJSONType = strawchemy.field()
    auto_json: AutoJSONType = strawchemy.field()
    container: ContainerType = strawchemy.field()


@strawberry.type
class ExecutableQuery:
    """Resolves an instance directly (no database) so the overridden resolver can be executed."""

    @strawberry.field
    def overridden_json(self) -> OverriddenJSONType:
        return OverriddenJSONType(id=uuid4())  # ty: ignore[unknown-argument]
