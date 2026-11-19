from __future__ import annotations

from typing import Annotated

import strawberry
from pydantic import AfterValidator

from strawchemy import Input, InputValidationError, Strawchemy, StrawchemySyncRepository, ValidationErrorType
from strawchemy.validation.pydantic import PydanticValidation
from tests.unit.models import Group, User

strawchemy = Strawchemy("postgresql")


def _check_lower_case(value: str) -> str:
    if not value.islower():
        msg = "Name must be lower cased"
        raise ValueError(msg)
    return value


@strawchemy.create_input(User, include="all")
class UserCreate: ...


@strawchemy.filter_update_input(User, include="all")
class UserUpdate: ...


@strawchemy.pk_update_input(User, include="all")
class UserPkUpdate: ...


@strawchemy.type(User, include="all")
class UserType: ...


@strawchemy.filter(User, include="all")
class UserFilter: ...


# Validation


@strawchemy.pydantic.create(Group, include="all")
class GroupCreateValidation:
    name: Annotated[str, AfterValidator(_check_lower_case)]


@strawchemy.pydantic.create(User, include="all")
class UserCreateValidation:
    name: Annotated[str, AfterValidator(_check_lower_case)]
    group: GroupCreateValidation | None = strawberry.UNSET  # ty: ignore[invalid-type-form]


@strawchemy.pydantic.pk_update(User, include="all")
class UserPkUpdateValidation:
    name: Annotated[str, AfterValidator(_check_lower_case)]


@strawchemy.pydantic.filter_update(User, include="all")
class UserFilterValidation:
    name: Annotated[str, AfterValidator(_check_lower_case)]


@strawberry.type
class Mutation:
    create_user: UserType | ValidationErrorType = strawchemy.create(
        UserCreate, validation=PydanticValidation(UserCreateValidation)
    )
    missing_validation_in_type: UserType = strawchemy.create(
        UserCreate, validation=PydanticValidation(UserCreateValidation)
    )
    update_users: list[UserType | ValidationErrorType] = strawchemy.update(
        UserUpdate, filter_input=UserFilter, validation=PydanticValidation(UserFilterValidation)
    )
    update_user_by_id: UserType | ValidationErrorType = strawchemy.update_by_ids(
        UserPkUpdate, validation=PydanticValidation(UserPkUpdateValidation)
    )
    update_user_by_ids: list[UserType | ValidationErrorType] = strawchemy.update_by_ids(
        UserPkUpdate, validation=PydanticValidation(UserPkUpdateValidation)
    )

    @strawberry.field
    def create_user_custom(self, info: strawberry.Info, data: UserCreate) -> UserType | ValidationErrorType:
        try:
            return (
                StrawchemySyncRepository(UserType, info)
                .create(Input(data, PydanticValidation(UserCreateValidation)))
                .graphql_type()
            )
        except InputValidationError as error:
            return error.graphql_type()
