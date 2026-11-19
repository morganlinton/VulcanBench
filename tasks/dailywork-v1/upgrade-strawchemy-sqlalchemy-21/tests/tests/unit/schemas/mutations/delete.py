from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import Group

strawchemy = Strawchemy("postgresql")


@strawchemy.type(Group, include="all")
class GroupType: ...


@strawchemy.filter(Group, include="all")
class GroupFilter: ...


@strawberry.type
class Mutation:
    delete_groups: list[GroupType] = strawchemy.delete()
    delete_groups_filter: list[GroupType] = strawchemy.delete(GroupFilter)
