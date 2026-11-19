from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import GeoModel

strawchemy = Strawchemy("postgresql")


@strawchemy.type(GeoModel, include="all")
class GeosFieldsType: ...


@strawberry.type
class Query:
    geo: GeosFieldsType
