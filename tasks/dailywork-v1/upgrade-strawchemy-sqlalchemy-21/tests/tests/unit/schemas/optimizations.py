"""DB-free strawchemy schema over unit ``Color``/``Fruit``/``Group`` for optimization tests.

Built with the ``postgresql`` dialect, but executed under each runtime dialect: the
transpiler re-reads the runtime dialect name at execution time, so the same static
schema emits dialect-specific SQL. The aggregation functions exercised
(count/avg/sum/max) exist in every supported dialect's feature set.
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Any

import strawberry
from sqlalchemy import select
from sqlalchemy.orm import aliased
from typing_extensions import override

from strawchemy import QueryHook, Strawchemy
from tests.unit.models import Color, Fruit, Group

if TYPE_CHECKING:
    from sqlalchemy.orm.util import AliasedClass

    from strawchemy.typing import SelectOf

strawchemy = Strawchemy("postgresql")


class SweetFruitHook(QueryHook[Fruit]):
    @override
    def apply_hook(self, statement: SelectOf[Fruit], alias: AliasedClass[Fruit]) -> SelectOf[Fruit]:
        return statement.where(alias.sweetness > 5)


class FirstFruitsHook(QueryHook[Fruit]):
    @override
    def apply_hook(self, statement: SelectOf[Fruit], alias: AliasedClass[Fruit]) -> SelectOf[Fruit]:
        return statement.order_by(alias.name.asc()).limit(3)


class VisibleColorHook(QueryHook[Color]):
    @override
    def apply_hook(self, statement: SelectOf[Color], alias: AliasedClass[Color]) -> SelectOf[Color]:
        return statement.where(alias.name != "hidden")


class NameOrderedColorHook(QueryHook[Color]):
    @override
    def apply_hook(self, statement: SelectOf[Color], alias: AliasedClass[Color]) -> SelectOf[Color]:
        return statement.order_by(alias.name.asc())


class FirstColorHook(QueryHook[Color]):
    @override
    def apply_hook(self, statement: SelectOf[Color], alias: AliasedClass[Color]) -> SelectOf[Color]:
        return statement.order_by(alias.name.asc()).limit(1)


class ColoredFruitHook(QueryHook[Fruit]):
    @override
    def apply_hook(self, statement: SelectOf[Fruit], alias: AliasedClass[Fruit]) -> SelectOf[Fruit]:
        color = aliased(Color, name="hook_color")
        return statement.join(color, color.id == alias.color_id).where(color.name != "hidden")


@strawchemy.type(Fruit, include="all", override=True)
class FruitType: ...


@strawchemy.type(Color, include="all", order="all", override=True)
class ColorType: ...


@strawchemy.type(Color, include="all", paginate=["fruits"])
class ColorPaginatedFruitsType: ...


@strawchemy.type(Color, include="all", order="all", paginate=["fruits"])
class ColorOrderedPaginatedFruitsType: ...


@strawchemy.aggregate(Color, include="all")
class ColorAggregationType: ...


@strawchemy.filter(Color, include="all")
class ColorFilter: ...


def _named(statement: SelectOf[Color], value: str, **_ctx: Any) -> SelectOf[Color]:
    return statement.where(Color.name == value)


@strawchemy.filter(Color, include=["id", "name"], name="ColorCustomFilter")
class ColorCustomFilter:
    named_in: str = strawchemy.filter_field(apply=_named, join="in")


@strawchemy.order(Color, include="all")
class ColorOrder: ...


@strawchemy.type(Color, include="all")
class ColorSweetFruitsType:
    fruits: list[FruitType] = strawchemy.field(query_hook=SweetFruitHook())


@strawchemy.order(Fruit, include="all")
class FruitOrder: ...


@strawchemy.type(Color, include="all")
class ColorOrderedSweetFruitsType:
    fruits: list[FruitType] = strawchemy.field(query_hook=SweetFruitHook(), order_by_input=FruitOrder)


@strawchemy.type(Color, include="all")
class ColorOrderedFirstFruitsType:
    fruits: list[FruitType] = strawchemy.field(query_hook=FirstFruitsHook(), order_by_input=FruitOrder)


@strawchemy.type(Fruit, include="all", query_hook=ColoredFruitHook(load=[Fruit.sweetness]))
class ColoredFruitType: ...


@strawchemy.type(Color, include="all", query_hook=NameOrderedColorHook())
class NameOrderedColorType: ...


@strawchemy.type(Color, include="all", query_hook=FirstColorHook())
class FirstColorType: ...


@strawchemy.type(Group, include="all", override=True)
class GroupType: ...


@strawchemy.type(Group, include="all")
class GroupVisibleColorType:
    color: ColorType = strawchemy.field(query_hook=VisibleColorHook())


@strawchemy.type(Group, include="all")
class GroupNameOrderedColorType:
    color: ColorType = strawchemy.field(query_hook=NameOrderedColorHook())


@strawchemy.type(Group, include="all", order="all")
class GroupOrderedUsersType: ...


@strawchemy.filter(Group, include="all")
class GroupFilter: ...


@strawchemy.order(Group, include="all")
class GroupOrder: ...


@strawberry.type
class Query:
    colors: list[ColorType] = strawchemy.field(filter_input=ColorFilter, order_by_input=ColorOrder)
    colors_paginated: list[ColorType] = strawchemy.field(
        filter_input=ColorFilter, order_by_input=ColorOrder, pagination=True
    )
    color_aggregations_paginated: ColorAggregationType = strawchemy.field(root_aggregations=True, pagination=True)
    colors_custom_filter: list[ColorType] = strawchemy.field(filter_input=ColorCustomFilter)
    colors_paginated_fruits: list[ColorPaginatedFruitsType] = strawchemy.field()
    colors_ordered_paginated_fruits: list[ColorOrderedPaginatedFruitsType] = strawchemy.field()
    colors_by_name_desc: list[ColorType] = strawchemy.field(
        order_by_input=ColorOrder, default_order_by=[Color.name.desc()]
    )
    colors_distinct: list[ColorType] = strawchemy.field(order_by_input=ColorOrder, distinct_on="all")
    groups: list[GroupType] = strawchemy.field(filter_input=GroupFilter)
    groups_paginated: list[GroupType] = strawchemy.field(order_by_input=GroupOrder, pagination=True)
    groups_ordered_users: list[GroupOrderedUsersType] = strawchemy.field()
    colors_sweet_fruits: list[ColorSweetFruitsType] = strawchemy.field()
    colors_ordered_sweet_fruits: list[ColorOrderedSweetFruitsType] = strawchemy.field()
    colors_ordered_first_fruits: list[ColorOrderedFirstFruitsType] = strawchemy.field()
    colored_fruits: list[ColoredFruitType] = strawchemy.field()
    colored_fruits_paginated: list[ColoredFruitType] = strawchemy.field(pagination=True)
    groups_visible_color: list[GroupVisibleColorType] = strawchemy.field(filter_input=GroupFilter)
    groups_visible_color_paginated: list[GroupVisibleColorType] = strawchemy.field(
        filter_input=GroupFilter, pagination=True
    )
    groups_name_ordered_color: list[GroupNameOrderedColorType] = strawchemy.field(order_by_input=GroupOrder)
    name_ordered_colors: list[NameOrderedColorType] = strawchemy.field()
    first_colors: list[FirstColorType] = strawchemy.field()
    colors_named_red: list[ColorType] = strawchemy.field(
        filter_statement=lambda _: select(Color).where(Color.name == "red")
    )
    colors_with_sweet_fruits: list[ColorType] = strawchemy.field(
        filter_statement=lambda _: select(Color).join(Color.fruits).where(Fruit.sweetness > 5)
    )


schema = strawberry.Schema(query=Query)
