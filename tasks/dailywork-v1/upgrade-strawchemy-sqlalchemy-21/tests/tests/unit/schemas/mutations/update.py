from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import Group, SQLDataTypes, Tag

strawchemy = Strawchemy("postgresql")


@strawchemy.type(SQLDataTypes, include="all")
class SQLDataTypesType: ...


@strawchemy.type(Group, include="all")
class GroupType: ...


@strawchemy.type(Tag, include="all", override=True)
class TagType: ...


@strawchemy.pk_update_input(SQLDataTypes, include="all")
class SQLDataTypesUpdate: ...


@strawchemy.pk_update_input(Group, include="all")
class GroupUpdate: ...


@strawchemy.filter_update_input(Group, include="all")
class GroupPartial: ...


@strawchemy.filter(Group, include="all")
class GroupFilter: ...


@strawchemy.pk_update_input(Tag, include="all")
class TagUpdate: ...


@strawberry.type
class Mutation:
    update_data_type: SQLDataTypesType = strawchemy.update_by_ids(SQLDataTypesUpdate)
    update_data_types: list[SQLDataTypesType] = strawchemy.update_by_ids(SQLDataTypesUpdate)

    update_groups: list[GroupType] = strawchemy.update(GroupPartial, GroupFilter)
    update_group_by_id: GroupType = strawchemy.update_by_ids(GroupUpdate)
    update_groups_by_ids: list[GroupType] = strawchemy.update_by_ids(GroupUpdate)

    update_tag: TagType = strawchemy.update_by_ids(TagUpdate)
