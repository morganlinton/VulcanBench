from __future__ import annotations

from typing import TYPE_CHECKING

import pytest
from strawberry.types import get_object_definition

from tests.unit.models import User

if TYPE_CHECKING:
    from strawchemy.mapper import Strawchemy

TYPE_DECORATOR_NAMES: list[str] = ["type", "aggregate", "filter", "aggregate_filter", "order"]


@pytest.mark.parametrize("decorator", TYPE_DECORATOR_NAMES)
def test_type_no_purpose_excluded(decorator: str, strawchemy: Strawchemy) -> None:
    @getattr(strawchemy, decorator)(User, include="all", override=True)
    class UserType: ...

    type_def = get_object_definition(UserType, strict=True)
    assert type_def.get_field("private") is None


@pytest.mark.parametrize("decorator", ["create_input", "pk_update_input", "filter_update_input"])
def test_type_no_purpose_excluded_input(decorator: str, strawchemy: Strawchemy) -> None:
    @getattr(strawchemy, decorator)(User, include="all")
    class UserType: ...

    type_def = get_object_definition(UserType, strict=True)
    assert type_def.get_field("private") is None
