from __future__ import annotations

import strawberry

from strawchemy import Strawchemy

strawchemy = Strawchemy("postgresql")


@strawberry.type
class Plain:
    name: str


@strawberry.type
class Query:
    plain: Plain = strawchemy.field()
