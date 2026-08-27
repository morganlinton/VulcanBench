"""Normalized reasoning-effort helpers for benchmark runs."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

EFFORT_LEVELS = frozenset({"minimal", "low", "medium", "high", "extra-high", "max"})
DEFAULT_SWEEP_EFFORTS = ("low", "medium", "high")

_OPENAI_EFFORT_VALUES = {
    "minimal": "minimal",
    "low": "low",
    "medium": "medium",
    "high": "high",
    "extra-high": "xhigh",
    "max": "max",
}

# Claude Code CLI `--effort`. Same reachable set as before `minimal` entered the
# vocabulary: the CLI has no minimal level, so it stays recorded-but-not-sent
# rather than being forwarded to a flag that would reject it.
_CLAUDE_CODE_EFFORT_VALUES = {
    "low": "low",
    "medium": "medium",
    "high": "high",
    "extra-high": "xhigh",
    "max": "max",
}

# Anthropic Messages API `output_config.effort`. `xhigh` is model-dependent
# (Opus 4.7+); unsupported combinations are rejected by the API with a clear
# error, mirroring how OpenAI handles model-dependent effort values.
_ANTHROPIC_EFFORT_VALUES = {
    "low": "low",
    "medium": "medium",
    "high": "high",
    "extra-high": "xhigh",
}

# Moonshot Kimi K3 `reasoning_effort`. Only "max" exists today (thinking is
# always on); levels the API doesn't accept yet are absent from this map and
# fall back to recorded-but-not-sent.
_KIMI_EFFORT_VALUES = {
    "extra-high": "max",
}

# DeepSeek `reasoning_effort` (V4 API). The documented enum is low/high/max
# (default high). "medium" does not exist, the API silently coerces it to
# "high", so medium is absent from this map and falls back to
# recorded-but-not-sent (the run then executes at the default, which is also
# high; the metadata honestly says supported=False rather than pretending a
# medium level ran).
_DEEPSEEK_EFFORT_VALUES = {
    "low": "low",
    "high": "high",
    "extra-high": "max",
}

# Qwen `reasoning_effort` (DashScope compatible-mode, Qwen3.8+). The documented
# enum is low/medium/xhigh with xhigh as the DEFAULT, there is no "high", so
# "high" is absent from this map and falls back to recorded-but-not-sent (the
# run then executes at the model default, which is xhigh; the metadata says
# supported=False rather than pretending a distinct high level ran).
_QWEN_EFFORT_VALUES = {
    "low": "low",
    "medium": "medium",
    "extra-high": "xhigh",
}

# Meta Model API `reasoning.effort` values for Muse Spark. The documented enum
# is minimal/low/medium/high/xhigh (dev.meta.ai/docs/reasoning.md); `none`
# returns HTTP 400 and is not part of VulcanBench's vocabulary anyway. An unset
# request reasons at "a model-determined level", Meta does not document which,
# so omitting --effort is not a known effort point.
_META_EFFORT_VALUES = {
    "minimal": "minimal",
    "low": "low",
    "medium": "medium",
    "high": "high",
    "extra-high": "xhigh",
}

# xAI `reasoning_effort` (Grok). Documented enum is low/medium/high/xhigh with
# high as the DEFAULT (reasoning cannot be disabled; an unset request runs at
# high). `xhigh` is Grok 4.6+, pre-4.6 models silently coerce it to high, so
# only sweep extra-high on 4.6+. No minimal/max/none levels exist.
_XAI_EFFORT_VALUES = {
    "low": "low",
    "medium": "medium",
    "high": "high",
    "extra-high": "xhigh",
}

# Cursor CLI bracket parameter (`model[effort=...]`). Cursor documents
# low/medium/high; anything else is recorded-but-not-sent so the bracket never
# carries a value Cursor might silently coerce.
_CURSOR_EFFORT_VALUES = {
    "low": "low",
    "medium": "medium",
    "high": "high",
}

# Grok Build CLI. The knob is --reasoning-effort (none/minimal/low/medium/
# high/xhigh); the adapter never uses the CLI's --effort flag, which parses
# but is silently ignored for reasoning (verified on grok 0.2.69: a session
# run with `--effort low` records reasoning_effort=high, the default).
# "none" is outside VulcanBench's vocabulary; "max" does not exist here.
_GROK_BUILD_EFFORT_VALUES = {
    "minimal": "minimal",
    "low": "low",
    "medium": "medium",
    "high": "high",
    "extra-high": "xhigh",
}

# Z.ai GLM `reasoning_effort`. Only GLM 5.3+ exposes the knob; its documented
# enum is low/high/max (default max, thinking always enabled, disabling is no
# longer supported). "medium" does not exist. Like DeepSeek's identical
# low/high/max enum, only extra-high reaches the API ceiling ("max"); medium and
# max fall back to recorded-but-not-sent rather than pretending a level the API
# lacks ran. Earlier GLMs (5, 5.1, 5.2) have no effort knob at all, see
# ``zai_supports_effort``: so every level stays recorded-but-not-sent for them.
_ZAI_EFFORT_VALUES = {
    "low": "low",
    "high": "high",
    "extra-high": "max",
}

# GLM model families that expose reasoning_effort, matched by prefix so point
# releases inherit support. Extend deliberately as new effort-capable GLMs ship.
_ZAI_EFFORT_MODELS = ("glm-5.3",)

# ZCode (Z.ai's coding harness) "thought level". The harness exposes the same
# low/high/max enum as the GLM 5.3 API (ZCode's model catalog lists exactly
# those three for glm-5.3, default max) and the adapter pins the level through
# the per-run project config's ``modelCatalog.overrides[...].reasoning.
# defaultLevel``, then reads the level ZCode actually ran back from its usage
# ledger (``model_usage.variant``) as ``reported_effort``. "medium" does not
# exist, so it stays recorded-but-not-sent.
_ZCODE_EFFORT_VALUES = {
    "low": "low",
    "high": "high",
    "extra-high": "max",
}


def zai_supports_effort(model: str | None) -> bool:
    """Whether a Z.ai GLM model exposes the ``reasoning_effort`` knob."""
    if not model:
        return False
    name = model.strip().lower()
    return any(name.startswith(prefix) for prefix in _ZAI_EFFORT_MODELS)


_PI_EFFORT_VALUES = {
    "minimal": "minimal",
    "low": "low",
    "medium": "medium",
    "high": "high",
    "extra-high": "xhigh",
}

_PROVIDER_EFFORT_MAPS = {
    "kimi": _KIMI_EFFORT_VALUES,
    "deepseek": _DEEPSEEK_EFFORT_VALUES,
    "qwen": _QWEN_EFFORT_VALUES,
    "meta": _META_EFFORT_VALUES,
    "xai": _XAI_EFFORT_VALUES,
    "cursor": _CURSOR_EFFORT_VALUES,
    "grok-build": _GROK_BUILD_EFFORT_VALUES,
    "zcode": _ZCODE_EFFORT_VALUES,
    "pi": _PI_EFFORT_VALUES,
}


class EffortNotSupportedError(ValueError):
    """Raised when a provider cannot run a requested effort level."""


@dataclass(frozen=True)
class EffortConfig:
    requested: str
    provider: str
    provider_value: str | None
    supported: bool

    def as_summary(self) -> dict[str, Any]:
        return {
            "requested": self.requested,
            "provider": self.provider,
            "provider_value": self.provider_value,
            "supported": self.supported,
        }


def normalize_effort(value: str | None) -> str | None:
    """Normalize a user-facing effort string, returning ``None`` when unset."""
    if value is None:
        return None
    normalized = value.strip().lower()
    if normalized not in EFFORT_LEVELS:
        allowed = ", ".join(sorted(EFFORT_LEVELS))
        raise ValueError(f"effort must be one of {allowed}, got {value!r}")
    return normalized


def parse_efforts(raw: str | None) -> list[str]:
    """Parse a comma-separated effort list for sweep runs."""
    if raw is None or not raw.strip():
        return list(DEFAULT_SWEEP_EFFORTS)
    values = [normalize_effort(part) for part in raw.split(",") if part.strip()]
    efforts = [v for v in values if v is not None]
    if not efforts:
        raise ValueError("at least one effort level is required")
    seen: set[str] = set()
    deduped: list[str] = []
    for effort in efforts:
        if effort not in seen:
            seen.add(effort)
            deduped.append(effort)
    return deduped


def effort_config(
    provider: str, requested: str | None, model: str | None = None
) -> EffortConfig | None:
    """Resolve benchmark effort metadata for a provider.

    ``model`` is consulted only where effort support is model-dependent within a
    provider (e.g. Z.ai, where only GLM 5.3+ exposes ``reasoning_effort``).
    """
    effort = normalize_effort(requested)
    if effort is None:
        return None

    provider_name = provider.strip().lower()
    if provider_name in {"openai", "codex"}:
        return EffortConfig(
            requested=effort,
            provider=provider_name,
            provider_value=_OPENAI_EFFORT_VALUES[effort],
            supported=True,
        )

    # Every remaining provider resolves its sent value through a lookup map (a
    # level absent from the map is recorded-but-not-sent). A ``None`` map means
    # the provider never sends effort at all. Each API's documented
    # reasoning_effort enum lives in its map above.
    effort_map: dict[str, str] | None
    if provider_name in _PROVIDER_EFFORT_MAPS:
        effort_map = _PROVIDER_EFFORT_MAPS[provider_name]
    elif provider_name == "claude-code":
        effort_map = _CLAUDE_CODE_EFFORT_VALUES
    elif provider_name == "anthropic":
        effort_map = _ANTHROPIC_EFFORT_VALUES
    elif provider_name == "zai":
        # GLM 5.3+ exposes reasoning_effort; earlier GLMs are effort-less.
        effort_map = _ZAI_EFFORT_VALUES if zai_supports_effort(model) else None
    elif provider_name in {"mock", "ollama", "openrouter"}:
        effort_map = None
    else:
        raise EffortNotSupportedError(
            f"reasoning effort is not supported for provider {provider!r}"
        )

    provider_value = effort_map.get(effort) if effort_map else None
    return EffortConfig(
        requested=effort,
        provider=provider_name,
        provider_value=provider_value,
        supported=provider_value is not None,
    )
