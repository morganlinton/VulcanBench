"""Postgres access: one database per service, migrations applied on boot.

Each service owns exactly one database (``orders_db``, ``inventory_db``,
``billing_db``); cross-service reads go through HTTP or events, never SQL.
The worker is the one deliberate exception: it owns no database but holds
read connections for building the read model, plus write access to
``orders_db.order_read_model`` (its output table lives with the data it
summarizes).
"""

from __future__ import annotations

from pathlib import Path

import psycopg2
import psycopg2.extensions

from orderflow.ofkit import topology

_ADMIN_DB = "vb"


def connect(dbname: str) -> psycopg2.extensions.connection:
    return psycopg2.connect(
        host="127.0.0.1",
        port=topology.infra_port("postgres", "5432"),
        user="vb",
        password="vb",
        dbname=dbname,
    )


def ensure_database(dbname: str) -> None:
    """Create ``dbname`` if missing (CREATE DATABASE cannot run in a txn)."""
    conn = connect(_ADMIN_DB)
    try:
        conn.autocommit = True
        with conn.cursor() as cur:
            cur.execute("SELECT 1 FROM pg_database WHERE datname = %s", (dbname,))
            if cur.fetchone() is None:
                cur.execute(f'CREATE DATABASE "{dbname}"')
    finally:
        conn.close()


def apply_migrations(dbname: str, migrations_dir: Path) -> None:
    """Apply ``NNN_*.sql`` files in order, tracked in ``schema_migrations``."""
    ensure_database(dbname)
    conn = connect(dbname)
    try:
        with conn, conn.cursor() as cur:
            cur.execute(
                "CREATE TABLE IF NOT EXISTS schema_migrations ("
                "  name text PRIMARY KEY,"
                "  applied_at timestamptz NOT NULL DEFAULT now()"
                ")"
            )
        for path in sorted(migrations_dir.glob("[0-9][0-9][0-9]_*.sql")):
            with conn, conn.cursor() as cur:
                cur.execute("SELECT 1 FROM schema_migrations WHERE name = %s", (path.name,))
                if cur.fetchone() is not None:
                    continue
                cur.execute(path.read_text(encoding="utf-8"))
                cur.execute("INSERT INTO schema_migrations (name) VALUES (%s)", (path.name,))
    finally:
        conn.close()
