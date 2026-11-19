from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import Color

strawchemy = Strawchemy("postgresql")


@strawchemy.create_input(Color, include=["name"])
class ColorCreate: ...


@strawberry.type
class Plain:
    name: str


@strawberry.type
class Mutation:
    plain: Plain = strawchemy.create(ColorCreate)
