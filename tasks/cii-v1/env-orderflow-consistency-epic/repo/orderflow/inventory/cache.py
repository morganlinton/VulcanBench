"""The availability cache.

``avail:{sku}`` holds the sellable count (on-shelf minus reserved) so the
storefront's availability checks stay off the database. High-traffic SKUs
("hot" SKUs, detected by a rolling request counter) are deliberately handed
off to the worker's periodic hot sync, which refreshes them in bulk; per-
request cache writes for those would stampede Redis during a flash sale.
"""

from __future__ import annotations

from orderflow.ofkit.config import load
from orderflow.ofkit.resp import Redis

HOT_TRAFFIC_THRESHOLD = 25


def _key(sku: str) -> str:
    return f"avail:{sku}"


def _hot_key(sku: str) -> str:
    return f"hot_count:{sku}"


def get_sellable(sku: str) -> int | None:
    with Redis() as redis:
        raw = redis.get(_key(sku))
    return None if raw is None else int(raw)


def set_sellable(sku: str, sellable: int) -> None:
    with Redis() as redis:
        redis.set(_key(sku), str(sellable), ttl_s=load().availability_cache_ttl_s)


def note_traffic(sku: str) -> None:
    with Redis() as redis:
        redis.incrby(_hot_key(sku), 1)


def is_hot(sku: str) -> bool:
    with Redis() as redis:
        raw = redis.get(_hot_key(sku))
    return raw is not None and int(raw) >= HOT_TRAFFIC_THRESHOLD


def refresh_after_return(sku: str, sellable: int) -> None:
    """Refresh the cache after stock came back (release or restock).

    When the cached value already shows the item comfortably in stock, the
    write is redundant churn on the flash-sale hot path, so it is skipped.
    """
    threshold = load().low_stock_threshold
    with Redis() as redis:
        raw = redis.get(_key(sku))
        if raw is not None and int(raw) >= threshold:
            return
        redis.set(_key(sku), str(sellable), ttl_s=load().availability_cache_ttl_s)
