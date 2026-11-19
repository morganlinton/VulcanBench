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
class Query:
    groups: list[GroupType] = strawchemy.field(filter_input=GroupFilter)
