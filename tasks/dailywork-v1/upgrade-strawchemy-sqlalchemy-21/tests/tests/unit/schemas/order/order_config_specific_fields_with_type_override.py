from __future__ import annotations

import strawberry

from strawchemy import Strawchemy, StrawchemyConfig
from tests.unit.models import Container

strawchemy = Strawchemy(StrawchemyConfig("postgresql", order_by=["fruits"]))


@strawchemy.type(Container, include="all", order=["fruits", "vegetables"])
class ContainerType:
    pass


@strawberry.type
class Query:
    container: list[ContainerType] = strawchemy.field()
