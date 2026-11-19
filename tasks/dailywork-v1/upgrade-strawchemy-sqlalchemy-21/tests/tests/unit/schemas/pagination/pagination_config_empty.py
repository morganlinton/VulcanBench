from __future__ import annotations

import strawberry

from strawchemy import Strawchemy, StrawchemyConfig
from tests.unit.models import Container

strawchemy = Strawchemy(StrawchemyConfig("postgresql", pagination=[]))


@strawchemy.type(Container, include="all")
class ContainerType:
    pass


@strawberry.type
class Query:
    container: list[ContainerType] = strawchemy.field()
