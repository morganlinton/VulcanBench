from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import Fruit

strawchemy = Strawchemy("postgresql")


@strawchemy.upsert_conflict_fields(Fruit, include="all")
class FruitConflictFields: ...


@strawchemy.upsert_update_fields(Fruit, include="all")
class FruitUpdateFieldsInput: ...


@strawchemy.filter_update_input(Fruit, include="all")
class FruitPartial: ...


@strawberry.type
class Plain:
    name: str


@strawberry.type
class Mutation:
    plain: Plain = strawchemy.upsert(
        FruitPartial, conflict_fields=FruitConflictFields, update_fields=FruitUpdateFieldsInput
    )
