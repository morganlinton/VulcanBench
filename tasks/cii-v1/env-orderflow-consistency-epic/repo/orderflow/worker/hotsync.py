"""Bulk availability refresh for hot SKUs.

The inventory service skips per-request cache writes for SKUs under flash-
sale traffic; this job refreshes those keys in bulk instead. SETNX is used
so a concurrent per-request write (a SKU that just cooled down) is never
stomped by a bulk value computed moments earlier.
"""

from __future__ import annotations

from orderflow.ofkit import client, pg, topology
from orderflow.ofkit.config import load
from orderflow.ofkit.resp import Redis
from orderflow.inventory.cache import HOT_TRAFFIC_THRESHOLD


def hot_skus() -> list[str]:
    with pg.connect("inventory_db") as conn, conn.cursor() as cur:
        cur.execute("SELECT sku FROM products ORDER BY sku")
        skus = [row[0] for row in cur.fetchall()]
    hot = []
    with Redis() as redis:
        for sku in skus:
            raw = redis.get(f"hot_count:{sku}")
            if raw is not None and int(raw) >= HOT_TRAFFIC_THRESHOLD:
                hot.append(sku)
    return hot


def run() -> dict:
    refreshed = 0
    ttl = load().availability_cache_ttl_s
    for sku in hot_skus():
        info = client.get(
            topology.service_url("inventory") + f"/internal/products/{sku}"
        )
        sellable = int(info["available"]) - int(info["reserved"])
        with Redis() as redis:
            if redis.setnx(f"avail:{sku}", str(sellable)):
                redis.command("EXPIRE", f"avail:{sku}", str(ttl))
                refreshed += 1
    return {"hot": len(hot_skus()), "refreshed": refreshed}
