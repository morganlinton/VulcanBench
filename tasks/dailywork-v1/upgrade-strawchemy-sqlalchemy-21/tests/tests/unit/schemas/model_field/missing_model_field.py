from __future__ import annotations

from strawchemy import Strawchemy
from tests.unit.models import Fruit

strawchemy = Strawchemy("postgresql")


@strawchemy.type(Fruit)
class FruitType:
    bad: str = strawchemy.field(model_field="nope")
