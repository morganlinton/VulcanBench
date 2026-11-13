"""Code quality maintenance v3.30.5: GPT-6.1 Sol reviews Claude Opus 5.5, for a matched panel against Fable 5.2.

The second half of the owner's request for Sol on the earlier columns of the
Claude Fable 5.2 card (chat, 2026-11-12); v3.30.4 covers Claude Fable 5.1.
Claude Opus 5.5 is published from Muse Spark 1.3 and Grok 4.6 (v3.15). This
round is a comparison record only and does not amend v3.15: Opus 5.5's
published board scores stay the Muse and Grok means.

Population: the v3.15 freeze, byte for byte, copied and checked as v3.30.2
copies v3.30. Every v3.15 submission is Anthropic's, so nothing is dropped.

Judges: no new exam, as in v3.30.4. Sol is gated by its v3.30.2 verdict and
must use the settings and Codex pins v3.30.2 examined; Muse's v3.15 reviews and
probes are scored from the v3.15 directory as a scored sibling, gated by its
v3.15 verdict.

    python -m harness.maintenance_review_v3305 prepare
    VB_MAINT_MODULE=harness.maintenance_review_v3305 \\
        python -m harness.maintenance_review_v3_resume run --panel sol
"""

from __future__ import annotations

from pathlib import Path

from harness import maintenance_review_v3 as v3
from harness import maintenance_review_v327 as v327
from harness import maintenance_review_v3302 as v3302
from harness import maintenance_review_v3304 as v3304

ROOT = v3.ROOT
PROTOCOL_ID = "code-quality-maintenance-v3.30.5"
OUT = ROOT / "runs-code-quality-maintenance-v3.30.5"
SOURCE_ID = "code-quality-maintenance-v3.15"
SOURCE_OUT = ROOT / "runs-code-quality-maintenance-v3.15"  # scored sibling: Muse on Opus 5.5
COMPARISON = ROOT / "docs/results/swe-v4-opus55-2026-09/comparison.json"
SOURCE_DOC = v3302.SOURCE_DOC
MODELS = {"opus55": ("low", "medium", "high", "extra-high", "max")}
PANELS = ("sol",)
SCORED_PANELS = ("sol", "muse")
VERDICTS = {
    "muse": (SOURCE_OUT, SOURCE_ID),
    "sol": (v3302.OUT, v3302.PROTOCOL_ID),
}
ROUND = v3304.Round(
    PROTOCOL_ID,
    OUT,
    SOURCE_ID,
    SOURCE_OUT,
    COMPARISON,
    VERDICTS,
    Path(__file__),
    (v3304.EARLIER_HOST, ROOT),  # runs-effort-opus55, restored from the private backup
)


def verify_frozen() -> dict:
    return v3304.verify_frozen_for(ROUND)


def _bind() -> None:
    """Point the frozen v3 implementation at this protocol's population and directory."""
    v3.OUT = OUT
    v3.COMPARISON = COMPARISON
    v3.PROTOCOL_ID = PROTOCOL_ID
    v3.PANELS = PANELS
    v3.SCORED_SIBLING_OUT = {"muse": SOURCE_OUT}
    v3.SCORED_PANELS = SCORED_PANELS
    v3.SENSITIVITY_PANELS = ()
    v3.SENSITIVITY_OUT = OUT
    v3.DOC = OUT / "protocol-document.md"
    v3.prepare = prepare
    v3.panel_passed = panel_passed
    v3.call = v327.call
    v3.parse_stream_for = v327.parse_stream_for
    v3.verify_frozen = verify_frozen
    v3302.check_codex_pins(OUT)


def panel_passed(panel: str) -> bool:
    """Muse by its v3.15 verdict, Sol by its v3.30.2 verdict."""
    return v3302.verdict_passed(VERDICTS, panel)


def prepare() -> None:
    src = v3304.start(ROUND)
    v3302.copy_freeze(SOURCE_OUT, OUT, COMPARISON)
    manifest = v3.read(OUT / "private-manifest.json")
    if {row["model"] for row in manifest} != set(MODELS):
        raise ValueError("The v3.15 freeze holds another model")
    protocol = v3304.build_protocol(
        ROUND,
        src,
        {
            "amends": {},
            "population": {
                "description": "the v3.15 freeze of Claude Opus 5.5, byte for byte",
                "cells": src["population"]["cells"],
                "excluded": src["population"]["excluded"],
                "missing": src["population"]["missing"],
            },
            "comparability": "Claude Opus 5.5 becomes a Muse Spark 1.3 and GPT-6.1 Sol mean, matching Fable 5.2 "
            "under v3.30.2 and v3.30.3",
        },
    )
    for field in v3302.SAME_AS_BASE:
        if protocol[field] != src[field]:
            raise ValueError(f"Protocol field {field} differs from {SOURCE_ID}")
    v3304.check_sol_exam_applies(protocol)
    v3.freeze(OUT / "protocol.json", protocol)
    v3304.preflight(ROUND, protocol)


_bind()


def main() -> None:
    v3.main()


if __name__ == "__main__":
    main()
