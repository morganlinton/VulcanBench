from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import Color, Fruit

strawchemy = Strawchemy("postgresql")


@strawchemy.type(Color, include={"id", "name"})
class ShadeType: ...


@strawchemy.type(Fruit, include={"id", "name"})
class FruitType: ...


@strawchemy.type(Fruit, include={"id", "name"})
class ShadedFruitType:
    name: str = strawchemy.field(description="Fruit name.", deprecation_reason="Use id.")
    shade: ShadeType = strawchemy.field(model_field="color", description="To-one relation.")


@strawchemy.type(Color, include={"id"})
class ColorType:
    fruits: list[ShadedFruitType] = strawchemy.field(description="Plain relation field.")


@strawchemy.type(Color, include={"id"})
class RenamedColorType:
    items: list[FruitType] = strawchemy.field(
        model_field="fruits", pagination=True, order_by_input="all", description="Renamed relation field."
    )


@strawchemy.type(Color, include={"id"}, paginate="all")
class PaginatedColorType:
    fruits: list[FruitType] = strawchemy.field(description="Inherits type-level pagination.")


@strawchemy.type(Color, include={"id"}, paginate="all")
class UnpaginatedColorType:
    fruits: list[FruitType] = strawchemy.field(pagination=False)


@strawberry.type
class Query:
    colors: list[ColorType] = strawchemy.field()
    renamed_colors: list[RenamedColorType] = strawchemy.field()
    paginated_colors: list[PaginatedColorType] = strawchemy.field()
    unpaginated_colors: list[UnpaginatedColorType] = strawchemy.field()
