from __future__ import annotations

from typing import TypeAlias

from tests.integration.types import mysql, postgres, sqlite

__all__ = ("AnyAsyncMutationType", "AnyAsyncQueryType", "AnySyncMutationType", "AnySyncQueryType")

AnyAsyncQueryType: TypeAlias = "postgres.AsyncQuery | mysql.AsyncQuery | sqlite.AsyncQuery"
AnySyncQueryType: TypeAlias = "postgres.SyncQuery | mysql.SyncQuery | sqlite.SyncQuery"
AnyAsyncMutationType: TypeAlias = "postgres.AsyncMutation | mysql.AsyncMutation | sqlite.AsyncMutation"
AnySyncMutationType: TypeAlias = "postgres.SyncMutation | mysql.SyncMutation | sqlite.SyncMutation"
