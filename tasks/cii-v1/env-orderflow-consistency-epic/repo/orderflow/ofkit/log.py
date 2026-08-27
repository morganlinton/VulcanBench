"""Structured-ish logging: one line per event, service-tagged, stderr."""

from __future__ import annotations

import logging
import os
import sys

_configured: set[str] = set()


def get_logger(service: str) -> logging.Logger:
    logger = logging.getLogger(f"orderflow.{service}")
    if service not in _configured:
        handler = logging.StreamHandler(sys.stderr)
        handler.setFormatter(
            logging.Formatter(f"%(asctime)s {service} %(levelname)s %(message)s")
        )
        logger.addHandler(handler)
        logger.setLevel(os.environ.get("OF_LOG_LEVEL", "WARNING"))
        logger.propagate = False
        _configured.add(service)
    return logger
