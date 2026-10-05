# Native candidate calibration

Completed October 1, 2026 PDT (2026-10-02 UTC), on this Mac without Docker.
The three candidates scored **7/9 complete task passes, 77.8% task pass@1**.
This is below 80% for this candidate trio. It is not a below-80% result for
the entire expanded OCaml pool.

| Task | Complete passes | Behavior groups per attempt | Regression guards |
|---|---|---|---|
| async-durable-delivery | 3/3 | 9/9 in every attempt | Both pass in all attempts |
| typed-schema-evolution | 1/3 | 5/6, 5/6, 6/6 | Both pass in all attempts |
| base-nested-transactions | 3/3 | 6/6 in every attempt | Both pass in all attempts |

Task pass@1 requires every behavior and regression check to pass. It is not
the mean fraction of passing assertion groups. Each task has equal weight.
The engineering view is 6/6 complete passes (100%); the overlapping language
mastery view is 4/6 (66.7%). Base enters both views but only once in the total.

## Conditions and evidence

GPT-6.1 Sol through pinned Codex CLI 0.159.0, medium effort, subscription billing,
three fresh independent attempts per task, serial concurrency, native local
execution, no agent container, and no judges. Every receipt records the unchanged
10800-second solve allowance and 540 configured steps. No shortened budgets,
automatic retries, selective omission, or task edits during measurement occurred.
The configured step allowance is not a guarantee of a CLI turn limit.

Frozen candidate identity: e0a390e323df. The original ocaml-v1 identity remains
c0a1cc221d60. All scoring, condition, control and code digests still match their
launch receipt. Every model patch preserves the guarded interfaces. No generated
build files, benchmark-data reads, answer-key reads or contamination flags were
observed in the patch/receipt audit. Native filesystem telemetry is observational,
not a container boundary. Model execution has no enforced network isolation or
Docker resource caps. CLI model identity is requested-only, not independently
reported by the provider.

References passed three fresh offline base/reference pairs per task, every
regression guard, all eight compiling faulty controls, and standard validation.
The initial metadata failure and pre-license proofs are preserved. The final
[validation receipt](VALIDATION_OFFLINE.json) records the matching source proofs.
[CI](CI.json) passed 1093 tests with 86.38% coverage, five unrelated analyzer
skips and four deselections. A Starlette deprecation warning remains unrelated.

- [Complete functional card JSON](CALIBRATION.json)
- [HTML model card](../../runs-ocaml-candidates-20261002T011915Z/MODEL_CARD.html)
- [Launch conditions](../../runs-ocaml-candidates-20261002T011915Z/LAUNCH.json)
- [Completed dispatcher state](../../runs-ocaml-candidates-20261002T011915Z/PROGRESS.json)
- [Receipt and patch audit](RECEIPT_AUDIT.json)
- [Clean failure replay](FAILURE_REPLAY.json) and [log](FAILURE_REPLAY.log)

## Confirmed failure

Both failed attempts reproduce the same transactional_cursor failure in fresh
offline workspaces, with all other behavior groups and guards unchanged.
Backtraces point to hidden/checks.ml line 112: the cursor must return to its
entry value after a constructor changes that shared reference and raises.
The prompt states that every exception leaves the cursor unchanged and explicitly
calls out raising constructors. The first two implementations delay their own
cursor update until success but do not restore it after arbitrary callback
exceptions. The successful third attempt independently added a constructor test
that changes the cursor before raising, then resets the cursor on every exception.
This was derived from the published contract before hidden grading.

These two failures identify a specific exception-safety gap. They are not
compiler failures, missing dependencies, timeouts, provider refusals, or evidence
that all serialization behavior failed. Replay is supplemental diagnosis,
not a fresh model attempt or additional scored coverage.

Source review also found a future specification question: the successful model
tests propagation of a constructor-raised public Error.Decode exception, while
the reference uses that exception as its parser signal. Current checks require
cursor rollback and do not specify propagation policy for this exact exception.
Clarifying that policy belongs in a separately versioned task with fresh gates
and calibration, not a retrospective change to these receipts.

## Scope of the difficulty result

The earlier frozen seven-library native cohort remains 19/21 (90.5%). Together
with these candidates, the contextual ten-library arithmetic is 26/30 (86.7%).
That context is preserved in [EXPANSION_CONTEXT.json](EXPANSION_CONTEXT.json);
it is not a frozen expanded-suite model card. The original compiler task still
has no fresh current-revision outcome because its earlier provider refusal is
unresolved. A full eleven-task score remains withheld.

The candidate trio meets the observed target, but the engineering tasks remain
saturated. Supplying a large Base repository increased navigation scope without
causing measured failures. More engineering difficulty should come from deeper
stated behavior and invariants, not source size alone or reduced reasoning.

Three tasks and nine development attempts are a small selected pool, with no
independent confirmation or stable leaderboard claim. All current receipts,
including failures, are retained. The suite reports functional completion;
OCaml quality and security analyzers are not implemented. HTML semantics are
tested, but visual rendering remains unverified after the app's earlier
local-file preview restriction.

Suggested next step: build a full-repository Incremental propagation task to
add engineering difficulty before freezing an expanded suite for independent
confirmation.
