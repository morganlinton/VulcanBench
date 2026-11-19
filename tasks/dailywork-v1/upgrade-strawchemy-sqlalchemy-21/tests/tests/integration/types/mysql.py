from __future__ import annotations

from collections.abc import Awaitable, Callable
from typing import TYPE_CHECKING, Annotated, Any, TypeAlias, cast

import strawberry
from pydantic import AfterValidator
from sqlalchemy import select
from sqlalchemy.orm import aliased
from strawberry.extensions.field_extension import FieldExtension
from typing_extensions import override

from strawchemy import (
    Input,
    InputValidationError,
    ModelInstance,
    QueryHook,
    Strawchemy,
    StrawchemyAsyncRepository,
    StrawchemyConfig,
    StrawchemySyncRepository,
    TextComparison,
    ValidationErrorType,
)
from strawchemy.schema.pagination import DefaultOffsetPagination
from strawchemy.validation.pydantic import PydanticValidation
from tests.integration.models import Color, DateTimeModel, Fruit, FruitFarm, IntervalModel, JSONModel, RankedUser, User

if TYPE_CHECKING:
    from collections.abc import Sequence

    from sqlalchemy.ext.asyncio import AsyncSession
    from sqlalchemy.orm import Session
    from sqlalchemy.orm.util import AliasedClass

    from strawchemy.transpiler.hook import LoadType
    from strawchemy.typing import SelectOf

SyncExtensionResolver: TypeAlias = Callable[..., Any]
AsyncExtensionResolver: TypeAlias = Callable[..., Awaitable[Any]]


strawchemy = Strawchemy(StrawchemyConfig("mysql"))


def _check_lower_case(value: str) -> str:
    if not value.islower():
        msg = "Name must be lower cased"
        raise ValueError(msg)
    return value


class SomeExtension(FieldExtension):
    @override
    async def resolve_async(
        self, next_: AsyncExtensionResolver, source: Any, info: strawberry.Info, **kwargs: Any
    ) -> Any:
        fruit = await next_(source, info, **kwargs)
        assert fruit.instance.name == "Apple"
        return fruit

    @override
    def resolve(self, next_: SyncExtensionResolver, source: Any, info: strawberry.Info, **kwargs: Any) -> Any:
        fruit = next_(source, info, **kwargs)
        assert fruit.instance.name == "Apple"
        return fruit


# Hooks


class FruitFilterHook(QueryHook[Fruit]):
    @override
    def apply_hook(self, statement: SelectOf[Fruit], alias: AliasedClass[Fruit]) -> SelectOf[Fruit]:
        if self.info.context.role == "user":
            return statement.where(alias.name == "Apple")
        return statement


class FruitOrderingHook(QueryHook[Fruit]):
    load: Sequence[LoadType] = [Fruit.water_percent]

    @override
    def apply_hook(self, statement: SelectOf[Fruit], alias: AliasedClass[Fruit]) -> SelectOf[Fruit]:
        return statement.order_by(alias.water_percent.asc())


class SweetFruitHook(QueryHook[Fruit]):
    @override
    def apply_hook(self, statement: SelectOf[Fruit], alias: AliasedClass[Fruit]) -> SelectOf[Fruit]:
        return statement.where(alias.sweetness > 5)


class MultiFarmFruitHook(QueryHook[Fruit]):
    @override
    def apply_hook(self, statement: SelectOf[Fruit], alias: AliasedClass[Fruit]) -> SelectOf[Fruit]:
        return statement.join(FruitFarm, FruitFarm.fruit_id == alias.id).where(FruitFarm.name.endswith(" 2"))


class FruitColorNameOrderingHook(QueryHook[Fruit]):
    @override
    def apply_hook(self, statement: SelectOf[Fruit], alias: AliasedClass[Fruit]) -> SelectOf[Fruit]:
        color = aliased(Color)
        return statement.outerjoin(color, color.id == alias.color_id).order_by(color.name.desc())


class ColorOrderingHook(QueryHook[Color]):
    @override
    def apply_hook(self, statement: SelectOf[Color], alias: AliasedClass[Color]) -> SelectOf[Color]:
        return statement.order_by(alias.name.asc())


# User


@strawchemy.type(User, include="all")
class UserType: ...


@strawchemy.order(User, include="all", override=True)
class UserOrderBy: ...


@strawchemy.filter(User, include="all")
class UserFilter: ...


@strawchemy.create_input(User, include="all")
class UserCreate: ...


@strawchemy.pk_update_input(User, include="all")
class UserUpdateInput: ...


@strawchemy.distinct_on(User, include="all")
class UserDistinctOn: ...


# Fruit


@strawchemy.type(Fruit, include="all", override=True)
class FruitType: ...


@strawchemy.type(Fruit, exclude={"color"})
class FruitTypeHooks:
    instance: ModelInstance[Fruit]

    @strawchemy.field(query_hook=QueryHook(load=[Fruit.name, Fruit.color_id]))
    def description(self) -> str:
        return self.instance.description

    @strawchemy.field(query_hook=QueryHook())
    def empty_query_hook(self) -> str:
        return "success"

    @strawchemy.field(query_hook=QueryHook(load=[(Fruit.color, [Color.name, Color.created_at])]))
    def pretty_color(self) -> str:
        return f"Color is {self.instance.color.name}" if self.instance.color else "No color!"

    @strawchemy.field(query_hook=QueryHook(load=[Fruit.farms]))
    def pretty_farms(self) -> str:
        return (
            f"Farms are: {', '.join(farm.name for farm in self.instance.farms)}" if self.instance.farms else "No farm!"
        )


@strawchemy.type(Fruit, exclude={"color"}, query_hook=FruitFilterHook())
class FilteredFruitType: ...


@strawchemy.type(Fruit, exclude={"color"}, query_hook=FruitOrderingHook())
class OrderedFruitType: ...


@strawchemy.type(Fruit, exclude={"color"}, query_hook=FruitColorNameOrderingHook())
class ColorNameOrderedFruitType: ...


@strawchemy.aggregate(Fruit, include="all")
class FruitAggregationType: ...


@strawchemy.type(Fruit, include="all", paginate="all", order="all")
class FruitTypeWithPaginationAndOrderBy: ...


@strawchemy.filter(Fruit, include="all")
class FruitFilter: ...


def _fruit_sweeter_than(statement: SelectOf[Fruit], value: int, **_ctx: Any) -> SelectOf[Fruit]:
    return statement.where(Fruit.sweetness >= value)


@strawchemy.filter(Fruit, include=["id", "name", "sweetness"], name="FruitFineGrainedFilter")
class FruitFineGrainedFilter:
    name: TextComparison = strawchemy.filter_field(ops=["eq", "like"])
    sweeter_than: int = strawchemy.filter_field(apply=_fruit_sweeter_than)
    sweeter_than_in: int = strawchemy.filter_field(apply=_fruit_sweeter_than, join="in")


# `sweetness` is included so `sum`'s `arguments=["sweetness"]` stays within the decorator's scope.
@strawchemy.aggregate_filter(
    Fruit, include=["id", "sweetness"], functions=["count", "sum"], name="FruitFineGrainedAggregateFilter"
)
class FruitFineGrainedAggregateFilter:
    count: int = strawchemy.filter_field(ops=["gt"])
    sum: float = strawchemy.filter_field(arguments=["sweetness"], ops=["gte"])


@strawchemy.order(Fruit, include="all", scope="schema")
class FruitOrderBy: ...


@strawchemy.create_input(Fruit, include="all")
class FruitCreateInput: ...


@strawchemy.pk_update_input(Fruit, include="all")
class FruitUpdateInput: ...


@strawchemy.upsert_update_fields(Fruit, include="all")
class FruitUpsertFields: ...


@strawchemy.upsert_conflict_fields(Fruit, include="all")
class FruitUpsertConflictFields: ...


# Color


@strawchemy.type(Color, include="all", override=True, order="all")
class ColorType: ...


@strawchemy.order(Color, include="all")
class ColorOrder: ...


@strawchemy.distinct_on(Color, include="all", override=True)
class ColorDistinctOn: ...


@strawchemy.distinct_on(Fruit, include="all")
class FruitDistinctOn: ...


@strawchemy.type(Color, include="all", paginate="all", order="all")
class ColorTypeWithPagination: ...


@strawchemy.type(Color, include="all", distinct_on="all", order="all", paginate="all")
class ColorTypeWithDistinctOn: ...


@strawchemy.type(Color, include="all")
class ColorWithFilteredFruit:
    instance: ModelInstance[Color]

    fruits: list[FilteredFruitType]

    @strawchemy.field(query_hook=QueryHook(load=[(Color.fruits, [(Fruit.farms, [FruitFarm.name])])]))
    def farms(self) -> str:
        return f"Farms are: {', '.join(farm.name for fruit in self.instance.fruits for farm in fruit.farms)}"

    @strawchemy.field(query_hook=QueryHook(load=[(Color.fruits, [Fruit.farms])]))
    def farms_no_columns(self) -> str:
        return f"Farms are: {', '.join(farm.name for fruit in self.instance.fruits for farm in fruit.farms)}"


@strawchemy.type(Color, include="all")
class ColorTypeHooks:
    instance: ModelInstance[Color]

    fruits: list[FruitTypeHooks]


@strawchemy.type(Color, include="all")
class ColorWithSweetFruits:
    fruits: list[FruitType] = strawchemy.field(query_hook=SweetFruitHook())


@strawchemy.type(Color, include="all")
class ColorWithDefaultPaginatedFruits:
    fruits: list[FruitType] = strawchemy.field(pagination=DefaultOffsetPagination(limit=1, offset=1))


@strawchemy.type(Color, include="all")
class ColorWithPaginatedSweetFruits:
    fruits: list[FruitType] = strawchemy.field(query_hook=SweetFruitHook(load=[Fruit.water_percent]), pagination=True)


@strawchemy.type(Color, include="all")
class ColorWithMultiFarmFruits:
    fruits: list[FruitType] = strawchemy.field(query_hook=MultiFarmFruitHook())


@strawchemy.type(Color, include="all")
class ColorWithOrderedFruits:
    fruits: list[OrderedFruitType] = strawchemy.field(order_by_input=FruitOrderBy, distinct_on=FruitDistinctOn)


@strawchemy.type(Color, include="all")
class ColorWithColorNameOrderedFruits:
    fruits: list[ColorNameOrderedFruitType] = strawchemy.field(order_by_input=FruitOrderBy, distinct_on=FruitDistinctOn)


@strawchemy.type(Color, include="all")
class ColorWithPaginatedOrderedFruits:
    fruits: list[OrderedFruitType] = strawchemy.field(pagination=True)


@strawchemy.type(Color, include="all", query_hook=ColorOrderingHook())
class OrderedColorType: ...


@strawchemy.type(Fruit, include="all", paginate="all", order="all")
class FruitWithOrderedColorType:
    color: OrderedColorType | None


@strawchemy.create_input(Color, include="all")
class ColorCreateInput: ...


@strawchemy.pydantic.create(Color, include="all")
class ColorCreateValidation:
    name: Annotated[str, AfterValidator(_check_lower_case)]


@strawchemy.pydantic.pk_update(Color, include="all")
class ColorPkUpdateValidation: ...


@strawchemy.pk_update_input(Color, include="all")
class ColorUpdateInput: ...


@strawchemy.filter_update_input(Color, include="all")
class ColorPartial: ...


@strawchemy.filter_update_input(Fruit, include=["name", "sweetness"])
class FruitPartial: ...


@strawchemy.filter_update_input(User, include=["name"])
class UserPartial: ...


@strawchemy.filter(Color, include="all")
class ColorFilter: ...


@strawchemy.filter(Color, include=["name", "fruits"], name="ColorFineGrainedFilter")
class ColorFineGrainedFilter:
    fruits_aggregate: FruitFineGrainedAggregateFilter  # ty: ignore[invalid-type-form]


# Ranked User


@strawchemy.create_input(RankedUser, include="all")
class RankedUserCreateInput: ...


@strawchemy.type(RankedUser, include="all")
class RankedUserType: ...


@strawchemy.pydantic.create(RankedUser, include="all")
class RankedUserCreateValidation:
    name: Annotated[str, AfterValidator(_check_lower_case)]


# Interval type


@strawchemy.filter(IntervalModel, include="all")
class IntervalFilter: ...


@strawchemy.type(IntervalModel, include="all")
class IntervalType: ...


# JSON type


@strawchemy.filter(JSONModel, include="all")
class JSONFilter: ...


@strawchemy.type(JSONModel, include="all")
class JSONType: ...


# Date/Time


@strawchemy.filter(DateTimeModel, include="all")
class DateTimeFilter: ...


@strawchemy.type(DateTimeModel, include="all")
class DateTimeType: ...


# Queries


@strawberry.type
class AsyncQuery:
    # Fruit
    fruits: list[FruitType] = strawchemy.field(
        filter_input=FruitFilter, order_by_input=FruitOrderBy, repository_type=StrawchemyAsyncRepository
    )
    fruits_fine_grained: list[FruitType] = strawchemy.field(
        filter_input=FruitFineGrainedFilter, repository_type=StrawchemyAsyncRepository
    )
    fruits_paginated: list[FruitTypeWithPaginationAndOrderBy] = strawchemy.field(
        filter_input=FruitFilter,
        pagination=True,
        repository_type=StrawchemyAsyncRepository,
    )
    fruits_paginated_default_limit_1: list[FruitType] = strawchemy.field(
        filter_input=FruitFilter,
        pagination=DefaultOffsetPagination(limit=1),
        repository_type=StrawchemyAsyncRepository,
    )
    fruit_aggregations: FruitAggregationType = strawchemy.field(
        root_aggregations=True, repository_type=StrawchemyAsyncRepository
    )
    fruit_aggregations_paginated: FruitAggregationType = strawchemy.field(
        root_aggregations=True, pagination=True, repository_type=StrawchemyAsyncRepository
    )
    fruit_aggregations_paginated_limit_2: FruitAggregationType = strawchemy.field(
        root_aggregations=True, pagination=DefaultOffsetPagination(limit=2), repository_type=StrawchemyAsyncRepository
    )
    fruits_default_order: list[FruitType] = strawchemy.field(
        default_order_by=Fruit.name.asc(),
        order_by_input=FruitOrderBy,
        pagination=True,
        repository_type=StrawchemyAsyncRepository,
    )
    fruits_default_order_no_pagination: list[FruitType] = strawchemy.field(
        default_order_by=Fruit.name.asc(),
        order_by_input=FruitOrderBy,
        pagination=False,
        repository_type=StrawchemyAsyncRepository,
    )
    fruits_default_order_multi: list[FruitType] = strawchemy.field(
        default_order_by=[Fruit.sweetness.desc(), Fruit.name.asc()],
        order_by_input=FruitOrderBy,
        pagination=False,
        repository_type=StrawchemyAsyncRepository,
    )
    fruits_default_order_distinct: list[FruitType] = strawchemy.field(
        default_order_by=Fruit.name.asc(),
        distinct_on=FruitDistinctOn,
        order_by_input=FruitOrderBy,
        pagination=False,
        repository_type=StrawchemyAsyncRepository,
    )
    fruits_default_order_sweetness: list[FruitTypeWithPaginationAndOrderBy] = strawchemy.field(
        default_order_by=Fruit.sweetness.desc(), repository_type=StrawchemyAsyncRepository
    )
    colors_default_order: list[ColorTypeWithPagination] = strawchemy.field(
        default_order_by=Color.name.desc(), repository_type=StrawchemyAsyncRepository
    )
    fruits_hooks: list[FruitTypeHooks] = strawchemy.field(repository_type=StrawchemyAsyncRepository)
    fruits_paginated_hooks: list[FruitTypeHooks] = strawchemy.field(
        repository_type=StrawchemyAsyncRepository, pagination=True
    )
    filtered_fruits: list[FilteredFruitType] = strawchemy.field(repository_type=StrawchemyAsyncRepository)
    filtered_fruits_paginated: list[FilteredFruitType] = strawchemy.field(
        repository_type=StrawchemyAsyncRepository, pagination=True
    )
    ordered_fruits: list[OrderedFruitType] = strawchemy.field(
        order_by_input=FruitOrderBy, distinct_on=FruitDistinctOn, repository_type=StrawchemyAsyncRepository
    )
    ordered_fruits_paginated: list[OrderedFruitType] = strawchemy.field(
        order_by_input=FruitOrderBy,
        distinct_on=FruitDistinctOn,
        repository_type=StrawchemyAsyncRepository,
        pagination=True,
    )
    field_filtered_fruits: list[FruitType] = strawchemy.field(
        query_hook=FruitFilterHook(), repository_type=StrawchemyAsyncRepository
    )
    field_filtered_fruits_paginated: list[FruitType] = strawchemy.field(
        query_hook=FruitFilterHook(), repository_type=StrawchemyAsyncRepository, pagination=True
    )
    field_filtered_fruit: FruitType | None = strawchemy.field(
        query_hook=FruitFilterHook(), repository_type=StrawchemyAsyncRepository
    )
    field_filtered_fruit_aggregations: FruitAggregationType = strawchemy.field(
        query_hook=FruitFilterHook(), root_aggregations=True, repository_type=StrawchemyAsyncRepository
    )
    field_load_fruits: list[FruitType] = strawchemy.field(
        query_hook=QueryHook(load=[Fruit.water_percent]), repository_type=StrawchemyAsyncRepository
    )
    fruit_with_extension: FruitTypeHooks = strawchemy.field(
        filter_input=FruitFilter, repository_type=StrawchemyAsyncRepository, extensions=[SomeExtension()]
    )
    # Color
    color: ColorType = strawchemy.field(repository_type=StrawchemyAsyncRepository)
    colors: list[ColorType] = strawchemy.field(
        filter_input=ColorFilter,
        distinct_on=ColorDistinctOn,
        order_by_input=ColorOrder,
        repository_type=StrawchemyAsyncRepository,
    )
    colors_paginated: list[ColorTypeWithPagination] = strawchemy.field(
        pagination=True, repository_type=StrawchemyAsyncRepository
    )
    colors_nested_distinct: list[ColorTypeWithDistinctOn] = strawchemy.field(repository_type=StrawchemyAsyncRepository)
    colors_filterable_paginated: list[ColorType] = strawchemy.field(
        filter_input=ColorFilter,
        order_by_input=ColorOrder,
        pagination=True,
        repository_type=StrawchemyAsyncRepository,
    )

    colors_filtered: list[ColorType] = strawchemy.field(
        repository_type=StrawchemyAsyncRepository, filter_statement=lambda _: select(Color).where(Color.name == "Red")
    )
    colors_filtered_paginated: list[ColorType] = strawchemy.field(
        order_by_input=ColorOrder,
        pagination=True,
        repository_type=StrawchemyAsyncRepository,
        filter_statement=lambda _: select(Color).where(Color.name.in_(["Red", "Green", "Pink"])),
    )
    colors_filtered_distinct: list[ColorType] = strawchemy.field(
        distinct_on=ColorDistinctOn,
        order_by_input=ColorOrder,
        pagination=True,
        repository_type=StrawchemyAsyncRepository,
        filter_statement=lambda _: select(Color).where(Color.name.in_(["Red", "Green", "Pink"])),
    )
    colors_with_filtered_fruits: list[ColorWithFilteredFruit] = strawchemy.field(
        repository_type=StrawchemyAsyncRepository
    )
    colors_with_filtered_fruits_paginated: list[ColorWithFilteredFruit] = strawchemy.field(
        repository_type=StrawchemyAsyncRepository, pagination=True
    )
    colors_hooks: list[ColorTypeHooks] = strawchemy.field(repository_type=StrawchemyAsyncRepository)
    colors_hooks_paginated: list[ColorTypeHooks] = strawchemy.field(
        repository_type=StrawchemyAsyncRepository, pagination=True
    )
    colors_with_sweet_fruits: list[ColorWithSweetFruits] = strawchemy.field(
        filter_input=ColorFilter, repository_type=StrawchemyAsyncRepository
    )
    colors_with_sweet_fruits_paginated: list[ColorWithSweetFruits] = strawchemy.field(
        filter_input=ColorFilter, repository_type=StrawchemyAsyncRepository, pagination=True
    )
    colors_with_paginated_sweet_fruits: list[ColorWithPaginatedSweetFruits] = strawchemy.field(
        repository_type=StrawchemyAsyncRepository
    )
    colors_with_default_paginated_fruits: list[ColorWithDefaultPaginatedFruits] = strawchemy.field(
        repository_type=StrawchemyAsyncRepository
    )
    colors_with_multi_farm_fruits: list[ColorWithMultiFarmFruits] = strawchemy.field(
        filter_input=ColorFilter, repository_type=StrawchemyAsyncRepository
    )
    colors_with_ordered_fruits: list[ColorWithOrderedFruits] = strawchemy.field(
        repository_type=StrawchemyAsyncRepository
    )
    fruits_with_ordered_color: list[FruitWithOrderedColorType] = strawchemy.field(
        repository_type=StrawchemyAsyncRepository
    )
    fruits_with_ordered_color_paginated: list[FruitWithOrderedColorType] = strawchemy.field(
        pagination=True, repository_type=StrawchemyAsyncRepository
    )
    colors_with_paginated_ordered_fruits: list[ColorWithPaginatedOrderedFruits] = strawchemy.field(
        repository_type=StrawchemyAsyncRepository
    )
    colors_with_color_name_ordered_fruits: list[ColorWithColorNameOrderedFruits] = strawchemy.field(
        repository_type=StrawchemyAsyncRepository
    )
    color_name_ordered_fruits: list[ColorNameOrderedFruitType] = strawchemy.field(
        order_by_input=FruitOrderBy, distinct_on=FruitDistinctOn, repository_type=StrawchemyAsyncRepository
    )
    color_name_ordered_fruits_paginated: list[ColorNameOrderedFruitType] = strawchemy.field(
        order_by_input=FruitOrderBy,
        distinct_on=FruitDistinctOn,
        repository_type=StrawchemyAsyncRepository,
        pagination=True,
    )
    colors_fine_grained: list[ColorType] = strawchemy.field(
        filter_input=ColorFineGrainedFilter, repository_type=StrawchemyAsyncRepository
    )
    # User
    user: UserType = strawchemy.field(repository_type=StrawchemyAsyncRepository)
    users: list[UserType] = strawchemy.field(
        filter_input=UserFilter,
        order_by_input=UserOrderBy,
        repository_type=StrawchemyAsyncRepository,
        distinct_on=UserDistinctOn,
    )
    users_paginated: list[UserType] = strawchemy.field(
        filter_input=UserFilter,
        order_by_input=UserOrderBy,
        pagination=True,
        repository_type=StrawchemyAsyncRepository,
        distinct_on=UserDistinctOn,
    )

    # Custom resolvers
    @strawchemy.field
    async def red_color(self, info: strawberry.Info) -> ColorType:
        repo = StrawchemyAsyncRepository(ColorType, info, filter_statement=select(Color).where(Color.name == "Red"))
        return (await repo.get_one()).graphql_type()

    @strawchemy.field
    async def get_color(self, info: strawberry.Info, color: str) -> ColorType | None:
        repo = StrawchemyAsyncRepository(ColorType, info, filter_statement=select(Color).where(Color.name == color))
        return (await repo.get_one_or_none()).graphql_type_or_none()


@strawberry.type
class SyncQuery:
    # Fruit
    fruits: list[FruitType] = strawchemy.field(
        filter_input=FruitFilter, order_by_input=FruitOrderBy, repository_type=StrawchemySyncRepository
    )
    fruits_fine_grained: list[FruitType] = strawchemy.field(
        filter_input=FruitFineGrainedFilter, repository_type=StrawchemySyncRepository
    )
    fruits_paginated: list[FruitTypeWithPaginationAndOrderBy] = strawchemy.field(
        filter_input=FruitFilter,
        pagination=True,
        repository_type=StrawchemySyncRepository,
    )
    fruits_paginated_default_limit_1: list[FruitType] = strawchemy.field(
        filter_input=FruitFilter,
        pagination=DefaultOffsetPagination(limit=1),
        repository_type=StrawchemySyncRepository,
    )
    fruit_aggregations: FruitAggregationType = strawchemy.field(
        root_aggregations=True, repository_type=StrawchemySyncRepository
    )
    fruit_aggregations_paginated: FruitAggregationType = strawchemy.field(
        root_aggregations=True, pagination=True, repository_type=StrawchemySyncRepository
    )
    fruit_aggregations_paginated_limit_2: FruitAggregationType = strawchemy.field(
        root_aggregations=True, pagination=DefaultOffsetPagination(limit=2), repository_type=StrawchemySyncRepository
    )
    fruits_default_order: list[FruitType] = strawchemy.field(
        default_order_by=Fruit.name.asc(),
        order_by_input=FruitOrderBy,
        pagination=True,
        repository_type=StrawchemySyncRepository,
    )
    fruits_default_order_no_pagination: list[FruitType] = strawchemy.field(
        default_order_by=Fruit.name.asc(),
        order_by_input=FruitOrderBy,
        pagination=False,
        repository_type=StrawchemySyncRepository,
    )
    fruits_default_order_multi: list[FruitType] = strawchemy.field(
        default_order_by=[Fruit.sweetness.desc(), Fruit.name.asc()],
        order_by_input=FruitOrderBy,
        pagination=False,
        repository_type=StrawchemySyncRepository,
    )
    fruits_default_order_distinct: list[FruitType] = strawchemy.field(
        default_order_by=Fruit.name.asc(),
        distinct_on=FruitDistinctOn,
        order_by_input=FruitOrderBy,
        pagination=False,
        repository_type=StrawchemySyncRepository,
    )
    fruits_default_order_sweetness: list[FruitTypeWithPaginationAndOrderBy] = strawchemy.field(
        default_order_by=Fruit.sweetness.desc(), repository_type=StrawchemySyncRepository
    )
    colors_default_order: list[ColorTypeWithPagination] = strawchemy.field(
        default_order_by=Color.name.desc(), repository_type=StrawchemySyncRepository
    )
    fruits_hooks: list[FruitTypeHooks] = strawchemy.field(repository_type=StrawchemySyncRepository)
    fruits_paginated_hooks: list[FruitTypeHooks] = strawchemy.field(
        repository_type=StrawchemySyncRepository, pagination=True
    )
    filtered_fruits: list[FilteredFruitType] = strawchemy.field(repository_type=StrawchemySyncRepository)
    filtered_fruits_paginated: list[FilteredFruitType] = strawchemy.field(
        repository_type=StrawchemySyncRepository, pagination=True
    )
    ordered_fruits: list[OrderedFruitType] = strawchemy.field(
        order_by_input=FruitOrderBy, distinct_on=FruitDistinctOn, repository_type=StrawchemySyncRepository
    )
    ordered_fruits_paginated: list[OrderedFruitType] = strawchemy.field(
        order_by_input=FruitOrderBy,
        distinct_on=FruitDistinctOn,
        repository_type=StrawchemySyncRepository,
        pagination=True,
    )
    field_filtered_fruits: list[FruitType] = strawchemy.field(
        query_hook=FruitFilterHook(), repository_type=StrawchemySyncRepository
    )
    field_filtered_fruits_paginated: list[FruitType] = strawchemy.field(
        query_hook=FruitFilterHook(), repository_type=StrawchemySyncRepository, pagination=True
    )
    field_filtered_fruit: FruitType | None = strawchemy.field(
        query_hook=FruitFilterHook(), repository_type=StrawchemySyncRepository
    )
    field_filtered_fruit_aggregations: FruitAggregationType = strawchemy.field(
        query_hook=FruitFilterHook(), root_aggregations=True, repository_type=StrawchemySyncRepository
    )
    field_load_fruits: list[FruitType] = strawchemy.field(
        query_hook=QueryHook(load=[Fruit.water_percent]), repository_type=StrawchemySyncRepository
    )
    fruit_with_extension: FruitTypeHooks = strawchemy.field(
        filter_input=FruitFilter, repository_type=StrawchemySyncRepository, extensions=[SomeExtension()]
    )
    # Color
    color: ColorType = strawchemy.field(repository_type=StrawchemySyncRepository)
    colors: list[ColorType] = strawchemy.field(
        filter_input=ColorFilter,
        distinct_on=ColorDistinctOn,
        order_by_input=ColorOrder,
        repository_type=StrawchemySyncRepository,
    )
    colors_paginated: list[ColorTypeWithPagination] = strawchemy.field(
        pagination=True, repository_type=StrawchemySyncRepository
    )
    colors_nested_distinct: list[ColorTypeWithDistinctOn] = strawchemy.field(repository_type=StrawchemySyncRepository)
    colors_filterable_paginated: list[ColorType] = strawchemy.field(
        filter_input=ColorFilter,
        order_by_input=ColorOrder,
        pagination=True,
        repository_type=StrawchemySyncRepository,
    )
    colors_filtered: list[ColorType] = strawchemy.field(
        repository_type=StrawchemySyncRepository, filter_statement=lambda _: select(Color).where(Color.name == "Red")
    )
    colors_filtered_paginated: list[ColorType] = strawchemy.field(
        order_by_input=ColorOrder,
        pagination=True,
        repository_type=StrawchemySyncRepository,
        filter_statement=lambda _: select(Color).where(Color.name.in_(["Red", "Green", "Pink"])),
    )
    colors_filtered_distinct: list[ColorType] = strawchemy.field(
        distinct_on=ColorDistinctOn,
        order_by_input=ColorOrder,
        pagination=True,
        repository_type=StrawchemySyncRepository,
        filter_statement=lambda _: select(Color).where(Color.name.in_(["Red", "Green", "Pink"])),
    )
    colors_with_filtered_fruits: list[ColorWithFilteredFruit] = strawchemy.field(
        repository_type=StrawchemySyncRepository
    )
    colors_with_filtered_fruits_paginated: list[ColorWithFilteredFruit] = strawchemy.field(
        repository_type=StrawchemySyncRepository, pagination=True
    )
    colors_hooks: list[ColorTypeHooks] = strawchemy.field(repository_type=StrawchemySyncRepository)
    colors_hooks_paginated: list[ColorTypeHooks] = strawchemy.field(
        repository_type=StrawchemySyncRepository, pagination=True
    )
    colors_with_sweet_fruits: list[ColorWithSweetFruits] = strawchemy.field(
        filter_input=ColorFilter, repository_type=StrawchemySyncRepository
    )
    colors_with_sweet_fruits_paginated: list[ColorWithSweetFruits] = strawchemy.field(
        filter_input=ColorFilter, repository_type=StrawchemySyncRepository, pagination=True
    )
    colors_with_paginated_sweet_fruits: list[ColorWithPaginatedSweetFruits] = strawchemy.field(
        repository_type=StrawchemySyncRepository
    )
    colors_with_default_paginated_fruits: list[ColorWithDefaultPaginatedFruits] = strawchemy.field(
        repository_type=StrawchemySyncRepository
    )
    colors_with_multi_farm_fruits: list[ColorWithMultiFarmFruits] = strawchemy.field(
        filter_input=ColorFilter, repository_type=StrawchemySyncRepository
    )
    colors_with_ordered_fruits: list[ColorWithOrderedFruits] = strawchemy.field(
        repository_type=StrawchemySyncRepository
    )
    fruits_with_ordered_color: list[FruitWithOrderedColorType] = strawchemy.field(
        repository_type=StrawchemySyncRepository
    )
    fruits_with_ordered_color_paginated: list[FruitWithOrderedColorType] = strawchemy.field(
        pagination=True, repository_type=StrawchemySyncRepository
    )
    colors_with_paginated_ordered_fruits: list[ColorWithPaginatedOrderedFruits] = strawchemy.field(
        repository_type=StrawchemySyncRepository
    )
    colors_with_color_name_ordered_fruits: list[ColorWithColorNameOrderedFruits] = strawchemy.field(
        repository_type=StrawchemySyncRepository
    )
    color_name_ordered_fruits: list[ColorNameOrderedFruitType] = strawchemy.field(
        order_by_input=FruitOrderBy, distinct_on=FruitDistinctOn, repository_type=StrawchemySyncRepository
    )
    color_name_ordered_fruits_paginated: list[ColorNameOrderedFruitType] = strawchemy.field(
        order_by_input=FruitOrderBy,
        distinct_on=FruitDistinctOn,
        repository_type=StrawchemySyncRepository,
        pagination=True,
    )
    colors_fine_grained: list[ColorType] = strawchemy.field(
        filter_input=ColorFineGrainedFilter, repository_type=StrawchemySyncRepository
    )
    # User
    user: UserType = strawchemy.field(repository_type=StrawchemySyncRepository)
    users: list[UserType] = strawchemy.field(
        filter_input=UserFilter,
        order_by_input=UserOrderBy,
        repository_type=StrawchemySyncRepository,
        distinct_on=UserDistinctOn,
    )
    users_paginated: list[UserType] = strawchemy.field(
        filter_input=UserFilter,
        order_by_input=UserOrderBy,
        pagination=True,
        repository_type=StrawchemySyncRepository,
        distinct_on=UserDistinctOn,
    )

    # Custom resolvers
    @strawchemy.field
    def red_color(self, info: strawberry.Info) -> ColorType:
        repo = StrawchemySyncRepository(ColorType, info, filter_statement=select(Color).where(Color.name == "Red"))
        return repo.get_one().graphql_type()

    @strawchemy.field
    def get_color(self, info: strawberry.Info, color: str) -> ColorType | None:
        repo = StrawchemySyncRepository(ColorType, info, filter_statement=select(Color).where(Color.name == color))
        return repo.get_one_or_none().graphql_type_or_none()


@strawberry.type
class IntervalAsyncQuery:
    intervals: list[IntervalType] = strawchemy.field(
        filter_input=IntervalFilter, repository_type=StrawchemyAsyncRepository
    )


@strawberry.type
class IntervalSyncQuery:
    intervals: list[IntervalType] = strawchemy.field(
        filter_input=IntervalFilter, repository_type=StrawchemySyncRepository
    )


@strawberry.type
class JSONAsyncQuery:
    json: list[JSONType] = strawchemy.field(filter_input=JSONFilter, repository_type=StrawchemyAsyncRepository)


@strawberry.type
class JSONSyncQuery:
    json: list[JSONType] = strawchemy.field(filter_input=JSONFilter, repository_type=StrawchemySyncRepository)


@strawberry.type
class DateTimeAsyncQuery:
    date_times: list[DateTimeType] = strawchemy.field(
        filter_input=DateTimeFilter, repository_type=StrawchemyAsyncRepository
    )


@strawberry.type
class DateTimeSyncQuery:
    date_times: list[DateTimeType] = strawchemy.field(
        filter_input=DateTimeFilter, repository_type=StrawchemySyncRepository
    )


# Mutations


@strawberry.type
class AsyncMutation:
    # Color - Create
    create_color: ColorType = strawchemy.create(ColorCreateInput, repository_type=StrawchemyAsyncRepository)
    create_validated_color: ColorType | ValidationErrorType = strawchemy.create(
        ColorCreateInput,
        validation=PydanticValidation(ColorCreateValidation),
        repository_type=StrawchemyAsyncRepository,
    )
    create_colors: list[ColorType] = strawchemy.create(ColorCreateInput, repository_type=StrawchemyAsyncRepository)
    # Color - Update
    update_color: ColorType = strawchemy.update_by_ids(ColorUpdateInput, repository_type=StrawchemyAsyncRepository)
    update_validated_color: ColorType | ValidationErrorType = strawchemy.update_by_ids(
        ColorUpdateInput,
        validation=PydanticValidation(ColorPkUpdateValidation),
        repository_type=StrawchemyAsyncRepository,
    )
    update_colors: list[ColorType] = strawchemy.update_by_ids(
        ColorUpdateInput, repository_type=StrawchemyAsyncRepository
    )
    update_colors_filter: list[ColorType] = strawchemy.update(
        ColorPartial, ColorFilter, repository_type=StrawchemyAsyncRepository
    )
    # Color - Delete
    delete_color: list[ColorType] = strawchemy.delete(ColorFilter, repository_type=StrawchemyAsyncRepository)
    delete_colors: list[ColorType] = strawchemy.delete(ColorFilter, repository_type=StrawchemyAsyncRepository)
    # Fruit - Create
    create_fruit: FruitType = strawchemy.create(FruitCreateInput, repository_type=StrawchemyAsyncRepository)
    create_fruits: list[FruitType] = strawchemy.create(FruitCreateInput, repository_type=StrawchemyAsyncRepository)
    # Fruit - Update
    update_fruit: FruitType = strawchemy.update_by_ids(FruitUpdateInput, repository_type=StrawchemyAsyncRepository)
    update_fruits: list[FruitType] = strawchemy.update_by_ids(
        FruitUpdateInput, repository_type=StrawchemyAsyncRepository
    )
    update_fruits_filter: list[FruitType] = strawchemy.update(
        FruitPartial, FruitFilter, repository_type=StrawchemyAsyncRepository
    )
    # Fruit - upsert
    upsert_fruit: FruitType = strawchemy.upsert(
        FruitCreateInput,
        update_fields=FruitUpsertFields,
        conflict_fields=FruitUpsertConflictFields,
        repository_type=StrawchemyAsyncRepository,
    )
    upsert_fruits: list[FruitType] = strawchemy.upsert(
        FruitCreateInput,
        update_fields=FruitUpsertFields,
        conflict_fields=FruitUpsertConflictFields,
        repository_type=StrawchemyAsyncRepository,
    )
    # User - Update
    update_user: UserType = strawchemy.update_by_ids(UserUpdateInput, repository_type=StrawchemyAsyncRepository)
    create_user: UserType = strawchemy.create(UserCreate, repository_type=StrawchemyAsyncRepository)
    # User - Delete
    delete_users: list[UserType] = strawchemy.delete(repository_type=StrawchemyAsyncRepository)
    delete_users_filter: list[UserType] = strawchemy.delete(UserFilter, repository_type=StrawchemyAsyncRepository)
    update_users_filter: list[UserType] = strawchemy.update(
        UserPartial, UserFilter, repository_type=StrawchemyAsyncRepository
    )
    delete_users_restricted: list[UserType] = strawchemy.delete(
        UserFilter,
        repository_type=StrawchemyAsyncRepository,
        filter_statement=lambda _: select(User).where(User.id == 2),
    )
    update_users_restricted: list[UserType] = strawchemy.update(
        UserPartial,
        UserFilter,
        repository_type=StrawchemyAsyncRepository,
        filter_statement=lambda _: select(User).where(User.id == 2),
    )

    @strawberry.field
    async def create_blue_color(self, info: strawberry.Info, data: ColorCreateInput) -> ColorType:
        return (await StrawchemyAsyncRepository(ColorType, info).create(Input(data, name="New Blue"))).graphql_type()

    @strawberry.field
    async def create_apple_color(self, info: strawberry.Info, data: ColorCreateInput) -> ColorType:
        color_input = Input(data)
        color_input.instances[0].fruits.extend(
            [
                Fruit(name="New Apple", sweetness=1, water_percent=0.4),
                Fruit(name="New Strawberry", sweetness=1, water_percent=0.3),
            ]
        )
        return (await StrawchemyAsyncRepository(ColorType, info).create(color_input)).graphql_type()

    @strawberry.field
    async def create_color_for_existing_fruits(self, info: strawberry.Info, data: ColorCreateInput) -> ColorType:
        color_input = Input(data)
        session = cast("AsyncSession", info.context.session)
        apple, strawberry = (
            Fruit(name="New Apple", sweetness=1, water_percent=0.4),
            Fruit(name="New Strawberry", sweetness=1, water_percent=0.3),
        )
        session.add_all([apple, strawberry])
        await session.commit()
        session.expire(strawberry)
        color_input.instances[0].fruits.extend([apple, strawberry])
        return (await StrawchemyAsyncRepository(ColorType, info).create(color_input)).graphql_type()

    @strawberry.field
    async def create_red_fruit(self, info: strawberry.Info, data: FruitCreateInput) -> FruitType:
        fruit_input = Input(data)
        fruit_input.instances[0].color = Color(name="New Red")
        return (await StrawchemyAsyncRepository(FruitType, info).create(fruit_input)).graphql_type()

    @strawberry.field
    async def create_fruit_for_existing_color(self, info: strawberry.Info, data: FruitCreateInput) -> FruitType:
        fruit_input = Input(data)
        session = cast("AsyncSession", info.context.session)
        red = Color(name="New Red")
        session.add(red)
        await session.commit()
        fruit_input.instances[0].color = red
        return (await StrawchemyAsyncRepository(FruitType, info).create(fruit_input)).graphql_type()

    @strawberry.field
    async def create_color_manual_validation(
        self, info: strawberry.Info, data: ColorCreateInput
    ) -> ColorType | ValidationErrorType:
        try:
            return (
                await StrawchemyAsyncRepository(ColorType, info).create(
                    Input(data, PydanticValidation(ColorCreateValidation))
                )
            ).graphql_type()
        except InputValidationError as error:
            return error.graphql_type()

    @strawberry.field
    async def create_validated_ranked_user(
        self, info: strawberry.Info, data: RankedUserCreateInput
    ) -> RankedUserType | ValidationErrorType:
        try:
            user_input = Input(data, PydanticValidation(RankedUserCreateValidation), rank=1)
        except InputValidationError as error:
            return error.graphql_type()
        return (await StrawchemyAsyncRepository(RankedUserType, info).create(user_input)).graphql_type()

    @strawberry.field
    async def create_ranked_user(self, info: strawberry.Info, data: RankedUserCreateInput) -> RankedUserType:
        return (await StrawchemyAsyncRepository(RankedUserType, info).create(Input(data, rank=1))).graphql_type()


@strawberry.type
class SyncMutation:
    # Color - Create
    create_color: ColorType = strawchemy.create(ColorCreateInput, repository_type=StrawchemySyncRepository)
    create_validated_color: ColorType | ValidationErrorType = strawchemy.create(
        ColorCreateInput, validation=PydanticValidation(ColorCreateValidation), repository_type=StrawchemySyncRepository
    )
    create_colors: list[ColorType] = strawchemy.create(ColorCreateInput, repository_type=StrawchemySyncRepository)
    # Color - Update
    update_color: ColorType = strawchemy.update_by_ids(ColorUpdateInput, repository_type=StrawchemySyncRepository)
    update_validated_color: ColorType | ValidationErrorType = strawchemy.update_by_ids(
        ColorUpdateInput,
        validation=PydanticValidation(ColorPkUpdateValidation),
        repository_type=StrawchemySyncRepository,
    )
    update_colors: list[ColorType] = strawchemy.update_by_ids(
        ColorUpdateInput, repository_type=StrawchemySyncRepository
    )
    update_colors_filter: list[ColorType] = strawchemy.update(
        ColorPartial, ColorFilter, repository_type=StrawchemySyncRepository
    )
    # Color - Delete
    delete_color: list[ColorType] = strawchemy.delete(ColorFilter, repository_type=StrawchemySyncRepository)
    delete_colors: list[ColorType] = strawchemy.delete(ColorFilter, repository_type=StrawchemySyncRepository)
    # Fruit - Create
    create_fruit: FruitType = strawchemy.create(FruitCreateInput, repository_type=StrawchemySyncRepository)
    create_fruits: list[FruitType] = strawchemy.create(FruitCreateInput, repository_type=StrawchemySyncRepository)
    # Fruit - Update
    update_fruit: FruitType = strawchemy.update_by_ids(FruitUpdateInput, repository_type=StrawchemySyncRepository)
    update_fruits: list[FruitType] = strawchemy.update_by_ids(
        FruitUpdateInput, repository_type=StrawchemySyncRepository
    )
    update_fruits_filter: list[FruitType] = strawchemy.update(
        FruitPartial, FruitFilter, repository_type=StrawchemySyncRepository
    )
    # Fruit - upsert
    upsert_fruit: FruitType = strawchemy.upsert(
        FruitCreateInput,
        update_fields=FruitUpsertFields,
        conflict_fields=FruitUpsertConflictFields,
        repository_type=StrawchemySyncRepository,
    )
    upsert_fruits: list[FruitType] = strawchemy.upsert(
        FruitCreateInput,
        update_fields=FruitUpsertFields,
        conflict_fields=FruitUpsertConflictFields,
        repository_type=StrawchemySyncRepository,
    )
    # User - Update
    update_user: UserType = strawchemy.update_by_ids(UserUpdateInput, repository_type=StrawchemySyncRepository)
    create_user: UserType = strawchemy.create(UserCreate, repository_type=StrawchemySyncRepository)
    # User - Delete
    delete_users: list[UserType] = strawchemy.delete(repository_type=StrawchemySyncRepository)
    delete_users_filter: list[UserType] = strawchemy.delete(UserFilter, repository_type=StrawchemySyncRepository)
    update_users_filter: list[UserType] = strawchemy.update(
        UserPartial, UserFilter, repository_type=StrawchemySyncRepository
    )
    delete_users_restricted: list[UserType] = strawchemy.delete(
        UserFilter,
        repository_type=StrawchemySyncRepository,
        filter_statement=lambda _: select(User).where(User.id == 2),
    )
    update_users_restricted: list[UserType] = strawchemy.update(
        UserPartial,
        UserFilter,
        repository_type=StrawchemySyncRepository,
        filter_statement=lambda _: select(User).where(User.id == 2),
    )

    @strawberry.field
    def create_blue_color(self, info: strawberry.Info, data: ColorCreateInput) -> ColorType:
        return StrawchemySyncRepository(ColorType, info).create(Input(data, name="New Blue")).graphql_type()

    @strawberry.field
    def create_apple_color(self, info: strawberry.Info, data: ColorCreateInput) -> ColorType:
        color_input = Input(data)
        color_input.instances[0].fruits.extend(
            [
                Fruit(name="New Apple", sweetness=1, water_percent=0.4),
                Fruit(name="New Strawberry", sweetness=1, water_percent=0.3),
            ]
        )
        return StrawchemySyncRepository(ColorType, info).create(color_input).graphql_type()

    @strawberry.field
    def create_color_for_existing_fruits(self, info: strawberry.Info, data: ColorCreateInput) -> ColorType:
        color_input = Input(data)
        session = cast("Session", info.context.session)
        apple, strawberry = (
            Fruit(name="New Apple", sweetness=1, water_percent=0.4),
            Fruit(name="New Strawberry", sweetness=1, water_percent=0.3),
        )
        session.add_all([apple, strawberry])
        session.commit()
        session.expire(strawberry)
        color_input.instances[0].fruits.extend([apple, strawberry])
        return StrawchemySyncRepository(ColorType, info).create(color_input).graphql_type()

    @strawberry.field
    def create_red_fruit(self, info: strawberry.Info, data: FruitCreateInput) -> FruitType:
        fruit_input = Input(data)
        fruit_input.instances[0].color = Color(name="New Red")
        return StrawchemySyncRepository(FruitType, info).create(fruit_input).graphql_type()

    @strawberry.field
    def create_fruit_for_existing_color(self, info: strawberry.Info, data: FruitCreateInput) -> FruitType:
        fruit_input = Input(data)
        session = cast("Session", info.context.session)
        red = Color(name="New Red")
        session.add(red)
        session.commit()
        fruit_input.instances[0].color = red
        return StrawchemySyncRepository(FruitType, info).create(fruit_input).graphql_type()

    @strawberry.field
    def create_color_manual_validation(
        self, info: strawberry.Info, data: ColorCreateInput
    ) -> ColorType | ValidationErrorType:
        try:
            return (
                StrawchemySyncRepository(ColorType, info)
                .create(Input(data, PydanticValidation(ColorCreateValidation)))
                .graphql_type()
            )
        except InputValidationError as error:
            return error.graphql_type()

    @strawberry.field
    def create_validated_ranked_user(
        self, info: strawberry.Info, data: RankedUserCreateInput
    ) -> RankedUserType | ValidationErrorType:
        try:
            user_input = Input(data, PydanticValidation(RankedUserCreateValidation), rank=1)
        except InputValidationError as error:
            return error.graphql_type()
        return StrawchemySyncRepository(RankedUserType, info).create(user_input).graphql_type()

    @strawberry.field
    def create_ranked_user(self, info: strawberry.Info, data: RankedUserCreateInput) -> RankedUserType:
        return StrawchemySyncRepository(RankedUserType, info).create(Input(data, rank=1)).graphql_type()
