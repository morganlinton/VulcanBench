from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from strawchemy.validation.pydantic import PydanticValidation
from tests.unit.models import Group, User

strawchemy = Strawchemy("postgresql")


@strawchemy.create_input(User, include="all")
class UserCreate: ...


@strawchemy.type(User, include="all")
class UserType: ...


@strawchemy.pydantic.create(Group, include="all")
class GroupCreateValidation: ...


@strawberry.type
class Mutation:
    create_user: UserType = strawchemy.create(UserCreate, validation=PydanticValidation(GroupCreateValidation))
