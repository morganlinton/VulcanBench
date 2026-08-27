#!/usr/bin/env python3
"""Run the whole orderflow deployment locally, until Ctrl-C.

Needs the infrastructure stack up first (postgres + redis published in
``.vb_services.json``). Prints each service URL, then idles.
"""

from __future__ import annotations

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from orderflow import launcher  # noqa: E402


def main() -> None:
    deployment = launcher.start(Path(__file__).resolve().parents[1])
    for service, url in deployment.urls.items():
        print(f"{service:>10}  {url}")
    print("stack is up; Ctrl-C to stop")
    try:
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        deployment.stop()


if __name__ == "__main__":
    main()
