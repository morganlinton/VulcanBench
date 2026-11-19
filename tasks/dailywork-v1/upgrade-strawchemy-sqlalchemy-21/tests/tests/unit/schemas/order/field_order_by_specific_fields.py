from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import Container

strawchemy = Strawchemy("postgresql")


@strawchemy.type(Container, include="all")
class ContainerType:
    pass


@strawberry.type
class Query:
    containers: list[ContainerType] = strawchemy.field(order_by_input=["fruits", "vegetables"])
