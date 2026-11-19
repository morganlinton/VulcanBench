from __future__ import annotations

from uuid import UUID, uuid4

from sqlalchemy import ForeignKey
from sqlalchemy.orm import Mapped, mapped_column, relationship

from strawchemy import Strawchemy
from strawchemy.dto.utils import READ_ONLY
from tests.unit.models import UUIDBase

strawchemy = Strawchemy("postgresql")


class NewUser(UUIDBase):
    __tablename__ = "new_user"

    id: Mapped[UUID] = mapped_column(primary_key=True, default=uuid4, info=READ_ONLY)
    group_id: Mapped[UUID] = mapped_column(ForeignKey("new_group.id"))


class NewGroup(UUIDBase):
    __tablename__ = "new_group"

    users: Mapped[list[NewUser]] = relationship(NewUser)


@strawchemy.pk_update_input(NewGroup, include="all")
class GroupInput: ...
