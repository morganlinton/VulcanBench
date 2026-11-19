from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import Group, SQLDataTypes

strawchemy = Strawchemy("postgresql")


@strawchemy.type(SQLDataTypes, include="all")
class SQLDataTypesType: ...


@strawchemy.type(Group, include="all")
class GroupType: ...


@strawchemy.create_input(SQLDataTypes, include="all")
class SQLDataTypesCreate: ...


@strawchemy.create_input(Group, include="all")
class GroupInput: ...


@strawberry.type
class Mutation:
    create_data_type: SQLDataTypesType = strawchemy.create(SQLDataTypesCreate)
    create_data_types: list[SQLDataTypesType] = strawchemy.create(SQLDataTypesCreate)

    create_group: GroupType = strawchemy.create(GroupInput)
    create_groups: list[GroupType] = strawchemy.create(GroupInput)
