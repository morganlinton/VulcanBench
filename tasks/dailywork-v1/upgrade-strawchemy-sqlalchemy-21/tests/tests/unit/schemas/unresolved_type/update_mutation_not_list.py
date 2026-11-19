from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import Color

strawchemy = Strawchemy("postgresql")


@strawchemy.filter(Color, include="all")
class ColorFilter: ...


@strawchemy.filter_update_input(Color, include=["name"])
class ColorUpdate: ...


@strawberry.type
class Mutation:
    update_color: ColorType = strawchemy.update(ColorUpdate, ColorFilter)


@strawchemy.type(Color, include=["name"])
class ColorType: ...


@strawberry.type
class Query:
    colors: list[ColorType] = strawchemy.field()
