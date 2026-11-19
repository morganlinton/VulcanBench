from __future__ import annotations

from dataclasses import dataclass, field
from unittest.mock import MagicMock

from sqlalchemy import Dialect, Engine
from sqlalchemy.dialects import mysql, postgresql, sqlite
from sqlalchemy.orm import Session

from strawchemy.typing import SupportedDialect

SQLA_DIALECTS: dict[str, Dialect] = {
    "postgresql": postgresql.psycopg2.dialect(),
    "sqlite": sqlite.dialect(),
    "mysql": mysql.dialect(),
}
"""Real dialect objects used to compile captured statements (no DB connection)."""


@dataclass
class DialectContext:
    """Strawberry context whose fake session reports the requested dialect name.

    Only ``get_bind().dialect.name`` is read during planning, so the session is a
    ``MagicMock`` and never executes anything.
    """

    dialect_name: SupportedDialect
    session: MagicMock = field(init=False)

    def __post_init__(self) -> None:
        dialect = MagicMock(spec=Dialect, name="DialectMock")
        dialect.name = self.dialect_name
        engine = MagicMock(spec=Engine, name="EngineMock", dialect=dialect)
        self.session = MagicMock(spec=Session, name="SessionMock", get_bind=MagicMock(return_value=engine))
