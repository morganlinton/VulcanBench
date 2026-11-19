# ruff: noqa: TC003

from __future__ import annotations

import enum
from datetime import date, datetime, time, timedelta
from decimal import Decimal
from typing import Any
from uuid import UUID, uuid4

from sqlalchemy import VARCHAR, Column, DateTime, Enum, ForeignKey, Table, Text, UniqueConstraint, func
from sqlalchemy.dialects import postgresql
from sqlalchemy.ext.hybrid import hybrid_property
from sqlalchemy.orm import DeclarativeBase, Mapped, column_property, mapped_column, relationship

from strawchemy.constants import GEO_INSTALLED
from strawchemy.dto.types import Purpose, PurposeConfig
from strawchemy.dto.utils import PRIVATE, READ_ONLY, WRITE_ONLY, field


def validate_tomato_type(value: str) -> str:
    if "rotten" in value:
        msg = "We do not allow rotten tomato."
        raise ValueError(msg)
    return value


class VegetableFamily(enum.Enum):
    MUSHROOM = enum.auto()
    GOURD = enum.auto()
    CABBAGE = enum.auto()
    ONION = enum.auto()
    SEEDS = enum.auto()


class UUIDBase(DeclarativeBase):
    __abstract__ = True

    id: Mapped[UUID] = mapped_column(primary_key=True, default=uuid4)
    private: Mapped[str] = mapped_column(info=PRIVATE)


class NameDescriptionMixin(DeclarativeBase):
    __abstract__ = True

    name: Mapped[str] = mapped_column(VARCHAR)
    description: Mapped[str] = mapped_column(Text)


class Vegetable(UUIDBase, NameDescriptionMixin):
    __tablename__ = "vegetable"

    family: Mapped[VegetableFamily] = mapped_column(Enum(VegetableFamily))


class Fruit(UUIDBase):
    __tablename__ = "fruit"

    name: Mapped[str] = mapped_column()
    color_id: Mapped[UUID | None] = mapped_column(ForeignKey("color.id"))
    color: Mapped[Color] = relationship("Color", back_populates="fruits")
    sweetness: Mapped[int]

    __table_args__ = (UniqueConstraint(name, color_id, name="uq_name_color_id"),)


class Tomato(UUIDBase):
    __tablename__ = "tomato"

    name: Mapped[str] = mapped_column(
        info=field(
            purposes={Purpose.READ, Purpose.WRITE},
            configs={Purpose.WRITE: PurposeConfig(validator=validate_tomato_type)},
        )
    )
    weight: Mapped[float] = mapped_column(info=field(configs={Purpose.WRITE: PurposeConfig(type_override=int)}))
    sweetness: Mapped[float] = mapped_column(info=field(configs={Purpose.WRITE: PurposeConfig(alias="sugarness")}))
    popularity: Mapped[float] = mapped_column(info=field(configs={Purpose.WRITE: PurposeConfig(partial=True)}))


class Color(UUIDBase):
    __tablename__ = "color"

    fruits: Mapped[list[Fruit]] = relationship("Fruit", back_populates="color")
    name: Mapped[str]


class User(UUIDBase):
    __tablename__ = "user"

    name: Mapped[str]
    group_id: Mapped[UUID | None] = mapped_column(ForeignKey("group.id"))
    group: Mapped[Group] = relationship("Group", back_populates="users")
    tag_id: Mapped[UUID | None] = mapped_column(ForeignKey("tag.id"))
    tag: Mapped[Tag] = relationship("Tag", uselist=False)
    departments: Mapped[list[Department]] = relationship(
        "Department",
        secondary="user_department_join_table",
        back_populates="users",
    )

    @property
    def group_prop(self) -> Group | None:
        return self.group

    def get_group(self) -> Group | None:
        return self.group


UserDepartmentJoinTable = Table(
    "user_department_join_table",
    UUIDBase.metadata,
    Column("user_id", ForeignKey("user.id"), primary_key=True),
    Column("department_id", ForeignKey("department.id"), primary_key=True),
)


class Department(UUIDBase):
    __tablename__ = "department"

    name: Mapped[str | None] = mapped_column(VARCHAR(255), nullable=True)
    users: Mapped[list[User]] = relationship(
        User,
        secondary="user_department_join_table",
        back_populates="departments",
    )


class SponsoredUser(UUIDBase):
    __tablename__ = "sponsored_user"

    name: Mapped[str]
    sponsor_id: Mapped[UUID | None] = mapped_column(ForeignKey("sponsored_user.id"))
    sponsor: Mapped[SponsoredUser | None] = relationship("SponsoredUser", back_populates="sponsored")
    sponsored: Mapped[list[SponsoredUser]] = relationship(
        "SponsoredUser", back_populates="sponsor", remote_side="SponsoredUser.id", uselist=True
    )


class UserWithGreeting(UUIDBase):
    __tablename__ = "user_with_greeting"

    name: Mapped[str] = mapped_column()
    greeting_column_property: Mapped[str] = column_property("Hello, " + name)

    @hybrid_property
    def greeting_hybrid_property(self) -> str:
        return f"Hello, {self.name}"


class Group(UUIDBase):
    __tablename__ = "group"

    name: Mapped[str]
    tag_id: Mapped[UUID] = mapped_column(ForeignKey("tag.id"))
    tag: Mapped[Tag] = relationship("Tag", uselist=False, back_populates="groups")
    users: Mapped[list[User]] = relationship("User", back_populates="group")
    color_id: Mapped[UUID] = mapped_column(ForeignKey("color.id"))
    color: Mapped[Color] = relationship(Color)


class Admin(UUIDBase):
    __tablename__ = "admin"

    name: Mapped[str]
    password: Mapped[str] = mapped_column(info=WRITE_ONLY)


class Tag(UUIDBase):
    __tablename__ = "tag"

    groups: Mapped[list[Group]] = relationship(Group, back_populates="tag")
    name: Mapped[str]


class Book(UUIDBase):
    __tablename__ = "book"

    title: Mapped[str]
    isbn: Mapped[str] = mapped_column(info=READ_ONLY)


class SQLDataTypes(UUIDBase):
    __tablename__ = "sql_data_types"

    date_col: Mapped[date]
    time_col: Mapped[time]
    time_delta_col: Mapped[timedelta]
    datetime_col: Mapped[datetime] = mapped_column(DateTime(timezone=True))
    str_col: Mapped[str]
    int_col: Mapped[int]
    float_col: Mapped[float]
    decimal_col: Mapped[Decimal]
    bool_col: Mapped[bool]
    uuid_col: Mapped[UUID]
    dict_col: Mapped[dict[str, Any]] = mapped_column(postgresql.JSONB, default=dict)
    array_str_col: Mapped[list[str]] = mapped_column(postgresql.ARRAY(Text), default=list)


class Container(UUIDBase):
    """Test model with multiple list relationships for testing paginate/order configuration."""

    __tablename__ = "container"

    name: Mapped[str]
    fruits: Mapped[list[Fruit]] = relationship(
        "Fruit", primaryjoin="Container.id == foreign(Fruit.color_id)", viewonly=True
    )
    vegetables: Mapped[list[Vegetable]] = relationship(
        "Vegetable", primaryjoin="Container.id == foreign(Vegetable.id)", viewonly=True
    )
    colors: Mapped[list[Color]] = relationship("Color", primaryjoin="Container.id == foreign(Color.id)", viewonly=True)


class SQLDefaultBase(DeclarativeBase):
    __abstract__ = True


class TimestampedRecord(SQLDefaultBase):
    """Model exercising a client-side SQL-expression default (func.now())."""

    __tablename__ = "timestamped_record"

    id: Mapped[int] = mapped_column(primary_key=True)
    created_at: Mapped[datetime] = mapped_column(default=func.now())


# Geo

if GEO_INSTALLED:
    from geoalchemy2 import Geometry, WKBElement

    class GeoModel(UUIDBase):
        __tablename__ = "geos_fields"

        point_required: Mapped[WKBElement] = mapped_column(Geometry("POINT"))
        point: Mapped[WKBElement | None] = mapped_column(Geometry("POINT"), nullable=True)
        line_string: Mapped[WKBElement | None] = mapped_column(Geometry("LINESTRING"), nullable=True)
        polygon: Mapped[WKBElement | None] = mapped_column(Geometry("POLYGON"), nullable=True)
        multi_point: Mapped[WKBElement | None] = mapped_column(Geometry("MULTIPOINT"), nullable=True)
        multi_line_string: Mapped[WKBElement | None] = mapped_column(Geometry("MULTILINESTRING"), nullable=True)
        multi_polygon: Mapped[WKBElement | None] = mapped_column(Geometry("MULTIPOLYGON"), nullable=True)
        geometry: Mapped[WKBElement | None] = mapped_column(Geometry("GEOMETRY"), nullable=True)
