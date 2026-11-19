from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import Fruit

strawchemy = Strawchemy("postgresql")


@strawchemy.type(Fruit, include="all")
class FruitType:
    pass


@strawchemy.filter(Fruit, include="all")
class FruitFilter:
    pass


@strawberry.type
class Query:
    fruit: FruitType = strawchemy.field(filter_input=FruitFilter)
