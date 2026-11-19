from __future__ import annotations

from geoalchemy2 import Geometry, WKBElement
from sqlalchemy import MetaData
from sqlalchemy.orm import DeclarativeBase, Mapped, mapped_column
from sqlalchemy.orm import registry as Registry  # noqa: N812

from tests.integration.models import BaseColumns

metadata, geo_metadata = MetaData(), MetaData()


class GeoUUIDBase(BaseColumns, DeclarativeBase):
    __abstract__ = True
    registry = Registry(metadata=geo_metadata)


class GeoModel(GeoUUIDBase):
    __tablename__ = "geos_fields"

    point_required: Mapped[WKBElement] = mapped_column(Geometry("POINT", srid=4326, spatial_index=False))
    point: Mapped[WKBElement | None] = mapped_column(
        Geometry("POINT", srid=4326, spatial_index=False, nullable=True), nullable=True
    )
    line_string: Mapped[WKBElement | None] = mapped_column(
        Geometry("LINESTRING", srid=4326, spatial_index=False), nullable=True
    )
    polygon: Mapped[WKBElement | None] = mapped_column(
        Geometry("POLYGON", srid=4326, spatial_index=False), nullable=True
    )
    multi_point: Mapped[WKBElement | None] = mapped_column(
        Geometry("MULTIPOINT", srid=4326, spatial_index=False), nullable=True
    )
    multi_line_string: Mapped[WKBElement | None] = mapped_column(
        Geometry("MULTILINESTRING", srid=4326, spatial_index=False), nullable=True
    )
    multi_polygon: Mapped[WKBElement | None] = mapped_column(
        Geometry("MULTIPOLYGON", srid=4326, spatial_index=False), nullable=True
    )
    geometry: Mapped[WKBElement | None] = mapped_column(
        Geometry("GEOMETRY", srid=4326, spatial_index=False), nullable=True
    )
