from __future__ import annotations

from datetime import date, datetime, time, timedelta
from typing import Any

from sqlalchemy import (
    ARRAY,
    JSON,
    VARCHAR,
    Column,
    Date,
    DateTime,
    Double,
    ForeignKey,
    Integer,
    Interval,
    MetaData,
    Sequence,
    Table,
    Text,
    Time,
    UniqueConstraint,
)
from sqlalchemy.dialects import mysql, postgresql, sqlite
from sqlalchemy.ext.declarative import declared_attr
from sqlalchemy.ext.hybrid import hybrid_property
from sqlalchemy.orm import DeclarativeBase, Mapped, MappedAsDataclass, column_property, mapped_column, relationship
from sqlalchemy.orm import registry as Registry  # noqa: N812

from strawchemy.dto.utils import PRIVATE, READ_ONLY

metadata = MetaData()
geo_metadata = MetaData()
dc_metadata = MetaData()
json_metadata = MetaData()
postgres_json_metadata = MetaData()
array_metadata = MetaData()
interval_metadata = MetaData()
date_time_metadata = MetaData()

TextArrayType = ARRAY(Text).with_variant(postgresql.ARRAY(Text), "postgresql")
JSONType = (
    JSON()
    .with_variant(postgresql.JSONB, "postgresql")
    .with_variant(mysql.JSON, "mysql")
    .with_variant(sqlite.JSON, "sqlite")
)
DateType = Date().with_variant(sqlite.DATE, "sqlite")
DateTimeType = DateTime().with_variant(sqlite.DATETIME, "sqlite")
TimeType = Time().with_variant(sqlite.TIME, "sqlite")

FREE_ID_RANGE = range(1, 300)

# Bases


class BaseColumns:
    @declared_attr
    @classmethod
    def id(cls) -> Mapped[int]:
        """BigInt Primary key column."""
        return mapped_column(
            Integer,
            Sequence(f"{cls.__tablename__}_id_seq", start=FREE_ID_RANGE.stop, optional=False),  # ty: ignore[unresolved-attribute]
            primary_key=True,
        )

    created_at: Mapped[datetime] = mapped_column(DateTimeType, default=datetime.now, info=READ_ONLY)
    """Date/time of instance creation."""
    updated_at: Mapped[datetime] = mapped_column(
        DateTimeType, default=datetime.now, onupdate=datetime.now, info=READ_ONLY
    )


class Base(BaseColumns, DeclarativeBase):
    __abstract__ = True
    registry = Registry(metadata=metadata)


class DataclassBase(BaseColumns, MappedAsDataclass, DeclarativeBase):
    __abstract__ = True
    registry = Registry(metadata=dc_metadata)


class GeoUUIDBase(BaseColumns, DeclarativeBase):
    __abstract__ = True
    registry = Registry(metadata=geo_metadata)


class ArrayBase(BaseColumns, DeclarativeBase):
    __abstract__ = True
    registry = Registry(metadata=array_metadata)


class JSONBase(BaseColumns, DeclarativeBase):
    __abstract__ = True
    registry = Registry(metadata=json_metadata)


class PostgresJSONBase(BaseColumns, DeclarativeBase):
    __abstract__ = True
    registry = Registry(metadata=postgres_json_metadata)


class IntervalBase(BaseColumns, DeclarativeBase):
    __abstract__ = True
    registry = Registry(metadata=interval_metadata)


class DateTimeBase(BaseColumns, DeclarativeBase):
    __abstract__ = True
    registry = Registry(metadata=date_time_metadata)


# Models


class FruitFarm(Base):
    __tablename__ = "fruit_farm"

    name: Mapped[str] = mapped_column(Text)
    fruit_id: Mapped[int] = mapped_column(ForeignKey("fruit.id"), info=PRIVATE)


class DerivedProduct(Base):
    __tablename__ = "derived_product"

    name: Mapped[str] = mapped_column(Text)


class Fruit(Base):
    __tablename__ = "fruit"

    name: Mapped[str] = mapped_column(VARCHAR(255))
    color_id: Mapped[int | None] = mapped_column(ForeignKey("color.id"), nullable=True, default=None)
    color: Mapped[Color | None] = relationship("Color", back_populates="fruits")
    farms: Mapped[list[FruitFarm]] = relationship(FruitFarm)
    derived_product_id: Mapped[int | None] = mapped_column(
        ForeignKey("derived_product.id"), nullable=True, default=None
    )
    product: Mapped[DerivedProduct | None] = relationship(DerivedProduct)
    sweetness: Mapped[int] = mapped_column(Integer)
    water_percent: Mapped[float] = mapped_column(Double)
    best_time_to_pick: Mapped[time] = mapped_column(TimeType, default=time(hour=9))

    __table_args__ = (UniqueConstraint(name), UniqueConstraint(sweetness, water_percent))

    @hybrid_property
    def description(self) -> str:
        return f"The {self.name} color id is {self.color_id}"


class Color(Base):
    __tablename__ = "color"

    fruits: Mapped[list[Fruit]] = relationship("Fruit", back_populates="color")
    name: Mapped[str] = mapped_column(VARCHAR(255))

    __table_args__ = (UniqueConstraint(name),)


class Group(Base):
    __tablename__ = "group"

    name: Mapped[str | None] = mapped_column(Text)
    topics: Mapped[list[Topic]] = relationship("Topic")


class Topic(Base):
    __tablename__ = "topic"

    name: Mapped[str] = mapped_column(Text)
    group_id: Mapped[int] = mapped_column(ForeignKey("group.id"))


class User(Base):
    __tablename__ = "user"

    name: Mapped[str] = mapped_column(Text)
    greeting: Mapped[str] = column_property("Hello, " + name)
    group_id: Mapped[int | None] = mapped_column(ForeignKey("group.id"))
    group: Mapped[Group | None] = relationship(Group)
    bio: Mapped[str | None] = mapped_column(Text, default=None)
    departments: Mapped[list[Department]] = relationship(
        "Department",
        secondary="user_department_join_table",
        back_populates="users",
    )
    filtered_departments: Mapped[list[Department]] = relationship(
        "Department",
        secondary="user_department_join_table",
        primaryjoin="and_(User.id == user_department_join_table.c.user_id, User.name != 'Bob')",
        secondaryjoin="and_(Department.id == user_department_join_table.c.department_id, Department.name != 'IT')",
        viewonly=True,
    )

    def __init__(self, **kw: Any) -> None:
        super().__init__(**kw)
        if self.bio is None:
            self.bio = "Lorem ipsum dolor sit amet, consectetur adipiscing elit"


UserDepartmentJoinTable = Table(
    "user_department_join_table",
    Base.metadata,
    Column("user_id", ForeignKey("user.id", ondelete="CASCADE"), primary_key=True),
    Column("department_id", ForeignKey("department.id", ondelete="CASCADE"), primary_key=True),
)


class Department(Base):
    __tablename__ = "department"

    name: Mapped[str] = mapped_column(VARCHAR(255), nullable=True)
    users: Mapped[list[User]] = relationship(
        User,
        secondary="user_department_join_table",
        back_populates="departments",
    )


class RankedUser(Base):
    __tablename__ = "ranked_user"

    name: Mapped[str] = mapped_column(Text)
    rank: Mapped[int] = mapped_column(info=READ_ONLY)


# Specific data types models


class ArrayModel(ArrayBase):
    __tablename__ = "array_model"

    array_str_col: Mapped[list[str]] = mapped_column(TextArrayType, default=list)


class IntervalModel(IntervalBase):
    __tablename__ = "interval_model"

    time_delta_col: Mapped[timedelta] = mapped_column(Interval)


class JSONModel(JSONBase):
    __tablename__ = "json_model"

    dict_col: Mapped[dict[str, Any]] = mapped_column(JSONType, default=dict)


class PostgresJSONModel(PostgresJSONBase):
    __tablename__ = "postgres_json_model"

    dict_col: Mapped[dict[str, Any] | None] = mapped_column(postgresql.JSON, nullable=True)
    children: Mapped[list[PostgresJSONChildModel]] = relationship("PostgresJSONChildModel")


class PostgresJSONChildModel(PostgresJSONBase):
    __tablename__ = "postgres_json_child_model"

    dict_col: Mapped[dict[str, Any] | None] = mapped_column(postgresql.JSON, nullable=True)
    parent_id: Mapped[int] = mapped_column(ForeignKey("postgres_json_model.id"))


class DateTimeModel(DateTimeBase):
    __tablename__ = "date_time_model"

    date_col: Mapped[date] = mapped_column(DateType)
    time_col: Mapped[time] = mapped_column(TimeType)
    datetime_col: Mapped[datetime] = mapped_column(DateTimeType)
