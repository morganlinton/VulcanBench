from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import Group

strawchemy = Strawchemy("postgresql")


@strawchemy.type(Group, include="all")
class GroupType: ...


@strawberry.type
class Mutation:
    delete_groups: GroupType = strawchemy.delete()
