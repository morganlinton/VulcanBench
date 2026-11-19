from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import Color

strawchemy = Strawchemy("postgresql")


@strawchemy.type(Color, include="all")
class ColorType: ...


@strawchemy.distinct_on(Color, include="all")
class ColorDistinctOnFields: ...


@strawberry.type
class Query:
    color: list[ColorType] = strawchemy.field(distinct_on=ColorDistinctOnFields)
