"""Per-service configuration, environment-driven with production defaults."""

from __future__ import annotations

import os
from dataclasses import dataclass


def _int(name: str, default: int) -> int:
    raw = os.environ.get(name)
    return int(raw) if raw not in (None, "") else default


@dataclass(frozen=True)
class Settings:
    # inventory
    reservation_ttl_s: int = 900
    low_stock_threshold: int = 5
    availability_cache_ttl_s: int = 3600
    # worker
    poll_batch: int = 50
    status_cache_ttl_s: int = 3600
    # billing
    settle_immediately: bool = True


def load() -> Settings:
    return Settings(
        reservation_ttl_s=_int("OF_RESERVATION_TTL_S", 900),
        low_stock_threshold=_int("OF_LOW_STOCK_THRESHOLD", 5),
        availability_cache_ttl_s=_int("OF_AVAILABILITY_CACHE_TTL_S", 3600),
        poll_batch=_int("OF_POLL_BATCH", 50),
        status_cache_ttl_s=_int("OF_STATUS_CACHE_TTL_S", 3600),
        settle_immediately=os.environ.get("OF_SETTLE_IMMEDIATELY", "1") != "0",
    )
