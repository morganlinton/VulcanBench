"""Common service entrypoint: migrations, serve, announce readiness.

Every service main is ``main(router, migrations=..., database=...)``. The
launcher watches stdout for the ``READY <port>`` line.
"""

from __future__ import annotations

import argparse
import sys
import threading
from pathlib import Path

from orderflow.ofkit import pg
from orderflow.ofkit.http import Router, serve


def main(router: Router, migrations: str | None, database: str | None) -> None:
    parser = argparse.ArgumentParser(prog=f"orderflow.{router.service}")
    parser.add_argument("--port", type=int, default=0)
    args = parser.parse_args()

    if database and migrations:
        pg.apply_migrations(database, Path(migrations))

    _server, port = serve(router, args.port)
    print(f"READY {port}", flush=True)
    threading.Event().wait()  # serve until killed


def run_argv(argv: list[str]) -> None:  # pragma: no cover - convenience for tooling
    sys.argv = argv
