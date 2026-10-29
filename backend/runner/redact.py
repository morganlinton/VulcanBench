"""Belt-and-braces redaction for text the Runs page serves to the browser."""

from __future__ import annotations

import re

from harness.redaction import REDACTED
from harness.redaction import redact as _harness_redact

_EXTRA: tuple[re.Pattern[str], ...] = (
    # JWT-shaped tokens (header.payload.signature). Over-redaction is fine here.
    re.compile(r"eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}"),
    # KEY=value assignments for anything that looks like a secret.
    re.compile(r"(?i)\b([A-Z0-9_]*(?:API_KEY|SECRET|TOKEN|PASSWORD))=\S+"),
    re.compile(r"(?i)(\"(?:access_token|refresh_token|api_key|id_token)\"\s*:\s*\")[^\"]{8,}"),
)


def redact(text: str) -> str:
    if not text:
        return text
    text = _harness_redact(text)
    text = _EXTRA[0].sub(REDACTED, text)
    text = _EXTRA[1].sub(rf"\1={REDACTED}", text)
    text = _EXTRA[2].sub(rf"\1{REDACTED}", text)
    return text
