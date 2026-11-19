from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import Color

strawchemy = Strawchemy("postgresql")


@strawberry.type
class Mutation:
    delete_colors: list[ColorType] = strawchemy.delete()


@strawchemy.type(Color, include=["name"])
class ColorType: ...


@strawberry.type
class Query:
    colors: list[ColorType] = strawchemy.field()
