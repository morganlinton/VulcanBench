from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import Group

strawchemy = Strawchemy("postgresql")


@strawchemy.pk_update_input(Group, include="all")
class GroupUpdate: ...


@strawberry.type
class Plain:
    name: str


@strawberry.type
class Mutation:
    plain: Plain = strawchemy.update_by_ids(GroupUpdate)
