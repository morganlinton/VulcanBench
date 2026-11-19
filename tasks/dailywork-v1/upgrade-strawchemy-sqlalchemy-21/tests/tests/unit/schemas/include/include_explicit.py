from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import Fruit

strawchemy = Strawchemy("postgresql")


@strawchemy.type(Fruit, include=["name", "sweetness"])
class FruitType:
    pass


@strawchemy.type(Fruit, include=["color"])
class FruitWithColorType:
    pass


@strawberry.type
class Query:
    fruit: FruitType
    fruit_with_color: FruitWithColorType
