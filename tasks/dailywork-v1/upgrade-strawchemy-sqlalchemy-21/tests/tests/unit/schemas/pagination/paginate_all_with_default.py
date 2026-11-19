from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from strawchemy.schema.pagination import DefaultOffsetPagination
from tests.unit.models import Fruit

strawchemy = Strawchemy("postgresql")


@strawchemy.type(Fruit, include="all", paginate="all", default_pagination=DefaultOffsetPagination(limit=20, offset=5))
class FruitType:
    pass


@strawberry.type
class Query:
    fruit_with_custom_default: list[FruitType] = strawchemy.field()
