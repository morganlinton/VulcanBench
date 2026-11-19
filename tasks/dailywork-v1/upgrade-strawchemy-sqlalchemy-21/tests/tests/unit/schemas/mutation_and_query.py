from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import Color, Fruit

strawchemy = Strawchemy("postgresql")


@strawchemy.type(Fruit, include="all")
class FruitType: ...


@strawchemy.type(Color, include="all", override=True)
class ColorType: ...


@strawchemy.create_input(Fruit, include="all")
class FruitInput: ...


@strawchemy.create_input(Color, include="all", override=True)
class ColorInput: ...


@strawberry.type
class Query:
    fruit: FruitType = strawchemy.field()
    fruits: list[FruitType] = strawchemy.field()

    color: ColorType = strawchemy.field()
    colors: list[ColorType] = strawchemy.field()


@strawberry.type
class Mutation:
    create_fruit: FruitType = strawchemy.create(FruitInput)
    create_fruits: list[FruitType] = strawchemy.create(FruitInput)

    create_color: ColorType = strawchemy.create(ColorInput)
    create_colors: list[ColorType] = strawchemy.create(ColorInput)
