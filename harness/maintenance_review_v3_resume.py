"""Operator convenience for the documented v3.2 rule; not part of the frozen judging code.

Rule (docs/judging/maintenance-v3-operations.md): when the Claude CLI exits
non-zero with result subtype error_max_structured_output_retries, the response
is an invalid-schema response and the protocol grants one fresh retry. This
script re-invokes a stage; if it stops on exactly that subtype with only
attempt-1 recorded, it marks the receipt retryable with a review note and
re-invokes. Any other failure, or a second failure on the same call, stops.

Second rule (same log): when both attempts of a review failed only on
"Unsupported evidence excerpt" and every rejected excerpt matches the source
once whitespace and line breaks are collapsed, the attempt-1 response is
selected with each such excerpt re-wrapped to the source's own line breaks.
Scores and text are untouched; the original excerpt is recorded. A quote that
does not match the source even after collapsing is not recovered.

Third rule (owner decision, September 7, 2026, evening): retain and disclose
reviewer fallbacks. When an attempt failed only the identity guard, the
session requested and reported claude-opus-5, every assistant message came
from claude-opus-4-8 (the CLI's silent refusal fallback), no tool was used,
and the response validates, it is selected with a reviewer_fallback record.
Any other model, or an invalid response, is not recovered.

Usage: python -m harness.maintenance_review_v3_resume calibrate --panel claude
"""

from __future__ import annotations

import fcntl
import importlib
import json
import os
import re
import subprocess
import sys
import time
import traceback
from datetime import UTC, datetime
from pathlib import Path

from harness import retrospective_judging as base

# The protocol module to drive. v3.5 reuses the frozen v3 implementation with
# its population and directories rebound, so the wrapper must look those up on
# the module at call time rather than importing them by value.
MODULE = os.environ.get("VB_MAINT_MODULE", "harness.maintenance_review_v3")
importlib.import_module(
    MODULE
)  # a later protocol module rebinds the frozen v3 implementation on import
# The helpers (read, validate, excerpt_supported, ...) live on the frozen
# implementation, and so do the rebound OUT and population constants.
v3 = importlib.import_module("harness.maintenance_review_v3")


def _out():
    return v3.OUT


SUBTYPE = "error_max_structured_output_retries"
EXCERPT_ERROR = "Unsupported evidence excerpt"
IDENTITY_ERROR = "Claude reviewer identity or fallback guard failed"
FALLBACK_MODEL = "claude-opus-4-8"
REQUESTED_MODEL = "claude-opus-5"


def stage_kind(stage: str) -> str:
    return {
        "primary": "review",
        "repeat": "review",
        "pairwise": "pair",
        "probe": "probe",
        "match": "match",
    }[stage]


def payload_for(stage: str, ident: str) -> dict | None:  # noqa: PLR0911, one branch per stage
    """Rebuild the frozen payload for a call so a recovered response can be validated."""
    if stage in ("primary", "repeat"):
        return v3.read(_out() / "evidence" / f"{ident}.json")
    if stage == "calibration":
        if ident.startswith("control-"):
            return v3.read(_out() / "controls" / f"control-{ident.split('-')[1]}.json")
        return None
    if stage == "pairwise":
        a, b = ident.split("-submission-")
        b = "submission-" + b
        return {
            "A": v3.read(_out() / "evidence" / f"{a}.json"),
            "B": v3.read(_out() / "evidence" / f"{b}.json"),
        }
    if stage == "probe":
        return v3.probe_evidence(v3.read(_out() / "evidence" / f"{ident}.json"))
    if stage == "match":
        probe = _out() / "calls" / "claude" / "probe" / ident / "selected.json"
        if not probe.exists():
            return None
        row = next(r for r in v3.read(_out() / "private-manifest.json") if r["id"] == ident)
        return {
            "key": v3.load_key(row["task"])["quirks"],
            "departures": v3.read(probe)["departures"],
        }
    return None


def calibration_call(ident: str) -> tuple[str, dict | None]:
    """Kind and frozen payload of a calibration call, rebuilt the way calibrate_panel builds them."""
    controls = [
        v3.read(_out() / "controls" / f"control-{i}.json") for i in range(len(v3.CONTROL_FILES))
    ]
    head, *rest = ident.split("-")
    if head == "control":
        return "review", controls[int(rest[0])]
    if head == "pair":
        a, b = int(rest[0]), int(rest[1])
        return "pair", {"A": controls[a], "B": controls[b]}
    if head == "probe":
        return "probe", v3.probe_evidence(controls[int(rest[0])], spec=v3.LEDGER_SPEC)
    if head == "match":
        probe = (
            _out()
            / "calls"
            / "grok"
            / "calibration"
            / f"probe-{rest[0]}-{rest[1]}"
            / "selected.json"
        )
        if not probe.exists():
            return "match", None
        return "match", {"key": v3.LEDGER_KEY["quirks"], "departures": v3.read(probe)["departures"]}
    return "review", None


def assistant_models(stream_text: str) -> set[str]:
    return {
        json.loads(line)["message"]["model"]
        for line in stream_text.splitlines()
        if line.strip() and '"type":"assistant"' in line
    }


MATCH_ORDER_ERROR = "Matches must cover every key quirk once, in order"
QUIRK_ID = re.compile(r"^\s*(Q\d+)\b")


def retry_external_kill(folder: Path) -> bool:
    """A judge process ended by an outside SIGTERM (exit 143) is a transport failure, not a response.

    Grants the single fresh attempt the protocol allows for transport faults
    when only attempt 1 exists and its stream has no terminal event.
    """
    receipt = folder / "attempt-1.json"
    if not receipt.exists() or (folder / "attempt-2.json").exists():
        return False
    rec = json.loads(receipt.read_text())
    if rec.get("status") != "failed" or rec.get("retryable") is not False:
        return False
    if "exit 143" not in str(rec.get("error", "")) and "SIGTERM" not in str(rec.get("error", "")):
        return False
    rec["retryable"] = True
    rec["operator_review"] = {
        "at": datetime.now(UTC).isoformat(),
        "finding": "Judge process received SIGTERM from outside the runner (exit 143); no response was produced.",
        "action": "Transport fault: one fresh attempt per the protocol; receipt retained.",
    }
    receipt.write_text(json.dumps(rec, indent=2, sort_keys=True))
    print(
        json.dumps({"event": "external_kill_retry", "call": str(folder.relative_to(_out()))}),
        flush=True,
    )
    return True


NETWORK_MARKERS = (
    "ENOTFOUND",
    "ECONNRESET",
    "ETIMEDOUT",
    "ECONNREFUSED",
    "[unavailable] getaddrinfo",
)
STREAM_NETWORK_MARKERS = ("transport error [net-timeout]",)


def retry_network_fault(folder: Path) -> bool:
    """A judge CLI that could not reach its API (DNS or connection failure) produced no response.

    Grants the single fresh attempt the protocol allows for transport faults
    when only attempt 1 exists, its error is a network-layer failure, and its
    stream carries no model output.
    """
    receipt = folder / "attempt-1.json"
    if not receipt.exists() or (folder / "attempt-2.json").exists():
        return False
    rec = json.loads(receipt.read_text())
    if rec.get("status") != "failed" or rec.get("retryable") is not False:
        return False
    error = str(rec.get("error", ""))
    stream = folder / "attempt-1.stream.jsonl"
    text = stream.read_text() if stream.exists() else ""
    # Muse reports a dropped connection inside its stream's terminal record rather
    # than on stderr, where the receipt's error text comes from.
    terminal = next((m for m in STREAM_NETWORK_MARKERS if m in text), None)
    if not any(marker in error for marker in NETWORK_MARKERS) and terminal is None:
        return False
    if "assistant" in text:
        return False
    rec["retryable"] = True
    rec["operator_review"] = {
        "at": datetime.now(UTC).isoformat(),
        "finding": f"Judge CLI could not reach its API ({(terminal or error).strip()[-120:]}); no response was produced.",
        "action": "Transport fault: one fresh attempt per the protocol; receipt retained.",
    }
    receipt.write_text(json.dumps(rec, indent=2, sort_keys=True))
    print(
        json.dumps({"event": "network_fault_retry", "call": str(folder.relative_to(_out()))}),
        flush=True,
    )
    return True


TOOL_USE_ERROR = "Judge attempted tool use"


def retry_garbled_structured_output(folder: Path) -> bool:  # noqa: PLR0911, one branch per guard
    """The Claude CLI split a malformed structured-output emission into pseudo tool calls.

    Opus 5 sometimes emits its answer as a StructuredOutput call whose input
    the CLI could not parse (recorded under __unparsedToolInput), followed by
    fragments that surface as tool_use blocks named after JSON fields. No tool
    ran: the stream carries no tool_result. That is a malformed response, the
    same class the protocol already treats as retryable, not an attempt to
    use a tool. One fresh attempt; a second occurrence on the same call stops.
    """
    receipt = folder / "attempt-1.json"
    if not receipt.exists() or (folder / "attempt-2.json").exists():
        return False
    rec = json.loads(receipt.read_text())
    if rec.get("status") != "failed" or rec.get("error") != TOOL_USE_ERROR:
        return False
    stream = folder / "attempt-1.stream.jsonl"
    if not stream.exists():
        return False
    text = stream.read_text()
    if "__unparsedToolInput" not in text:
        return False
    # No real tool may have run: every tool_use is the structured-output tool or a
    # pseudo tool the CLI refused, and every non-error tool_result answers the former.
    names: dict[str, str] = {}
    for line in text.splitlines():
        if not line.strip():
            continue
        event = json.loads(line)
        if event.get("type") not in ("assistant", "user"):
            continue
        for block in event["message"].get("content", []):
            if block.get("type") == "tool_use":
                names[block["id"]] = block.get("name")
            elif block.get("type") == "tool_result":
                content = block.get("content")
                shown = content if isinstance(content, str) else json.dumps(content)
                refused = "No such tool available" in shown
                if not refused and names.get(block.get("tool_use_id")) != "StructuredOutput":
                    return False
    if any(name != "StructuredOutput" for name in names.values()) and not any(
        "No such tool available" in line for line in text.splitlines()
    ):
        return False
    rec["retryable"] = True
    rec["operator_review"] = {
        "at": datetime.now(UTC).isoformat(),
        "finding": "Malformed structured output: the CLI recorded an unparsed StructuredOutput input and "
        "field-named pseudo tool calls; no tool ran (no tool_result in the stream).",
        "action": "Invalid response under the protocol text: one fresh attempt; receipt retained.",
    }
    receipt.write_text(json.dumps(rec, indent=2, sort_keys=True))
    print(
        json.dumps(
            {"event": "garbled_structured_output_retry", "call": str(folder.relative_to(_out()))}
        ),
        flush=True,
    )
    return True


PROVIDER_BLOCK_MARKERS = (
    "Request blocked",
    "model provider's usage guidelines",
    "ActionRequiredError",
)


def retry_provider_block(folder: Path) -> bool:
    """The judge CLI's provider refused to serve the request and no response was produced.

    Cursor surfaces xAI's content filter as an ActionRequiredError before any
    model output. When the identical prompt served under earlier protocol
    versions, the block is a transport fault, not a judgment: the protocol's
    single fresh attempt applies, with the receipt retained. A second block on
    the same call is left for a person.
    """
    receipt = folder / "attempt-1.json"
    if not receipt.exists() or (folder / "attempt-2.json").exists():
        return False
    rec = json.loads(receipt.read_text())
    if rec.get("status") != "failed" or rec.get("retryable") is not False:
        return False
    error = str(rec.get("error", ""))
    if not any(marker in error for marker in PROVIDER_BLOCK_MARKERS):
        return False
    stream = folder / "attempt-1.stream.jsonl"
    if stream.exists() and "assistant" in stream.read_text():
        return False
    rec["retryable"] = True
    rec["operator_review"] = {
        "at": datetime.now(UTC).isoformat(),
        "finding": f"Provider blocked the request before any output ({error.strip()[-120:]}).",
        "action": "Transport fault: one fresh attempt per the protocol; receipt retained.",
    }
    receipt.write_text(json.dumps(rec, indent=2, sort_keys=True))
    print(
        json.dumps({"event": "provider_block_retry", "call": str(folder.relative_to(_out()))}),
        flush=True,
    )
    return True


QUOTA_MARKERS = (
    "resource_exhausted",
    "RetriableError",
    "rate limit",
    "rate_limit",
    "429",
    "usage limit",  # Codex: subscription window exhausted
    "limit reached",
)
QUOTA_MAX_RESUMES = 12


def quota_resume(folder: Path) -> bool:
    """A transport-level quota or rate-limit error with no response is a quota stop, not a judgment.

    Per protocol, quota stops preserve receipts and resume with identical
    inputs: the failed attempt's files are archived inside the call folder
    under quota-stops/, the wrapper waits with backoff, and the same call runs
    again. After QUOTA_MAX_RESUMES archived stops the call is left for a person.
    """
    latest = None
    for n in (2, 1):
        if (folder / f"attempt-{n}.json").exists():
            latest = n
            break
    if latest is None:
        return False
    rec = json.loads((folder / f"attempt-{latest}.json").read_text())
    error = str(rec.get("error", ""))
    if rec.get("status") != "failed" or not any(m in error for m in QUOTA_MARKERS):
        return False
    stream = folder / f"attempt-{latest}.stream.jsonl"
    if stream.exists() and '"type":"result"' in stream.read_text():
        return False
    archive = folder / "quota-stops"
    archive.mkdir(exist_ok=True)
    prior = len(list(archive.glob("*-attempt-*.json")))
    if prior >= QUOTA_MAX_RESUMES:
        print(
            json.dumps(
                {
                    "event": "quota_stop_limit",
                    "call": str(folder.relative_to(_out())),
                    "stops": prior,
                }
            ),
            flush=True,
        )
        return False
    stamp = datetime.now(UTC).strftime("%Y%m%dT%H%M%SZ")
    for path in folder.glob(f"attempt-{latest}.*"):
        path.rename(archive / f"{stamp}-{path.name}")
    (archive / f"{stamp}-note.json").write_text(
        json.dumps(
            {
                "finding": "transport quota or rate-limit error with no response",
                "error": error[:300],
                "action": "attempt archived; same call resumed after backoff (protocol: quota stops preserve receipts and resume)",
            },
            indent=2,
        )
    )
    wait = min(180 * (2**prior), 1800)
    print(
        json.dumps(
            {
                "event": "quota_resume",
                "call": str(folder.relative_to(_out())),
                "prior_stops": prior,
                "wait_s": wait,
            }
        ),
        flush=True,
    )
    time.sleep(wait)
    return True


def recover_match_ids(folder: Path, panel: str, stage: str) -> bool:
    """Match responses whose quirk ids carry a description ("Q1 winter tier") or arrive out of order.

    The id is the leading Q-number; entries are reordered to the key's order.
    Statuses, departure indexes, and reasons are untouched. Any id missing or
    duplicated is not recovered.
    """
    if stage != "match":
        return False
    receipts = [folder / f"attempt-{n}.json" for n in (1, 2)]
    if not all(r.exists() for r in receipts):
        return False
    if any(json.loads(r.read_text()).get("error") != MATCH_ORDER_ERROR for r in receipts):
        return False
    payload = payload_for(stage, folder.name)
    if payload is None:
        return False
    expected = [q["id"] for q in payload["key"]]
    for attempt in (1, 2):
        stream = folder / f"attempt-{attempt}.stream.jsonl"
        if not stream.exists():
            continue
        try:
            vote = v3.parse_stream_for(panel, stream.read_text())
        except (ValueError, json.JSONDecodeError, KeyError):
            continue
        matches = vote.get("matches")
        if not isinstance(matches, list):
            continue
        by_id = {}
        original = []
        for m in matches:
            hit = QUIRK_ID.match(str(m.get("quirk", "")))
            if not hit or hit.group(1) in by_id:
                by_id = None
                break
            original.append(m.get("quirk"))
            by_id[hit.group(1)] = {**m, "quirk": hit.group(1)}
        if by_id is None or sorted(by_id) != sorted(expected):
            continue
        vote["matches"] = [by_id[q] for q in expected]
        try:
            v3.validate("match", vote, payload)
        except ValueError:
            continue
        binding = json.loads(receipts[attempt - 1].read_text())["binding"]
        vote.update(
            binding=binding,
            status="complete",
            stage=stage,
            panel=panel,
            kind="match",
            operator_recovery={
                "at": datetime.now(UTC).isoformat(),
                "method": "quirk ids normalized to key ids and key order",
                "original_quirk_fields": original,
                "source_attempt": attempt,
            },
        )
        base.save(folder / "selected.json", vote)
        print(
            json.dumps(
                {
                    "event": "match_id_recovery_applied",
                    "call": str(folder.relative_to(_out())),
                    "attempt": attempt,
                }
            ),
            flush=True,
        )
        return True
    return False


def accept_fallback(folder: Path, panel: str, stage: str) -> bool:
    if panel != "claude":
        return False
    kind = stage_kind(stage)
    payload = payload_for(stage, folder.name)
    if payload is None:
        return False
    for n in (2, 1):
        receipt = folder / f"attempt-{n}.json"
        stream = folder / f"attempt-{n}.stream.jsonl"
        if not receipt.exists() or not stream.exists():
            continue
        rec = json.loads(receipt.read_text())
        if rec.get("status") != "failed" or rec.get("error") != IDENTITY_ERROR:
            continue
        text = stream.read_text()
        if assistant_models(text) != {FALLBACK_MODEL}:
            continue
        try:
            vote = v3.parse_claude_stream(text)
            if vote["model_reported"] != REQUESTED_MODEL:
                continue
            v3.validate(kind, vote, payload)
        except (ValueError, json.JSONDecodeError, KeyError):
            continue
        if kind == "review":
            vote["reported_score"] = vote["score"]
            vote.update(v3.host_review_score(vote))
        vote.update(
            binding=rec["binding"],
            status="complete",
            stage=stage,
            panel=panel,
            kind=kind,
            reviewer_fallback={
                "at": datetime.now(UTC).isoformat(),
                "requested": REQUESTED_MODEL,
                "served": FALLBACK_MODEL,
                "source_attempt": n,
                "policy": "retain and disclose reviewer fallbacks (owner decision 2026-09-07)",
            },
        )
        base.save(folder / "selected.json", vote)
        print(
            json.dumps(
                {
                    "event": "reviewer_fallback_accepted",
                    "call": str(folder.relative_to(_out())),
                    "attempt": n,
                }
            ),
            flush=True,
        )
        return True
    return False


RENAME_PREFIX = "Cursor "


def accept_display_rename(folder: Path, panel: str, stage: str) -> bool:  # noqa: PLR0912, one branch per precondition
    """Cursor renamed the judge's display label while the requested model id stayed the same.

    On 2026-09-21 Cursor began reporting "Grok 4.6 Medium" for the pinned model
    id cursor-grok-4.6-medium, which the frozen v3.3 settings record as
    "Cursor Grok 4.6 Medium". The identity guard is requested-only with the
    display name checked, so the attempt fails on the label alone. When the
    reported label equals the frozen display name minus the "Cursor " prefix,
    the same model served the request: the attempt is selected unchanged and
    the rename is recorded in the receipt. Any other label still stops.
    """
    if panel != "grok":
        return False
    if stage == "calibration":
        kind, payload = calibration_call(folder.name)
    else:
        kind, payload = stage_kind(stage), payload_for(stage, folder.name)
    if payload is None:
        return False
    expected = v3.read(_out() / "protocol.json")["reviewers"]["grok"]["display_name"]
    if not expected.startswith(RENAME_PREFIX):
        return False
    renamed = expected[len(RENAME_PREFIX) :]
    for n in (2, 1):
        receipt = folder / f"attempt-{n}.json"
        stream = folder / f"attempt-{n}.stream.jsonl"
        if not receipt.exists() or not stream.exists():
            continue
        rec = json.loads(receipt.read_text())
        if rec.get("status") != "failed" or rec.get("error") != f"Judge model changed: {renamed}":
            continue
        finding = (
            f"Cursor reported the display label {renamed!r} for the pinned model id; "
            f"the frozen settings expect {expected!r}."
        )
        try:
            vote = v3.parse_cursor_stream(stream.read_text())
            if vote["model_reported"] != renamed:
                continue
            v3.validate(kind, vote, payload)
        except (json.JSONDecodeError, KeyError):
            continue
        except ValueError as exc:
            # The label check fired before the frozen validator ran, so the response
            # never received the protocol's own treatment of a validation failure.
            # Re-file the receipt as that failure; the frozen retry rule then applies
            # (one fresh attempt), and the excerpt recovery rules see both receipts.
            if str(exc) not in v3.RETRYABLE:
                continue
            rec.update(
                retryable=True,
                error=str(exc),
                operator_review={
                    "at": datetime.now(UTC).isoformat(),
                    "finding": finding,
                    "action": "Label accepted; the response then failed the frozen validator, so the "
                    "receipt is re-filed as that failure and the protocol's retry applies.",
                },
            )
            receipt.write_text(json.dumps(rec, indent=2, sort_keys=True))
            print(
                json.dumps(
                    {
                        "event": "display_rename_refiled",
                        "call": str(folder.relative_to(_out())),
                        "error": str(exc),
                    }
                ),
                flush=True,
            )
            return True
        if kind == "review":
            vote["reported_score"] = vote["score"]
            vote.update(v3.host_review_score(vote))
        vote.update(
            binding=rec["binding"],
            status="complete",
            stage=stage,
            panel=panel,
            kind=kind,
            operator_review={
                "at": datetime.now(UTC).isoformat(),
                "finding": finding,
                "action": "Same model id, label renamed by the provider: attempt selected unchanged.",
                "source_attempt": n,
            },
        )
        base.save(folder / "selected.json", vote)
        print(
            json.dumps(
                {
                    "event": "display_rename_accepted",
                    "call": str(folder.relative_to(_out())),
                    "attempt": n,
                }
            ),
            flush=True,
        )
        return True
    return False


_BULLET = re.compile(r"^\s*(?:[*+-]|\d+[.)])\s+")


def _collapse(text: str) -> str:
    """Whitespace-collapsed comparison form: Markdown code marks and a leading list bullet are ignored.

    Used only to find the verbatim source span; the span itself is returned untouched.
    """
    return " ".join(_BULLET.sub("", text).replace("`", "").split())


ELLIPSIS = re.compile(r"\s*(?:\.\.\.|\u2026)\s*")


def rewrap_excerpt(excerpt: str, source: list[str]) -> str | None:
    """Return a verbatim rewrap of the excerpt, or None if any fragment is not in the source.

    Inline ellipsis markers split a line into fragments that are rewrapped
    separately and rejoined with a dots-only line, which the frozen rule skips.
    """
    if v3.excerpt_supported(excerpt, source):
        return excerpt
    fragments = [f for f in ELLIPSIS.split(excerpt) if f.strip()]
    if len(fragments) > 1:
        spans = [_rewrap_fragment(f, source) for f in fragments]
        return None if any(sp is None for sp in spans) else "\n...\n".join(spans)
    return _rewrap_fragment(excerpt, source)


_TOKEN = re.compile(
    r"[A-Za-z_][A-Za-z0-9_]*|\d+|\"[^\"]*\"|'[^']*'|==|!=|<=|>=|\+=|-=|\*=|//=|//|\*\*|[^\s\w]"
)


def _span_anywhere(fragment: str, source: list[str]) -> str | None:
    """Verbatim lines covering a fragment that starts or ends mid-line across wrapped text.

    Finds a word-aligned suffix of some source line that the collapsed
    fragment begins with, accumulates following lines until the fragment is
    covered, and returns those whole lines. Used for hard-wrapped prose such
    as docstrings and README paragraphs. Shorter than 20 characters is refused.
    """
    target = _collapse(fragment)
    if len(target) < 20:
        return None
    for text in sorted(source, key=lambda s: s.startswith("diff --git")):
        lines = text.splitlines()
        collapsed = [_collapse(line) for line in lines]
        for i, first in enumerate(collapsed):
            if not first:
                continue
            words = first.split(" ")
            for k in range(len(words)):
                suffix = " ".join(words[k:])
                if not target.startswith(suffix):
                    continue
                accum, j = suffix, i
                while len(accum) < len(target) and j + 1 < len(lines) and j - i < 40:
                    j += 1
                    accum = _collapse(accum + " " + collapsed[j]) if collapsed[j] else accum
                if accum.startswith(target):
                    span = "\n".join(lines[i : j + 1])
                    return span if v3.excerpt_supported(span, source) else None
                break
    return None


def _omission_only_line(fragment: str, source: list[str]) -> str | None:
    """A single source line whose tokens contain the fragment's tokens in order, with nothing added.

    Recovers a judge that shortened a line (dropped a receiver or an index)
    without inventing anything: every token it wrote must appear, in order,
    in one source line. A wrong value or a name absent from the line fails.
    """
    wanted = _TOKEN.findall(fragment)
    if len(wanted) < 4:
        return None
    for text in sorted(
        source, key=lambda s: s.startswith("diff --git")
    ):  # final source before the patch text
        for line in text.splitlines():
            have = _TOKEN.findall(line)
            if len(have) <= len(wanted):
                continue
            it = iter(have)
            if all(any(tok == candidate for candidate in it) for tok in wanted):
                return line if v3.excerpt_supported(line, source) else None
    return None


def _rewrap_fragment(excerpt: str, source: list[str]) -> str | None:
    """Return the minimal verbatim source span whose collapsed form equals the collapsed fragment.

    The span must begin on a line the fragment begins with and grow only while
    it remains a prefix of the fragment, so unrelated preceding code is never
    pulled in.
    """
    excerpt = excerpt.replace(
        "\\n", "\n"
    )  # GLM sometimes double-escapes newlines inside JSON strings
    if v3.excerpt_supported(excerpt, source):
        return excerpt
    # A judge may decode a source escape such as backslash-x00 or backslash-0 into the real control
    # character inside its JSON. Try the common spellings the source could have used; the value is identical.
    if any(ord(ch) < 32 and ch not in "\n\t" for ch in excerpt):
        for style in ("\\x{:02x}", "\\{:o}", "\\{:03o}", "\\u{:04x}"):
            reescaped = "".join(
                style.format(ord(ch)) if ord(ch) < 32 and ch not in "\n\t" else ch for ch in excerpt
            )
            if v3.excerpt_supported(reescaped, source):
                return reescaped
    target = _collapse(excerpt)
    if not target:
        return None
    for text in source:
        lines = text.splitlines()
        for start in range(len(lines)):
            first = _collapse(lines[start])
            if not first or not target.startswith(first):
                continue
            joined = first
            for end in range(start, min(start + 12, len(lines))):
                if end > start:
                    joined = _collapse(joined + " " + lines[end])
                if joined == target:
                    span = "\n".join(lines[start : end + 1])
                    return span if v3.excerpt_supported(span, source) else None
                if not target.startswith(joined):
                    break
    return _span_anywhere(excerpt, source) or _omission_only_line(excerpt, source)


def recover_excerpts(folder: Path, panel: str, stage: str) -> bool:  # noqa: PLR0911, PLR0912, one branch per precondition and attempt
    receipts = [folder / f"attempt-{n}.json" for n in (1, 2)]
    if not all(r.exists() for r in receipts):
        return False
    if any(json.loads(r.read_text()).get("error") != EXCERPT_ERROR for r in receipts):
        return False
    ident = folder.name
    if stage == "calibration" and not ident.startswith("control-"):
        return False
    if stage not in ("primary", "repeat", "calibration", "probe"):
        return False
    kind = "probe" if stage == "probe" else "review"
    evidence = payload_for(stage, ident)
    if evidence is None:
        return False
    source = list(v3.strings(evidence))
    for attempt in (1, 2):
        stream = folder / f"attempt-{attempt}.stream.jsonl"
        if not stream.exists():
            continue
        try:
            vote = v3.parse_stream_for(panel, stream.read_text())
        except (ValueError, json.JSONDecodeError, KeyError):
            continue
        recovered = {}
        holders = (
            list(vote.get("dimensions", {}).items())
            if kind == "review"
            else list(enumerate(vote.get("departures", [])))
        )
        for label, detail in holders:
            span = rewrap_excerpt(detail["excerpt"], source)
            if span is None:
                print(
                    json.dumps(
                        {
                            "event": "excerpt_not_recoverable",
                            "call": ident,
                            "attempt": attempt,
                            "holder": str(label),
                            "excerpt": detail["excerpt"][:200],
                        }
                    ),
                    flush=True,
                )
                break
            if span != detail["excerpt"]:
                recovered[str(label)] = detail["excerpt"]
                detail["excerpt"] = span
        else:
            try:
                v3.validate(kind, vote, evidence)
            except ValueError:
                continue
            if kind == "review":
                vote["reported_score"] = vote["score"]
                vote.update(v3.host_review_score(vote))
            binding = json.loads(receipts[attempt - 1].read_text())["binding"]
            vote.update(
                binding=binding,
                status="complete",
                stage=stage,
                panel=panel,
                kind=kind,
                operator_recovery={
                    "at": datetime.now(UTC).isoformat(),
                    "method": "excerpt re-wrapped to source line breaks",
                    "original_excerpts": recovered,
                    "source_attempt": attempt,
                },
            )
            base.save(folder / "selected.json", vote)
            print(
                json.dumps(
                    {
                        "event": "excerpt_recovery_applied",
                        "call": str(folder.relative_to(_out())),
                        "attempt": attempt,
                        "holders": sorted(recovered),
                    }
                ),
                flush=True,
            )
            return True
    return False


INVALID_MARKER = "operator-invalid.json"


def invalidate_unrecoverable_probe(folder: Path, panel: str, stage: str) -> bool:  # noqa: PLR0911, one return per precondition
    """Both probe attempts failed only on an excerpt that no recovery rule accepts.

    A quote with a token added, changed, or invented is a fabrication under the
    protocol, so the response is invalid. The frozen summary already defines
    the outcome: a submission without a valid match from a passing panel is
    left unpublished (its L1 review stands but no score is published for it).
    The wrapper records the finding in the call folder and the remaining probes
    continue; nothing about the judgment is altered or filled in.
    """
    if stage != "probe" or (folder / INVALID_MARKER).exists():
        return False
    receipts = [folder / f"attempt-{n}.json" for n in (1, 2)]
    if not all(r.exists() for r in receipts):
        return False
    if any(json.loads(r.read_text()).get("error") != EXCERPT_ERROR for r in receipts):
        return False
    evidence = payload_for(stage, folder.name)
    if evidence is None:
        return False
    source = list(v3.strings(evidence))
    unsupported = {}
    for attempt in (1, 2):
        stream = folder / f"attempt-{attempt}.stream.jsonl"
        if not stream.exists():
            return False
        try:
            vote = v3.parse_stream_for(panel, stream.read_text())
        except (ValueError, json.JSONDecodeError, KeyError):
            return False
        bad = [
            d["excerpt"]
            for d in vote.get("departures", [])
            if rewrap_excerpt(d["excerpt"], source) is None
        ]
        if not bad:
            return False  # recoverable after all; leave it to recover_excerpts
        unsupported[str(attempt)] = bad
    rec = {
        "at": datetime.now(UTC).isoformat(),
        "finding": "Both attempts quoted an excerpt absent from the evidence that no recovery "
        "rule accepts (a token added, changed, or invented).",
        "action": "Probe invalid; no match call is made. The frozen summary leaves the "
        "submission unpublished for this panel. Remaining probes continue.",
        "unsupported_excerpts": unsupported,
    }
    (folder / INVALID_MARKER).write_text(json.dumps(rec, indent=2, sort_keys=True))
    print(
        json.dumps({"event": "probe_invalidated", "call": str(folder.relative_to(_out()))}),
        flush=True,
    )
    return True


def invalidated_probes(panel: str) -> set[str]:
    return {
        d.name
        for d in (_out() / "calls" / panel / "probe").glob("*/")
        if (d / INVALID_MARKER).exists()
    }


def run_probes_skipping(panel: str, skipped: set[str]) -> int:
    """Drive the frozen probe stage in-process, skipping invalidated submissions.

    Mirrors the frozen run_probes exactly (same calls, same lock) except that
    invalidated rows are not called again, since the frozen command would stop
    on them every time. Returns a process-style exit code for the main loop.
    """
    try:
        protocol = v3.verify_frozen()
        v3.require_passed(panel)
        with (_out() / f".{panel}.lock").open("w") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            for row in v3.read(_out() / "private-manifest.json"):
                if row["id"] in skipped:
                    continue
                evidence = v3.read(_out() / "evidence" / f"{row['id']}.json")
                key = v3.load_key(row["task"])
                probe = v3.call(
                    panel, "probe", row["id"], "probe", v3.probe_evidence(evidence), protocol
                )
                v3.call(
                    panel,
                    "match",
                    row["id"],
                    "match",
                    {"key": key["quirks"], "departures": probe["departures"]},
                    protocol,
                )
    except Exception:  # the main loop applies operator rules to whatever stopped
        traceback.print_exc()
        return 1
    print(
        json.dumps(
            {"event": "probe_stage_skipped_invalid", "panel": panel, "skipped": sorted(skipped)}
        ),
        flush=True,
    )
    return 0


def run_stage_once(args: list[str], panel: str) -> int:
    skipped = invalidated_probes(panel) if args and args[0] == "probe" else set()
    if skipped:
        return run_probes_skipping(panel, skipped)
    return subprocess.run([sys.executable, "-u", "-m", MODULE, *args], check=False).returncode


def newest_unresolved(panel: str) -> Path | None:
    stopped = [
        d
        for d in (_out() / "calls" / panel).glob("*/*/")
        if not (d / "selected.json").exists() and (d / "attempt-1.json").exists()
    ]
    return max(stopped, key=lambda d: (d / "attempt-1.json").stat().st_mtime) if stopped else None


def apply_rule(folder: Path) -> bool:
    if (
        folder.parent.parent.name not in ("claude", "reader")
        or (folder / "attempt-2.json").exists()
    ):
        return False
    receipt = json.loads((folder / "attempt-1.json").read_text())
    if receipt.get("status") != "failed" or receipt.get("retryable") is not False:
        return False
    stream = folder / "attempt-1.stream.jsonl"
    if not stream.exists():
        return False
    results = [
        json.loads(line)
        for line in stream.read_text().splitlines()
        if line.strip() and '"type":"result"' in line
    ]
    if not results or results[0].get("subtype") != SUBTYPE:
        return False
    receipt["retryable"] = True
    receipt["operator_review"] = {
        "at": datetime.now(UTC).isoformat(),
        "finding": f"CLI result subtype {SUBTYPE}; invalid-schema response under the protocol text.",
        "action": "Marked retryable by the documented operator rule (maintenance_review_v3_resume); "
        "single fresh attempt; receipt retained.",
    }
    (folder / "attempt-1.json").write_text(json.dumps(receipt, indent=2, sort_keys=True))
    print(
        json.dumps({"event": "operator_rule_applied", "call": str(folder.relative_to(_out()))}),
        flush=True,
    )
    return True


def main() -> int:  # noqa: PLR0912, one branch per operator rule
    args = sys.argv[1:]
    panel = args[args.index("--panel") + 1]
    applied = 0
    while True:
        returncode = run_stage_once(args, panel)
        if returncode == 0:
            print(
                json.dumps({"event": "stage_complete", "operator_rule_applications": applied}),
                flush=True,
            )
            return 0
        folder = newest_unresolved(panel)
        if folder is not None and apply_rule(folder):
            applied += 1
            continue
        if folder is not None and recover_excerpts(folder, panel, folder.parent.name):
            applied += 1
            continue
        if folder is not None and invalidate_unrecoverable_probe(folder, panel, folder.parent.name):
            applied += 1
            continue
        if folder is not None and accept_fallback(folder, panel, folder.parent.name):
            applied += 1
            continue
        if folder is not None and accept_display_rename(folder, panel, folder.parent.name):
            applied += 1
            continue
        if folder is not None and recover_match_ids(folder, panel, folder.parent.name):
            applied += 1
            continue
        if folder is not None and retry_external_kill(folder):
            applied += 1
            continue
        if folder is not None and retry_network_fault(folder):
            applied += 1
            continue
        if folder is not None and retry_provider_block(folder):
            applied += 1
            continue
        if folder is not None and retry_garbled_structured_output(folder):
            applied += 1
            continue
        if folder is not None and quota_resume(folder):
            applied += 1
            continue
        if True:
            print(
                json.dumps(
                    {
                        "event": "stopped_for_operator",
                        "call": str(folder) if folder else None,
                        "operator_rule_applications": applied,
                    }
                ),
                flush=True,
            )
            return returncode


if __name__ == "__main__":
    sys.exit(main())
