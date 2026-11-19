"""DB-free strawchemy schema over the unit ``User.departments`` secondary-table relationship.

Built with the ``postgresql`` dialect, but executed under any runtime dialect: the
transpiler re-reads the runtime dialect name at execution time, so the same static
schema emits dialect-specific SQL.
"""

from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import User

strawchemy = Strawchemy("postgresql")


@strawchemy.type(User, include="all", override=True, paginate="all")
class UserType: ...


@strawberry.type
class Query:
    users: list[UserType] = strawchemy.field()


schema = strawberry.Schema(query=Query)
