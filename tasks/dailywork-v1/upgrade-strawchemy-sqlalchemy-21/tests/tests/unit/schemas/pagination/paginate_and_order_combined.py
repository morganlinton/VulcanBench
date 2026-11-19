from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import Container

strawchemy = Strawchemy("postgresql")


# Different fields for paginate vs order
@strawchemy.type(Container, include="all", paginate=["fruits"], order=["vegetables"])
class ContainerType1:
    pass


# Overlapping fields
@strawchemy.type(Container, include="all", paginate=["fruits", "vegetables"], order=["fruits"])
class ContainerType2:
    pass


# All + specific
@strawchemy.type(Container, include="all", paginate="all", order=["fruits"])
class ContainerType3:
    pass


@strawberry.type
class Query:
    container1: list[ContainerType1] = strawchemy.field()
    container2: list[ContainerType2] = strawchemy.field()
    container3: list[ContainerType3] = strawchemy.field()
