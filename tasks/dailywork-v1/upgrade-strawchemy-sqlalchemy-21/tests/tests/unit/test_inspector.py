from __future__ import annotations

from typing import Any

import pytest
from sqlalchemy import ARRAY, JSON, ForeignKey, Integer, inspect
from sqlalchemy.dialects import postgresql
from sqlalchemy.orm import (
    DeclarativeBase,
    Mapped,
    RelationshipProperty,
    mapped_column,
    registry,
    relationship,
)
from sqlalchemy.sql.sqltypes import NullType
from sqlalchemy.sql.type_api import TypeDecorator, TypeEngine

from strawchemy.dto.inspectors import SQLAlchemyInspector
from strawchemy.dto.types import DTOMissing


class _Base(DeclarativeBase):
    registry = registry()


class _JSONDecorator(TypeDecorator[Any]):
    impl = JSON
    cache_ok = True


class Department(_Base):
    __tablename__ = "rev_department"
    id: Mapped[int] = mapped_column(primary_key=True)
    # back_populates pair with User.department
    users: Mapped[list[User]] = relationship(back_populates="department")
    # backref: the reverse `Tag.department` attribute is generated implicitly
    tags: Mapped[list[Tag]] = relationship(backref="department")


class Group(_Base):
    __tablename__ = "rev_group"
    id: Mapped[int] = mapped_column(primary_key=True)
    # `users` key collides by name with Department.users, but targets a different class
    users: Mapped[list[User]] = relationship(back_populates="group")


class User(_Base):
    __tablename__ = "rev_user"
    id: Mapped[int] = mapped_column(primary_key=True)
    department_id: Mapped[int] = mapped_column(ForeignKey("rev_department.id"))
    group_id: Mapped[int] = mapped_column(ForeignKey("rev_group.id"))
    # both back_populates == "users": collision the guard must disambiguate
    department: Mapped[Department] = relationship(back_populates="users")
    group: Mapped[Group] = relationship(back_populates="users")
    # self-referential pair
    manager_id: Mapped[int | None] = mapped_column(ForeignKey("rev_user.id"))
    manager: Mapped[User | None] = relationship(remote_side="User.id", back_populates="reports")
    reports: Mapped[list[User]] = relationship(back_populates="manager")


class Tag(_Base):
    __tablename__ = "rev_tag"
    id: Mapped[int] = mapped_column(primary_key=True)
    department_id: Mapped[int] = mapped_column(ForeignKey("rev_department.id"))
    # `department` attribute is created by Department.tags' backref


class Author(_Base):
    __tablename__ = "rev_author"
    id: Mapped[int] = mapped_column(primary_key=True)
    # One-directional: only this side declares back_populates; Book.author omits it.
    # SQLAlchemy still wires _reverse_property symmetrically, so the second branch
    # of _reverse_relationships is required to match it from the Book side.
    books: Mapped[list[Book]] = relationship(back_populates="author")


class Book(_Base):
    __tablename__ = "rev_book"
    id: Mapped[int] = mapped_column(primary_key=True)
    author_id: Mapped[int] = mapped_column(ForeignKey("rev_author.id"))
    # No back_populates — the reverse link is one-directional (Author.books declares it)
    author: Mapped[Author] = relationship()


_Base.registry.configure()

_ALL_RELATIONSHIPS = [
    rel for model in (Department, Group, User, Tag, Author, Book) for rel in inspect(model).relationships
]


@pytest.mark.parametrize("rel", _ALL_RELATIONSHIPS, ids=lambda r: f"{r.parent.class_.__name__}.{r.key}")
def test_reverse_relationships_matches_private(rel: RelationshipProperty[Any]) -> None:
    """`_reverse_relationships` equals SQLAlchemy's private `_reverse_property` for every relationship."""
    assert SQLAlchemyInspector._reverse_relationships(rel) == set(rel._reverse_property)  # noqa: SLF001


@pytest.mark.parametrize(
    ("type_engine", "expected"),
    [
        pytest.param(Integer(), int, id="integer"),
        pytest.param(JSON(), dict, id="json"),
        pytest.param(postgresql.JSONB(), dict, id="jsonb"),
        pytest.param(JSON().with_variant(postgresql.JSONB(), "postgresql"), dict, id="json-variant"),
        pytest.param(_JSONDecorator(), dict, id="json-decorator"),
        pytest.param(ARRAY(Integer), list, id="array"),
        pytest.param(NullType(), DTOMissing, id="no-python-type"),
    ],
)
def test_python_type(type_engine: TypeEngine[Any], expected: type[Any]) -> None:
    """Test that column types map to the same Python type on SQLAlchemy 2.0 and 2.1."""
    assert SQLAlchemyInspector._python_type(type_engine) is expected  # noqa: SLF001
