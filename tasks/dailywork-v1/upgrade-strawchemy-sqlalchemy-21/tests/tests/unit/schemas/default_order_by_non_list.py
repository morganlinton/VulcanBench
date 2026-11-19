from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import Fruit

strawchemy = Strawchemy("postgresql")


@strawchemy.type(Fruit, include=["name"])
class FruitType:
    id: strawberry.auto


@strawberry.type
class NonListQuery:
    fruit: FruitType = strawchemy.field(default_order_by=Fruit.name.asc())
