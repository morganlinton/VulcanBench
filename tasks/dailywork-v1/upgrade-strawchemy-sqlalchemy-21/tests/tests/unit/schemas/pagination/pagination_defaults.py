from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from strawchemy.schema.pagination import DefaultOffsetPagination
from tests.unit.models import Fruit

strawchemy = Strawchemy("postgresql")


@strawchemy.type(Fruit, include="all")
class FruitType:
    pass


@strawberry.type
class Query:
    fruits_with_default_limit: list[FruitType] = strawchemy.field(pagination=DefaultOffsetPagination(limit=10))
    fruits_with_default_offset: list[FruitType] = strawchemy.field(pagination=DefaultOffsetPagination(offset=10))
