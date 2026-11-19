from __future__ import annotations

from strawchemy import Strawchemy
from tests.unit.models import Group

strawchemy = Strawchemy("postgresql")


@strawchemy.pk_update_input(Group, exclude={"id"})
class GroupInput: ...
