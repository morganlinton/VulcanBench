from __future__ import annotations

import strawberry

from strawchemy import Strawchemy, StrawchemyAsyncRepository, StrawchemySyncRepository
from tests.integration.geo.models import GeoModel

strawchemy = Strawchemy("postgresql")


@strawchemy.type(GeoModel, include="all")
class GeoFieldsType: ...


@strawchemy.filter(GeoModel, include="all")
class GeoFieldsFilter: ...


@strawberry.type
class AsyncGeoQuery:
    geo_field: list[GeoFieldsType] = strawchemy.field(
        filter_input=GeoFieldsFilter, repository_type=StrawchemyAsyncRepository
    )


@strawberry.type
class SyncGeoQuery:
    geo_field: list[GeoFieldsType] = strawchemy.field(
        filter_input=GeoFieldsFilter, repository_type=StrawchemySyncRepository
    )
