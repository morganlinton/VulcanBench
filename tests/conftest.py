"""Shared test fixtures.

Importing ``harness.cli`` calls ``load_dotenv()`` at module scope, so whichever
test imports it first injects the developer's real ``.env`` into ``os.environ``
for the rest of the session. Provider tests then see whatever route and keys that
machine happens to have configured, and pass or fail depending on the checkout
rather than the code.
"""

from __future__ import annotations

import pytest

from harness import provenance

# Base-URL overrides select which API a provider talks to; keys decide whether it
# believes it is configured at all. Tests that need either set them explicitly.
_ROUTING_ENV = (
    "OPENAI_BASE_URL",
    "ANTHROPIC_BASE_URL",
    "ZAI_BASE_URL",
    "MOONSHOT_BASE_URL",
    "DASHSCOPE_BASE_URL",
    "DEEPSEEK_BASE_URL",
    "META_BASE_URL",
    "OLLAMA_BASE_URL",
    "XAI_BASE_URL",
)

_CREDENTIAL_ENV = (
    "OPENAI_API_KEY",
    "ANTHROPIC_API_KEY",
    "ZAI_API_KEY",
    "MOONSHOT_API_KEY",
    "DASHSCOPE_API_KEY",
    "DEEPSEEK_API_KEY",
    "META_MUSE_SPARK_API",
    "MODEL_API_KEY",
    "OPENROUTER_API_KEY",
    "OLLAMA_API_KEY",
    "XAI_API_KEY",
)


@pytest.fixture(autouse=True)
def isolate_provider_env(monkeypatch: pytest.MonkeyPatch) -> None:
    """Give every test the same empty provider environment."""
    for name in (*_ROUTING_ENV, *_CREDENTIAL_ENV):
        monkeypatch.delenv(name, raising=False)


@pytest.fixture(autouse=True)
def allow_dirty_checkout(monkeypatch: pytest.MonkeyPatch) -> None:
    """Suite runs refuse a dirty checkout; tests must not depend on the developer's
    working tree. tests/test_provenance.py exercises the refusal explicitly."""
    monkeypatch.setenv(provenance.ALLOW_DIRTY_ENV, "1")
