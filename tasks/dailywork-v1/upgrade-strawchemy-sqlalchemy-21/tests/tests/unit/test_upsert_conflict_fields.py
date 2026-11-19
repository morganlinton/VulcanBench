from __future__ import annotations

from typing import TYPE_CHECKING
from uuid import UUID, uuid4

import pytest
from sqlalchemy import UniqueConstraint, select
from sqlalchemy.orm import DeclarativeBase, Mapped, mapped_column

from strawchemy import Strawchemy
from strawchemy.dto.types import DTOConfig, Purpose, PurposeConfig
from strawchemy.dto.utils import PRIVATE, READ_ONLY, field
from strawchemy.exceptions import EmptyDTOError, StrawchemyError
from tests.unit.models import Fruit

if TYPE_CHECKING:
    from strawchemy.typing import SupportedDialect

_DIALECTS: tuple[SupportedDialect, ...] = ("postgresql", "mysql", "sqlite")


class _Base(DeclarativeBase): ...


class _Ledger(_Base):
    """Carries unique constraints on columns barred from write input."""

    __tablename__ = "ucf_ledger"

    id: Mapped[UUID] = mapped_column(primary_key=True, default=uuid4)
    name: Mapped[str] = mapped_column()
    serial: Mapped[str] = mapped_column(info=READ_ONLY, unique=True)
    secret: Mapped[str] = mapped_column(info=PRIVATE, unique=True)


class _Aliased(_Base):
    """Carries a unique column renamed for write purpose only."""

    __tablename__ = "ucf_aliased"

    id: Mapped[UUID] = mapped_column(primary_key=True, default=uuid4)
    sweetness: Mapped[int] = mapped_column(info=field(configs={Purpose.WRITE: PurposeConfig(alias="sugarness")}))
    weight: Mapped[int] = mapped_column()

    __table_args__ = (UniqueConstraint(sweetness, weight, name="uq_sweetness_weight"),)


class _Unconstrained(_Base):
    """Mapped to a subquery, so the inspector reports no unique constraint at all."""

    __table__ = select(_Ledger.__table__.c.id, _Ledger.__table__.c.name).subquery("ucf_unconstrained")
    __mapper_args__ = {"primary_key": [__table__.c.id]}  # noqa: RUF012


@pytest.mark.parametrize("dialect", _DIALECTS)
def test_upsert_conflict_fields_honors_include(dialect: SupportedDialect) -> None:
    """Test that a constraint is kept only when every one of its columns is included."""
    strawchemy = Strawchemy(dialect)

    @strawchemy.upsert_conflict_fields(Fruit, include=["name", "color_id"])
    class ConflictFields: ...

    assert [field_.name for field_ in ConflictFields] == ["nameAndColorId"]


@pytest.mark.parametrize("dialect", _DIALECTS)
def test_upsert_conflict_fields_rejects_partial_constraint_include(dialect: SupportedDialect) -> None:
    """Test that including one column of a composite constraint leaves no conflict target."""
    strawchemy = Strawchemy(dialect)

    with pytest.raises(EmptyDTOError):

        @strawchemy.upsert_conflict_fields(Fruit, include=["name"])
        class ConflictFields: ...


@pytest.mark.parametrize("dialect", _DIALECTS)
def test_upsert_conflict_fields_rejects_empty_include(dialect: SupportedDialect) -> None:
    """Test that an empty include selects nothing rather than falling back to every constraint."""
    strawchemy = Strawchemy(dialect)

    with pytest.raises(EmptyDTOError):

        @strawchemy.upsert_conflict_fields(Fruit, include=[])
        class ConflictFields: ...


def test_upsert_conflict_fields_empty_selection_error_is_strawchemy_error() -> None:
    """Test that an unsatisfiable selection surfaces as a strawchemy error."""
    strawchemy = Strawchemy("sqlite")

    with pytest.raises(StrawchemyError):

        @strawchemy.upsert_conflict_fields(Fruit, include=["sweetness"])
        class ConflictFields: ...


def test_upsert_conflict_fields_honors_exclude() -> None:
    """Test that excluding a constraint column drops that constraint."""
    strawchemy = Strawchemy("sqlite")

    @strawchemy.upsert_conflict_fields(Fruit, include="all", exclude=["name"])
    class ConflictFields: ...

    assert [field_.name for field_ in ConflictFields] == ["id"]


def test_upsert_conflict_fields_default_includes_all_constraints() -> None:
    """Test that an unfiltered config keeps every unique constraint of the model."""
    strawchemy = Strawchemy("sqlite")

    @strawchemy.upsert_conflict_fields(Fruit)
    class ConflictFields: ...

    assert [field_.name for field_ in ConflictFields] == ["id", "nameAndColorId"]


def test_upsert_conflict_fields_skips_relations_and_unconstrained_columns() -> None:
    """Test that a selected relation or plain column is not a conflict target."""
    strawchemy = Strawchemy("sqlite")

    @strawchemy.upsert_conflict_fields(Fruit, include="all")
    class ConflictFields: ...

    assert [field_.name for field_ in ConflictFields] == ["id", "nameAndColorId"]


def test_upsert_conflict_fields_honors_global_include() -> None:
    """Test that a constraint outside `global_include` is dropped even when `include` is unset."""
    strawchemy = Strawchemy("sqlite")
    dto = strawchemy.upsert_conflict_factory.factory(
        _Ledger, DTOConfig(Purpose.WRITE, global_include=["id", "serial"]), name="LedgerGlobalInclude"
    )

    assert [field_.name for field_ in dto] == ["id", "serial"]


def test_upsert_conflict_fields_global_include_can_reject_every_constraint() -> None:
    """Test that a `global_include` matching no whole constraint is reported, not ignored."""
    strawchemy = Strawchemy("sqlite")

    with pytest.raises(EmptyDTOError):
        strawchemy.upsert_conflict_factory.factory(
            _Ledger, DTOConfig(Purpose.WRITE, global_include=["name"]), name="LedgerGlobalIncludeMiss"
        )


def test_upsert_conflict_fields_honors_global_exclude() -> None:
    """Test that a constraint column in `global_exclude` drops that constraint."""
    strawchemy = Strawchemy("sqlite")
    dto = strawchemy.upsert_conflict_factory.factory(
        _Ledger, DTOConfig(Purpose.WRITE, global_exclude=["serial"]), name="LedgerGlobalExclude"
    )

    assert [field_.name for field_ in dto] == ["id", "secret"]


@pytest.mark.parametrize("dialect", _DIALECTS)
def test_upsert_conflict_fields_keeps_read_only_and_private_columns(dialect: SupportedDialect) -> None:
    """Test that read-only and private columns remain conflict targets of a write DTO."""
    strawchemy = Strawchemy(dialect)

    @strawchemy.upsert_conflict_fields(_Ledger)
    class ConflictFields: ...

    assert [field_.name for field_ in ConflictFields] == ["id", "secret", "serial"]


def test_upsert_conflict_fields_keeps_read_only_column_under_include_all() -> None:
    """Test that a read-only column is included by the `all` selector like any other column."""
    strawchemy = Strawchemy("sqlite")

    @strawchemy.upsert_conflict_fields(_Ledger, include="all")
    class ConflictFields: ...

    assert [field_.name for field_ in ConflictFields] == ["id", "secret", "serial"]


def test_upsert_conflict_fields_names_use_purpose_alias() -> None:
    """Test that a write-purpose alias names the generated conflict field."""
    strawchemy = Strawchemy("sqlite")

    @strawchemy.upsert_conflict_fields(_Aliased)
    class ConflictFields: ...

    assert [field_.name for field_ in ConflictFields] == ["id", "sugarnessAndWeight"]


def test_upsert_conflict_fields_warns_without_constraints() -> None:
    """Test that a model without unique constraints only warns when no selection was made."""
    strawchemy = Strawchemy("sqlite")

    with pytest.warns(UserWarning, match="have no fields"):
        dto = strawchemy.upsert_conflict_factory.factory(
            _Unconstrained, DTOConfig(Purpose.WRITE), name="UnconstrainedConflictFields"
        )

    assert list(dto) == []


def test_upsert_conflict_fields_raises_without_constraints_when_asked() -> None:
    """Test that `if_no_fields="raise"` turns the missing-constraint warning into an error."""
    strawchemy = Strawchemy("sqlite")

    with pytest.raises(EmptyDTOError):
        strawchemy.upsert_conflict_factory.factory(
            _Unconstrained,
            DTOConfig(Purpose.WRITE),
            name="UnconstrainedConflictFieldsRaise",
            if_no_fields="raise",
        )
