"""The twenty Verdict v2 families: what each asks and where its truth comes from.

Builders live in ``harness.verdict.v2.families.<module>`` and expose
``BUILDERS``: a map from family id to ``build(ctx) -> list[Item]``. The
registry is the single list of families; a family without a builder yet is
reported as unbuilt, never silently dropped.
"""

from __future__ import annotations

import importlib
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from harness.verdict.v2.items import Item

PILLARS = ("software", "general")


@dataclass(frozen=True)
class Family:
    family_id: str
    pillar: str
    area: str
    question_type: str
    reference: str
    module: str
    summary: str


FAMILIES: tuple[Family, ...] = (
    # Software pillar
    Family(
        "code-output",
        "software",
        "Reading code",
        "choice",
        "execution",
        "code_gen",
        "What does this short program print?",
    ),
    Family(
        "type-check-pair",
        "software",
        "Reading code",
        "choice",
        "tool",
        "code_gen",
        "Which of two snippets passes the type checker?",
    ),
    Family(
        "patch-pair",
        "software",
        "Reviewing changes",
        "choice",
        "verifier",
        "archive",
        "Two size-matched patches for one issue: which passes the hidden tests?",
    ),
    Family(
        "failing-test",
        "software",
        "Reviewing changes",
        "choice",
        "verifier",
        "archive",
        "Given a failing patch, which hidden test fails?",
    ),
    Family(
        "fix-file",
        "software",
        "Finding bugs",
        "choice",
        "merged-fix",
        "mined",
        "Which file does the fix touch?",
    ),
    Family(
        "bug-function",
        "software",
        "Finding bugs",
        "choice",
        "execution",
        "code_gen",
        "Given a failing test's output, which function holds the planted bug?",
    ),
    Family(
        "vuln-pair",
        "software",
        "Security",
        "choice",
        "merged-fix",
        "mined",
        "Before and after a security fix: which version is vulnerable?",
    ),
    Family(
        "weakness-class",
        "software",
        "Security",
        "choice",
        "advisory",
        "mined",
        "Which weakness class is this vulnerability?",
    ),
    Family(
        "mutant-kill",
        "software",
        "Testing",
        "noul",
        "execution",
        "code_gen",
        "Does this test catch this change?",
    ),
    Family(
        "expected-value",
        "software",
        "Testing",
        "noul",
        "execution",
        "code_gen",
        "Is this assertion's expected value correct for the spec?",
    ),
    Family(
        "ci-failure",
        "software",
        "Operations",
        "choice",
        "verifier",
        "archive",
        "Why did this CI run fail?",
    ),
    Family(
        "semver-impact",
        "software",
        "Operations",
        "score",
        "tool",
        "mined",
        "Is this API change a patch, minor or major version bump?",
    ),
    # General pillar
    Family(
        "constraint-pick",
        "general",
        "Logic",
        "choice",
        "generator",
        "general",
        "Which assignment satisfies every rule?",
    ),
    Family(
        "entailment",
        "general",
        "Logic",
        "noul",
        "generator",
        "general",
        "Does the conclusion follow from the premises?",
    ),
    Family(
        "word-problem",
        "general",
        "Math",
        "choice",
        "generator",
        "general",
        "Multi-step word problem: which answer is right?",
    ),
    Family(
        "estimate-band",
        "general",
        "Math",
        "score",
        "generator",
        "general",
        "Which band contains this computed quantity?",
    ),
    Family(
        "table-lookup",
        "general",
        "Tables",
        "choice",
        "generator",
        "general",
        "Which row or group answers this question about the table?",
    ),
    Family(
        "table-count-band",
        "general",
        "Tables",
        "score",
        "generator",
        "general",
        "How many rows match this filter?",
    ),
    Family(
        "policy-decision",
        "general",
        "Rules and policy",
        "noul",
        "generator",
        "general",
        "Under this policy, is this case allowed?",
    ),
    Family(
        "policy-clause",
        "general",
        "Rules and policy",
        "choice",
        "generator",
        "general",
        "Which clause decides this case?",
    ),
)
BY_ID = {f.family_id: f for f in FAMILIES}


@dataclass(frozen=True)
class BuildContext:
    """What a builder may read. ``seed`` fixes every generated family."""

    seed: int
    repo: Path
    run_roots: tuple[Path, ...] = ()
    tasks_roots: tuple[Path, ...] = ()
    per_family: int = 250
    options: dict[str, Any] | None = None


Builder = Callable[[BuildContext], list[Item]]


def builder_for(family_id: str) -> Builder | None:
    family = BY_ID[family_id]
    try:
        module = importlib.import_module(f"harness.verdict.v2.families.{family.module}")
    except ModuleNotFoundError as error:
        if error.name and error.name.endswith(family.module):
            return None
        raise
    builder: Builder | None = getattr(module, "BUILDERS", {}).get(family_id)
    return builder
