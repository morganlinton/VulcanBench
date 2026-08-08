"""Generic LLM provider interface for the VulcanBench agent loop.

The goal is to evaluate *any* model behind a uniform contract. A provider takes
a list of chat messages plus the OpenAI-style tool schemas and returns an
:class:`LLMResponse` (assistant text, requested tool calls, and token usage).

Providers implemented:

- ``mock:<name>``      Deterministic, offline. Solves ``hello-world`` and powers
                       tests without network or API keys.
- ``openai:<model>``   OpenAI Chat Completions API by default, and the Responses
                       API when reasoning effort is supplied.
- ``anthropic:<model>`` Anthropic Messages API.
- ``zai:<model>``      Z.ai (Zhipu) OpenAI-compatible Chat Completions API.
- ``kimi:<model>``     Moonshot AI (Kimi) OpenAI-compatible Chat Completions API.
- ``qwen:<model>``     Alibaba Cloud DashScope OpenAI-compatible Chat Completions
                       API (Qwen).
- ``deepseek:<model>`` DeepSeek OpenAI-compatible Chat Completions API.
- ``meta:<model>``     Meta Model API Responses endpoint (Muse Spark).

Only the Python standard library is used for HTTP so the harness stays
dependency-light; ``tenacity`` provides retry/backoff.

Usage::

    provider = get_provider("openai:gpt-4o")
    resp = provider.complete(messages, tools)
"""

from __future__ import annotations

import json
import os
import urllib.error
import urllib.request
from abc import ABC, abstractmethod
from collections.abc import Callable
from typing import Any

from pydantic import BaseModel, Field
from tenacity import Retrying, retry, retry_if_exception, stop_after_attempt, wait_exponential


class ProviderError(RuntimeError):
    """Raised when a provider cannot produce a response."""


class NonRetryableProviderError(ProviderError):
    """A ProviderError that must fail fast: retrying only re-bills the failure
    (e.g. a response truncated at the output ceiling)."""


class ToolInvocation(BaseModel):
    """A single tool call requested by the model."""

    id: str = ""
    name: str
    arguments: dict[str, Any] = Field(default_factory=dict)


class TokenUsage(BaseModel):
    prompt_tokens: int = 0
    completion_tokens: int = 0

    @property
    def total(self) -> int:
        return self.prompt_tokens + self.completion_tokens


class LLMResponse(BaseModel):
    """Uniform response shape across all providers."""

    content: str | None = None
    tool_calls: list[ToolInvocation] = Field(default_factory=list)
    usage: TokenUsage = Field(default_factory=TokenUsage)
    raw: dict[str, Any] = Field(default_factory=dict)

    @property
    def wants_tools(self) -> bool:
        return len(self.tool_calls) > 0


# Ceiling on a single provider HTTP call, independent of how much run budget is
# left. The remaining budget bounds the *run*, not one request: using it as the
# socket timeout let a stalled request consume the whole task.
#
# Calibrate this against the SLOWEST LEGITIMATE RESPONSE, not the average. Round
# trips grow with conversation length, so on a large repo they climb steadily as
# context accumulates -- one Opus 5 run on sqlglot went 2s, 4s, 43s, 111s, 153s,
# 190s over its life. Aggregating every request hides that: across all tasks the
# median is 3.3s and p99 is 87s, but per task the maxima split sharply by repo
# size -- 30s on the small repos versus 333s on large/xlarge ones. A 300s ceiling
# derived from the pooled distribution was below a response that had legitimately
# completed in 333s, and killed real work.
#
# Observed stalls, by contrast, ran 962-1785s and never returned. 600s sits ~1.8x
# above the slowest real response and ~1.6x below the shortest stall.
DEFAULT_MAX_REQUEST_TIMEOUT_S = 600


class LLMProvider(ABC):
    """Base class for all model backends."""

    #: Ceiling on any single HTTP call to this provider, in seconds. Raise it in
    #: a subclass when the model's thinking legitimately runs longer.
    MAX_REQUEST_TIMEOUT_S: float = DEFAULT_MAX_REQUEST_TIMEOUT_S

    def __init__(self, model: str) -> None:
        self.model = model

    @property
    def spec(self) -> str:
        return f"{self.name}:{self.model}"

    @property
    @abstractmethod
    def name(self) -> str:
        """Short provider id, e.g. ``openai``."""

    @abstractmethod
    def complete(
        self,
        messages: list[dict[str, Any]],
        tools: list[dict[str, Any]],
        timeout_s: float | None = None,
        effort: str | None = None,
    ) -> LLMResponse:
        """Return the next assistant turn given the conversation and tool schemas."""

    @property
    def supports_streaming(self) -> bool:
        """Whether this provider implements token streaming (v1: always False)."""
        return False


def _http_post_json(
    url: str, headers: dict[str, str], payload: dict[str, Any], timeout: float = 120
) -> dict[str, Any]:
    """POST JSON and parse the JSON response using only the stdlib."""
    data = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(url, data=data, headers=headers, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            parsed: dict[str, Any] = json.loads(resp.read().decode("utf-8"))
            return parsed
    except urllib.error.HTTPError as e:
        body = e.read().decode("utf-8", errors="replace")
        raise ProviderError(f"HTTP {e.code} from {url}: {body[:500]}") from e
    except urllib.error.URLError as e:
        raise ProviderError(f"network error calling {url}: {e.reason}") from e
    except TimeoutError as e:
        # socket timeout during resp.read() surfaces raw (not as URLError);
        # wrap it so the retry wrapper treats it like any other transient error.
        raise ProviderError(f"read timeout after {timeout:.0f}s calling {url}") from e


def _http_timeout(timeout_s: float | None, cap: float = DEFAULT_MAX_REQUEST_TIMEOUT_S) -> float:
    """Per-request socket timeout: the request ceiling, capped by run budget.

    ``timeout_s`` is the run's *remaining* budget. It caps the request so a call
    can't outlive the run, but never extends it past ``cap``. Providers whose
    models legitimately think for longer raise ``cap`` via
    ``LLMProvider.MAX_REQUEST_TIMEOUT_S``.
    """
    if timeout_s is None:
        return cap
    if timeout_s <= 0:
        raise ProviderError("run budget exhausted before provider call")
    return min(cap, timeout_s)


_MAX_ATTEMPTS = 4
_WAIT = wait_exponential(multiplier=1, min=1, max=20)


def _is_retryable(e: BaseException) -> bool:
    return isinstance(e, ProviderError) and not isinstance(e, NonRetryableProviderError)


def _budgeted_attempts(timeout_s: float | None, request_timeout: float) -> int:
    """How many attempts fit in the remaining run budget.

    A retry is only worth making if the run can afford another request. With no
    budget supplied, use the full allowance.
    """
    if timeout_s is None:
        return _MAX_ATTEMPTS
    if request_timeout <= 0:
        return 1
    return max(1, min(_MAX_ATTEMPTS, int(timeout_s // request_timeout)))


def _call_with_retry[T](fn: Callable[..., T], attempts: int, *args: Any) -> T:
    """Invoke ``fn`` with ProviderError retries, bounded to ``attempts``.

    Transient stalls are the common failure: retrying a timed-out request
    usually succeeds, whereas before a single stall consumed the whole run.
    NonRetryableProviderError (refusals, output-cap truncation) is surfaced
    immediately — a retry at the same settings cannot change the outcome.
    """
    retryer = Retrying(
        retry=retry_if_exception(_is_retryable),
        wait=_WAIT,
        stop=stop_after_attempt(attempts),
        reraise=True,
    )
    result: T = retryer(fn, *args)
    return result


_RETRY = retry(
    retry=retry_if_exception(_is_retryable),
    wait=_WAIT,
    stop=stop_after_attempt(4),
    reraise=True,
)

# Anthropic output ceiling by requested effort. Thinking bills against
# ``max_tokens`` on always-on-thinking models (Fable 5) and adaptive models
# (Sonnet 5), so the ceiling must scale with effort — the old fixed 16K cap
# truncated thinking-heavy steps to empty responses that were then scored as
# wrong answers (see Report No. 10's harness note).
_ANTHROPIC_MAX_TOKENS: dict[str, int] = {
    "low": 32_000,
    "medium": 64_000,
    "high": 128_000,
    "xhigh": 128_000,
    "extra-high": 128_000,
    "max": 128_000,
}
_ANTHROPIC_MAX_TOKENS_DEFAULT = 32_000


class OpenAIProvider(LLMProvider):
    """OpenAI provider.

    Uses Chat Completions for the legacy/no-effort path. When ``effort`` is
    supplied, switches to the Responses API so reasoning effort can be passed in
    the official ``reasoning.effort`` field.
    """

    @property
    def name(self) -> str:
        return "openai"

    def complete(
        self,
        messages: list[dict[str, Any]],
        tools: list[dict[str, Any]],
        timeout_s: float | None = None,
        effort: str | None = None,
    ) -> LLMResponse:
        timeout = _http_timeout(timeout_s, self.MAX_REQUEST_TIMEOUT_S)
        attempts = _budgeted_attempts(timeout_s, timeout)
        return _call_with_retry(self._complete_once, attempts, messages, tools, timeout, effort)

    def _complete_once(
        self,
        messages: list[dict[str, Any]],
        tools: list[dict[str, Any]],
        timeout: float,
        effort: str | None,
    ) -> LLMResponse:
        api_key = os.environ.get("OPENAI_API_KEY")
        if not api_key:
            raise ProviderError("OPENAI_API_KEY is not set")
        base = os.environ.get("OPENAI_BASE_URL", "https://api.openai.com/v1").rstrip("/")
        if effort is not None:
            return self._responses_complete(base, api_key, messages, tools, timeout, effort)

        return _chat_completions_complete(
            base,
            api_key,
            self.model,
            messages,
            tools,
            timeout,
            temperature=None if _openai_omits_chat_sampling(self.model) else 0,
        )

    def _responses_complete(
        self,
        base: str,
        api_key: str,
        messages: list[dict[str, Any]],
        tools: list[dict[str, Any]],
        timeout: float,
        effort: str,
    ) -> LLMResponse:
        payload: dict[str, Any] = {
            "model": self.model,
            "input": _to_responses_input(messages),
            "reasoning": {"effort": effort},
        }
        if tools:
            payload["tools"] = [_openai_tool_to_responses(t) for t in tools]
            payload["tool_choice"] = "auto"
        body = _http_post_json(
            f"{base}/responses",
            {"Authorization": f"Bearer {api_key}", "Content-Type": "application/json"},
            payload,
            timeout=timeout,
        )
        return _parse_responses_body(body)


class MetaProvider(LLMProvider):
    """Meta Model API provider for Muse Spark.

    Uses Meta's OpenAI-compatible Responses API directly.  The model request is
    made by the host-side harness while tool calls still execute through the
    selected VulcanBench executor (Docker by default), so this path does not
    depend on Muse Code being able to authenticate from inside a container.
    """

    @property
    def name(self) -> str:
        return "meta"

    def complete(
        self,
        messages: list[dict[str, Any]],
        tools: list[dict[str, Any]],
        timeout_s: float | None = None,
        effort: str | None = None,
    ) -> LLMResponse:
        timeout = _http_timeout(timeout_s, self.MAX_REQUEST_TIMEOUT_S)
        attempts = _budgeted_attempts(timeout_s, timeout)
        return _call_with_retry(self._complete_once, attempts, messages, tools, timeout, effort)

    def _complete_once(
        self,
        messages: list[dict[str, Any]],
        tools: list[dict[str, Any]],
        timeout: float,
        effort: str | None,
    ) -> LLMResponse:
        api_key = os.environ.get("MODEL_API_KEY")
        if not api_key:
            raise ProviderError("MODEL_API_KEY is not set")
        base = os.environ.get("META_BASE_URL", "https://api.meta.ai/v1").rstrip("/")
        payload: dict[str, Any] = {
            "model": self.model,
            "input": _to_responses_input(messages),
        }
        if effort is not None:
            payload["reasoning"] = {"effort": effort}
        if tools:
            payload["tools"] = [_openai_tool_to_responses(t) for t in tools]
            payload["tool_choice"] = "auto"
        body = _http_post_json(
            f"{base}/responses",
            {"Authorization": f"Bearer {api_key}", "Content-Type": "application/json"},
            payload,
            timeout=timeout,
        )
        # Meta's standard cached-input rate is 0.15/1.25 = 0.12 of uncached
        # input.  The Contributor model is 0.002/0.10 = 0.02.
        cached_factor = 0.02 if self.model.endswith("-contributor") else 0.12
        return _parse_responses_body(body, cached_input_factor=cached_factor)


_CACHE_CONTROL = {"type": "ephemeral"}


def _with_prompt_caching(
    system: str, converted: list[dict[str, Any]]
) -> tuple[str | list[dict[str, Any]], list[dict[str, Any]]]:
    """Add prompt-cache breakpoints so the agent loop's re-sent transcript bills at
    cache-read rates. Two ephemeral breakpoints: (1) tools+system (the stable prefix),
    and (2) the tail of the last message (the growing conversation, cached
    incrementally on each turn). No-ops safely on empty inputs. Prompt caching is GA
    (no beta header)."""
    system_field: str | list[dict[str, Any]] = system
    if system:
        system_field = [{"type": "text", "text": system, "cache_control": _CACHE_CONTROL}]
    if converted:
        last = dict(converted[-1])
        content = last.get("content")
        if isinstance(content, str):
            last["content"] = [{"type": "text", "text": content, "cache_control": _CACHE_CONTROL}]
        elif isinstance(content, list) and content:
            blocks = list(content)
            blocks[-1] = {**blocks[-1], "cache_control": _CACHE_CONTROL}
            last["content"] = blocks
        converted = [*converted[:-1], last]
    return system_field, converted


class AnthropicProvider(LLMProvider):
    """Anthropic Messages API.

    Reasoning effort maps to the API's ``output_config.effort`` field. No
    sampling parameters are sent: ``temperature``/``top_p``/``top_k`` are
    rejected with a 400 by Opus 4.7 and newer models. Prompt caching is applied to
    the tools+system prefix and the growing transcript so re-sent context in the
    agent loop bills at cache-read rates.
    """

    @property
    def name(self) -> str:
        return "anthropic"

    def complete(
        self,
        messages: list[dict[str, Any]],
        tools: list[dict[str, Any]],
        timeout_s: float | None = None,
        effort: str | None = None,
    ) -> LLMResponse:
        timeout = _http_timeout(timeout_s, self.MAX_REQUEST_TIMEOUT_S)
        attempts = _budgeted_attempts(timeout_s, timeout)
        return _call_with_retry(self._complete_once, attempts, messages, tools, timeout, effort)

    def _complete_once(
        self,
        messages: list[dict[str, Any]],
        tools: list[dict[str, Any]],
        timeout: float,
        effort: str | None,
    ) -> LLMResponse:
        api_key = os.environ.get("ANTHROPIC_API_KEY")
        if not api_key:
            raise ProviderError("ANTHROPIC_API_KEY is not set")
        base = os.environ.get("ANTHROPIC_BASE_URL", "https://api.anthropic.com").rstrip("/")
        system, converted = _to_anthropic_messages(messages)
        system_field, converted = _with_prompt_caching(system, converted)
        max_tokens = _ANTHROPIC_MAX_TOKENS.get(effort or "", _ANTHROPIC_MAX_TOKENS_DEFAULT)
        payload: dict[str, Any] = {
            "model": self.model,
            "max_tokens": max_tokens,
            "messages": converted,
        }
        if effort is not None:
            payload["output_config"] = {"effort": effort}
        if system_field:
            payload["system"] = system_field
        if tools:
            payload["tools"] = [_openai_tool_to_anthropic(t) for t in tools]
        headers = {
            "x-api-key": api_key,
            "anthropic-version": "2023-06-01",
            "Content-Type": "application/json",
        }
        # Opt-in server-side refusal fallback (e.g. Fable 5 -> Opus 4.8): a
        # request declined by the safety classifiers is re-served by the
        # fallback model inside the same call, with repricing applied by the
        # API. The response's ``model`` field reports which model answered.
        fallback = os.environ.get("VULCANBENCH_REFUSAL_FALLBACK")
        if fallback:
            headers["anthropic-beta"] = "server-side-fallback-2026-06-01"
            payload["fallbacks"] = [{"model": fallback}]
        body = _http_post_json(
            f"{base}/v1/messages",
            headers,
            payload,
            timeout=timeout,
        )
        served = body.get("model")
        if fallback and served and not str(served).startswith(self.model):
            print(f"[vulcanbench] refusal fallback: step served by {served}", flush=True)
        if body.get("stop_reason") == "refusal":
            details = body.get("stop_details") or {}
            raise ProviderError(
                "model refused the request"
                f" (category={details.get('category')!r}):"
                f" {details.get('explanation') or 'no explanation provided'}"
            )
        content_text: list[str] = []
        tool_calls: list[ToolInvocation] = []
        for block in body.get("content", []):
            if block.get("type") == "text":
                content_text.append(block.get("text", ""))
            elif block.get("type") == "tool_use":
                tool_calls.append(
                    ToolInvocation(
                        id=block.get("id", ""),
                        name=block.get("name", ""),
                        arguments=block.get("input", {}) or {},
                    )
                )
        if body.get("stop_reason") == "max_tokens" and not tool_calls:
            # Thinking consumed the whole output budget: the step carries no
            # action, and silently returning it would score the run as a wrong
            # answer instead of a harness-visible failure.
            raise NonRetryableProviderError(
                f"response truncated at max_tokens={max_tokens} with no tool call"
                " (stop_reason=max_tokens)"
            )
        usage = body.get("usage", {})
        # With prompt caching, ``input_tokens`` is the UNCACHED remainder; cache reads
        # bill ~0.1x and cache writes ~1.25x (separate usage fields). Fold them into an
        # effective prompt-token count so the existing per-token cost stays correct.
        # Raw usage (incl. cache_read_input_tokens / cache_creation_input_tokens) is
        # preserved in ``.raw`` for exact throughput accounting.
        uncached = usage.get("input_tokens", 0)
        cache_read = usage.get("cache_read_input_tokens", 0)
        cache_write = usage.get("cache_creation_input_tokens", 0)
        effective_prompt = round(uncached + cache_read * 0.1 + cache_write * 1.25)
        return LLMResponse(
            content="\n".join(content_text) or None,
            tool_calls=tool_calls,
            usage=TokenUsage(
                prompt_tokens=effective_prompt,
                completion_tokens=usage.get("output_tokens", 0),
            ),
            raw=body,
        )


class MockProvider(LLMProvider):
    """Deterministic, offline provider for tests and demos.

    It runs a tiny scripted policy: read the issue, then create the file the
    ``hello-world`` task asks for, then signal completion. This lets the *real*
    agent loop be exercised end-to-end with no network or API key.
    """

    @property
    def name(self) -> str:
        return "mock"

    def complete(
        self,
        messages: list[dict[str, Any]],
        tools: list[dict[str, Any]],
        timeout_s: float | None = None,
        effort: str | None = None,
    ) -> LLMResponse:
        del timeout_s, effort
        # Judge requests (human_like ensemble) carry a sentinel: answer with a
        # fixed JSON score so the metric is deterministic offline.
        if any("VULCANBENCH_JUDGE" in str(m.get("content", "")) for m in messages):
            return LLMResponse(
                content='{"score": 80, "rationale": "mock judge"}',
                usage=TokenUsage(prompt_tokens=120, completion_tokens=15),
            )
        # Agentic-grader requests carry their own sentinel: approve any candidate
        # that actually changed something (a real diff), so offline grading is
        # deterministic — gold passes, an empty pre-patch state fails.
        if any("VULCANBENCH_GRADER" in str(m.get("content", "")) for m in messages):
            has_change = any("diff --git" in str(m.get("content", "")) for m in messages)
            verdict = "true" if has_change else "false"
            # Emit both the binary (acceptance_criteria) and rubric verdict shapes so the
            # same mock deterministically grades either grader offline: gold (a real diff)
            # passes every criterion, an empty pre-patch change fails them.
            arr = "[" + ", ".join([verdict] * 16) + "]"
            return LLMResponse(
                content=(
                    f'{{"correct": {verdict}, "confidence": 0.9, "reasons": "mock grader", '
                    f'"blocking": {arr}, "weighted": {arr}, "notes": "mock grader"}}'
                ),
                usage=TokenUsage(prompt_tokens=140, completion_tokens=18),
            )
        # Decide the next action purely from what has happened so far.
        called = [m for m in messages if m.get("role") == "tool"]
        steps = len(called)
        usage = TokenUsage(prompt_tokens=50 + steps * 20, completion_tokens=20)
        if steps == 0:
            return LLMResponse(
                content="Reading the issue.",
                tool_calls=[
                    ToolInvocation(id="c1", name="read_file", arguments={"path": "issue.md"})
                ],
                usage=usage,
            )
        if steps == 1:
            return LLMResponse(
                content="Creating hello.py with the required output.",
                tool_calls=[
                    ToolInvocation(
                        id="c2",
                        name="edit_file",
                        arguments={
                            "path": "hello.py",
                            "old_string": "",
                            "new_string": 'print("hello from vulcanbench")\n',
                        },
                    )
                ],
                usage=usage,
            )
        if steps == 2:
            return LLMResponse(
                content="Running tests.",
                tool_calls=[ToolInvocation(id="c3", name="run_tests", arguments={})],
                usage=usage,
            )
        return LLMResponse(content="FINISH: implemented and verified.", usage=usage)


class ZaiProvider(LLMProvider):
    """Z.ai (Zhipu) OpenAI-compatible Chat Completions API.

    Uses ``/chat/completions`` only. Reasoning effort is not supported; pass
    ``--effort`` for metadata recording but it is ignored at the API layer.
    """

    @property
    def name(self) -> str:
        return "zai"

    def complete(
        self,
        messages: list[dict[str, Any]],
        tools: list[dict[str, Any]],
        timeout_s: float | None = None,
        effort: str | None = None,
    ) -> LLMResponse:
        del effort
        timeout = _http_timeout(timeout_s, self.MAX_REQUEST_TIMEOUT_S)
        attempts = _budgeted_attempts(timeout_s, timeout)
        return _call_with_retry(self._complete_once, attempts, messages, tools, timeout)

    def _complete_once(
        self,
        messages: list[dict[str, Any]],
        tools: list[dict[str, Any]],
        timeout: float,
    ) -> LLMResponse:
        api_key = os.environ.get("ZAI_API_KEY")
        if not api_key:
            raise ProviderError("ZAI_API_KEY is not set")
        base = os.environ.get("ZAI_BASE_URL", "https://api.z.ai/api/paas/v4").rstrip("/")
        return _chat_completions_complete(base, api_key, self.model, messages, tools, timeout)


class KimiProvider(LLMProvider):
    """Moonshot AI (Kimi) OpenAI-compatible Chat Completions API.

    Uses ``/chat/completions`` only. ``kimi-k3`` rejects sampling params, so no
    ``temperature`` is sent. Thinking is always on for K3; ``effort`` (when the
    harness resolves it as supported, e.g. ``extra-high`` -> ``"max"``) is sent
    as ``reasoning_effort``.
    """

    @property
    def name(self) -> str:
        return "kimi"

    # K3's always-on max thinking routinely exceeds the generic per-request
    # ceiling, so this provider gets a roomier one (used both for deadline-less
    # calls such as judge/grader invocations and as the cap on budgeted calls).
    MAX_REQUEST_TIMEOUT_S = 900.0

    def complete(
        self,
        messages: list[dict[str, Any]],
        tools: list[dict[str, Any]],
        timeout_s: float | None = None,
        effort: str | None = None,
    ) -> LLMResponse:
        timeout = _http_timeout(timeout_s, self.MAX_REQUEST_TIMEOUT_S)
        attempts = _budgeted_attempts(timeout_s, timeout)
        return _call_with_retry(self._complete_once, attempts, messages, tools, timeout, effort)

    def _complete_once(
        self,
        messages: list[dict[str, Any]],
        tools: list[dict[str, Any]],
        timeout: float,
        effort: str | None,
    ) -> LLMResponse:
        api_key = os.environ.get("MOONSHOT_API_KEY")
        if not api_key:
            raise ProviderError("MOONSHOT_API_KEY is not set")
        base = os.environ.get("MOONSHOT_BASE_URL", "https://api.moonshot.ai/v1").rstrip("/")
        return _chat_completions_complete(
            base,
            api_key,
            self.model,
            messages,
            tools,
            timeout,
            temperature=None,
            extra_payload={"reasoning_effort": effort} if effort else None,
        )


class QwenProvider(LLMProvider):
    """Alibaba Cloud DashScope (Qwen) OpenAI-compatible Chat Completions API.

    Uses ``/compatible-mode/v1/chat/completions``. Default base URL is the
    international endpoint; set ``DASHSCOPE_BASE_URL`` for China
    (``https://dashscope.aliyuncs.com/compatible-mode/v1``) or another region.
    Effort maps to the API's ``reasoning_effort`` field, supported on
    non-streaming calls since the Qwen3.8 series. The documented enum is
    low/medium/xhigh (default xhigh) — there is no ``high``, so ``--effort
    high`` is recorded as metadata only and the run executes at the model
    default. Pre-3.8 models may ignore the field.
    """

    @property
    def name(self) -> str:
        return "qwen"

    def complete(
        self,
        messages: list[dict[str, Any]],
        tools: list[dict[str, Any]],
        timeout_s: float | None = None,
        effort: str | None = None,
    ) -> LLMResponse:
        timeout = _http_timeout(timeout_s)
        if timeout_s is not None:
            return self._complete_once(messages, tools, timeout, effort)
        return self._complete_with_retry(messages, tools, timeout, effort)

    @_RETRY
    def _complete_with_retry(
        self,
        messages: list[dict[str, Any]],
        tools: list[dict[str, Any]],
        timeout: float,
        effort: str | None,
    ) -> LLMResponse:
        return self._complete_once(messages, tools, timeout, effort)

    def _complete_once(
        self,
        messages: list[dict[str, Any]],
        tools: list[dict[str, Any]],
        timeout: float,
        effort: str | None,
    ) -> LLMResponse:
        api_key = os.environ.get("DASHSCOPE_API_KEY")
        if not api_key:
            raise ProviderError("DASHSCOPE_API_KEY is not set")
        base = os.environ.get(
            "DASHSCOPE_BASE_URL",
            "https://dashscope-intl.aliyuncs.com/compatible-mode/v1",
        ).rstrip("/")
        return _chat_completions_complete(
            base,
            api_key,
            self.model,
            messages,
            tools,
            timeout,
            extra_payload={"reasoning_effort": effort} if effort else None,
        )


class DeepSeekProvider(LLMProvider):
    """DeepSeek OpenAI-compatible Chat Completions API.

    Uses ``/chat/completions`` on ``https://api.deepseek.com`` (override with
    ``DEEPSEEK_BASE_URL``). ``low``/``high`` effort maps straight to the API's
    ``reasoning_effort`` field and ``extra-high`` maps to ``max`` (DeepSeek's
    documented enum is low/high/max, default high). ``medium`` is recorded as
    metadata only — DeepSeek has no medium level and silently coerces it to
    ``high``, so the harness never sends it.
    """

    @property
    def name(self) -> str:
        return "deepseek"

    def complete(
        self,
        messages: list[dict[str, Any]],
        tools: list[dict[str, Any]],
        timeout_s: float | None = None,
        effort: str | None = None,
    ) -> LLMResponse:
        timeout = _http_timeout(timeout_s)
        if timeout_s is not None:
            return self._complete_once(messages, tools, timeout, effort)
        return self._complete_with_retry(messages, tools, timeout, effort)

    @_RETRY
    def _complete_with_retry(
        self,
        messages: list[dict[str, Any]],
        tools: list[dict[str, Any]],
        timeout: float,
        effort: str | None,
    ) -> LLMResponse:
        return self._complete_once(messages, tools, timeout, effort)

    def _complete_once(
        self,
        messages: list[dict[str, Any]],
        tools: list[dict[str, Any]],
        timeout: float,
        effort: str | None,
    ) -> LLMResponse:
        api_key = os.environ.get("DEEPSEEK_API_KEY")
        if not api_key:
            raise ProviderError("DEEPSEEK_API_KEY is not set")
        base = os.environ.get("DEEPSEEK_BASE_URL", "https://api.deepseek.com").rstrip("/")
        return _chat_completions_complete(
            base,
            api_key,
            self.model,
            messages,
            tools,
            timeout,
            extra_payload={"reasoning_effort": effort} if effort else None,
        )


def _loads_args(raw: Any) -> dict[str, Any]:
    if isinstance(raw, dict):
        return raw
    try:
        parsed = json.loads(raw or "{}")
        return parsed if isinstance(parsed, dict) else {}
    except (json.JSONDecodeError, TypeError):
        return {}


def _to_anthropic_messages(
    messages: list[dict[str, Any]],
) -> tuple[str, list[dict[str, Any]]]:
    """Split out the system prompt and convert OpenAI-style turns to Anthropic.

    Tool results (OpenAI ``role: tool``) become Anthropic ``tool_result`` blocks
    on a ``user`` turn keyed by ``tool_use_id``.
    """
    system_parts: list[str] = []
    out: list[dict[str, Any]] = []
    for m in messages:
        role = m.get("role")
        if role == "system":
            system_parts.append(str(m.get("content", "")))
        elif role == "tool":
            out.append(
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": m.get("tool_call_id", ""),
                            "content": str(m.get("content", "")),
                        }
                    ],
                }
            )
        elif role == "assistant" and m.get("tool_calls"):
            blocks: list[dict[str, Any]] = []
            if m.get("content"):
                blocks.append({"type": "text", "text": str(m["content"])})
            for tc in m["tool_calls"]:
                blocks.append(
                    {
                        "type": "tool_use",
                        "id": tc.get("id", ""),
                        "name": tc["function"]["name"],
                        "input": _loads_args(tc["function"].get("arguments", "{}")),
                    }
                )
            out.append({"role": "assistant", "content": blocks})
        else:
            out.append({"role": role or "user", "content": str(m.get("content", ""))})
    return "\n".join(system_parts), out


def _to_responses_input(messages: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Convert the harness' chat-style transcript to Responses API input items."""
    out: list[dict[str, Any]] = []
    for m in messages:
        role = m.get("role")
        content = str(m.get("content", ""))
        if role == "system":
            out.append({"role": "developer", "content": content})
        elif role in {"user", "assistant"}:
            if content:
                out.append({"role": role, "content": content})
            for tc in m.get("tool_calls") or []:
                fn = tc.get("function", {})
                out.append(
                    {
                        "type": "function_call",
                        "call_id": tc.get("id", ""),
                        "name": fn.get("name", ""),
                        "arguments": fn.get("arguments", "{}"),
                    }
                )
        elif role == "tool":
            out.append(
                {
                    "type": "function_call_output",
                    "call_id": m.get("tool_call_id", ""),
                    "output": content,
                }
            )
        else:
            out.append({"role": "user", "content": content})
    return out


def _openai_tool_to_responses(tool: dict[str, Any]) -> dict[str, Any]:
    fn = tool["function"]
    return {
        "type": "function",
        "name": fn["name"],
        "description": fn.get("description", ""),
        "parameters": fn.get("parameters", {"type": "object", "properties": {}}),
    }


def _parse_responses_body(body: dict[str, Any], cached_input_factor: float = 0.1) -> LLMResponse:
    content_parts: list[str] = []
    tool_calls: list[ToolInvocation] = []
    for item in body.get("output") or []:
        item_type = item.get("type")
        if item_type == "message":
            for block in item.get("content") or []:
                if block.get("type") in {"output_text", "text"}:
                    content_parts.append(block.get("text", ""))
        elif item_type == "function_call":
            tool_calls.append(
                ToolInvocation(
                    id=item.get("call_id") or item.get("id", ""),
                    name=item.get("name", ""),
                    arguments=_loads_args(item.get("arguments", "{}")),
                )
            )
    if body.get("output_text"):
        content_parts.append(str(body["output_text"]))

    usage = body.get("usage", {})
    return LLMResponse(
        content="\n".join(part for part in content_parts if part) or None,
        tool_calls=tool_calls,
        usage=TokenUsage(
            prompt_tokens=_openai_effective_prompt_tokens(
                usage.get("input_tokens", usage.get("prompt_tokens", 0)),
                (usage.get("input_tokens_details") or {}).get("cached_tokens", 0),
                cached_input_factor,
            ),
            completion_tokens=usage.get("output_tokens", usage.get("completion_tokens", 0)),
        ),
        raw=body,
    )


def _openai_effective_prompt_tokens(
    input_tokens: int, cached_tokens: int, cached_input_factor: float = 0.1
) -> int:
    """Fold OpenAI's automatic prompt-cache discount into an effective prompt count.

    Unlike Anthropic (where ``input_tokens`` is the uncached remainder), OpenAI reports
    ``input_tokens`` as the FULL prompt and carries the cached portion in
    ``input_tokens_details.cached_tokens``. Cached input on the GPT-5 series bills at
    ~0.1x the input rate, so the re-sent transcript in a long agent loop is far cheaper
    than the raw token count implies. Fold cache reads at 0.1x so ``cost_usd`` reflects
    what OpenAI actually bills.
    """
    cached = min(max(cached_tokens, 0), input_tokens)
    uncached = input_tokens - cached
    return round(uncached + cached * cached_input_factor)


def _parse_chat_completions_response(body: dict[str, Any]) -> LLMResponse:
    choice = (body.get("choices") or [{}])[0]
    msg = choice.get("message", {})
    tool_calls = [
        ToolInvocation(
            id=tc.get("id", ""),
            name=tc["function"]["name"],
            arguments=_loads_args(tc["function"].get("arguments", "{}")),
        )
        for tc in (msg.get("tool_calls") or [])
    ]
    usage = body.get("usage", {})
    return LLMResponse(
        content=msg.get("content"),
        tool_calls=tool_calls,
        usage=TokenUsage(
            prompt_tokens=_openai_effective_prompt_tokens(
                usage.get("prompt_tokens", 0),
                (usage.get("prompt_tokens_details") or {}).get("cached_tokens", 0),
            ),
            completion_tokens=usage.get("completion_tokens", 0),
        ),
        raw=body,
    )


def _openai_omits_chat_sampling(model: str) -> bool:
    """True when Chat Completions rejects non-default ``temperature`` (GPT-5, o-series)."""
    name = model.strip().lower()
    return name.startswith(("gpt-5", "o1", "o2", "o3", "o4"))


def _chat_completions_complete(
    base: str,
    api_key: str,
    model: str,
    messages: list[dict[str, Any]],
    tools: list[dict[str, Any]],
    timeout: float,
    temperature: float | None = 0,
    extra_payload: dict[str, Any] | None = None,
) -> LLMResponse:
    payload: dict[str, Any] = {
        "model": model,
        "messages": messages,
    }
    if temperature is not None:
        payload["temperature"] = temperature
    if extra_payload:
        payload.update(extra_payload)
    if tools:
        payload["tools"] = tools
        payload["tool_choice"] = "auto"
    body = _http_post_json(
        f"{base}/chat/completions",
        {"Authorization": f"Bearer {api_key}", "Content-Type": "application/json"},
        payload,
        timeout=timeout,
    )
    return _parse_chat_completions_response(body)


def _openai_tool_to_anthropic(tool: dict[str, Any]) -> dict[str, Any]:
    fn = tool["function"]
    return {
        "name": fn["name"],
        "description": fn.get("description", ""),
        "input_schema": fn.get("parameters", {"type": "object", "properties": {}}),
    }


_PROVIDERS: dict[str, type[LLMProvider]] = {
    "openai": OpenAIProvider,
    "anthropic": AnthropicProvider,
    "zai": ZaiProvider,
    "kimi": KimiProvider,
    "qwen": QwenProvider,
    "deepseek": DeepSeekProvider,
    "meta": MetaProvider,
    "mock": MockProvider,
}


def parse_model_spec(spec: str) -> tuple[str, str]:
    """Split ``provider:model`` into its parts.

    >>> parse_model_spec("openai:gpt-4o")
    ('openai', 'gpt-4o')
    """
    if ":" not in spec:
        raise ValueError(f"model spec must be 'provider:model', got {spec!r}")
    provider, _, model = spec.partition(":")
    provider = provider.strip().lower()
    model = model.strip()
    if not provider or not model:
        raise ValueError(f"model spec must be 'provider:model', got {spec!r}")
    return provider, model


def get_provider(spec: str) -> LLMProvider:
    """Construct a provider from a ``provider:model`` spec."""
    provider, model = parse_model_spec(spec)
    if provider == "claude-code":
        # Late import (circular otherwise: cli_agents imports from this
        # module). This provider is single-shot (judges/graders); full task
        # runs go through ``harness.agent.cli_agents.run_claude_code_task``.
        from harness.agent.cli_agents import ClaudeCodeProvider  # noqa: PLC0415

        return ClaudeCodeProvider(model)
    if provider not in _PROVIDERS:
        known = ", ".join(sorted([*_PROVIDERS, "claude-code"]))
        raise ValueError(f"unknown provider {provider!r}; known: {known}")
    return _PROVIDERS[provider](model)
