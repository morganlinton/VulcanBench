from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import Fruit

strawchemy = Strawchemy("postgresql")


@strawchemy.type(Fruit, exclude=["color", "color_id"])
class FruitType:
    pass


@strawberry.type
class Query:
    fruit: FruitType
