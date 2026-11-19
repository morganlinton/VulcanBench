from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import Container

strawchemy = Strawchemy("postgresql")


@strawchemy.type(Container, include="all", paginate=[])
class ContainerType:
    pass


@strawberry.type
class Query:
    container: list[ContainerType] = strawchemy.field()
