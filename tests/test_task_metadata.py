"""Tests for task scale metadata helpers."""

from __future__ import annotations

from harness.task_metadata import (
    repo_scale,
    resolve_agent_timeout_s,
    resolve_max_steps,
    resolve_verifier_timeout_s,
    validate_scale_fields,
)


def test_repo_scale_default() -> None:
    assert repo_scale({}) == "micro"


def test_resolve_max_steps_from_hints() -> None:
    meta = {"repo_scale": "large", "agent_hints": {"suggested_max_steps": 120}}
    assert resolve_max_steps(meta) == 120


def test_resolve_max_steps_cli_caps_hints() -> None:
    meta = {"agent_hints": {"suggested_max_steps": 100}}
    assert resolve_max_steps(meta, cli_max_steps=30) == 30
    assert resolve_max_steps(meta) == 100


def test_resolve_agent_timeout_ignores_test_timeout_s() -> None:
    meta = {"repo_scale": "medium", "test_timeout_s": 120}
    assert resolve_agent_timeout_s(meta) == 1200.0
    assert resolve_verifier_timeout_s(meta) == 120


def test_resolve_agent_timeout_cli_cap() -> None:
    meta = {"repo_scale": "medium", "test_timeout_s": 120}
    assert resolve_agent_timeout_s(meta, cli_timeout=100.0) == 100.0


def test_resolve_verifier_timeout_default() -> None:
    assert resolve_verifier_timeout_s({}) == 120


def test_validate_scale_oss_requires_base_commit() -> None:
    reasons = validate_scale_fields(
        __import__("pathlib").Path("."),
        {"source": "oss", "upstream": {"url": "https://example.com"}},
    )
    assert any("base_commit" in r for r in reasons)


def test_validate_scale_rejects_placeholder_commit() -> None:
    reasons = validate_scale_fields(
        __import__("pathlib").Path("."),
        {
            "source": "oss",
            "base_commit": "0000000000000000000000000000000000000001",
            "upstream": {"url": "https://example.com"},
        },
    )
    assert any("placeholder" in r for r in reasons)
