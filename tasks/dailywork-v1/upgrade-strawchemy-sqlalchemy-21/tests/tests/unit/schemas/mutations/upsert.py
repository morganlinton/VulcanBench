from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import Fruit

strawchemy = Strawchemy("postgresql")


@strawchemy.type(Fruit, include="all")
class FruitType: ...


@strawchemy.upsert_conflict_fields(Fruit, include="all")
class FruitConflictFields: ...


@strawchemy.upsert_update_fields(Fruit, include="all")
class FruitUpdateFieldsInput: ...


@strawchemy.filter_update_input(Fruit, include="all")
class FruitPartial: ...


@strawberry.type
class Mutation:
    upsert_fruit: FruitType = strawchemy.upsert(
        FruitPartial, conflict_fields=FruitConflictFields, update_fields=FruitUpdateFieldsInput
    )
