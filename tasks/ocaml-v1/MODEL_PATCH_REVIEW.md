# OCaml model patch review

All 21 current original-task patches reproduce their recorded functional scores
and exact failing-check names in fresh offline source-only workspaces. The result
remains 17 complete passes and four failures. This is patch reproducibility,
not 21 new model attempts or independent confirmation.

[MODEL_SOURCE_REPLAY.json](MODEL_SOURCE_REPLAY.json) preserves every command and
verdict. [MODEL_PATCH_REVIEW.json](MODEL_PATCH_REVIEW.json) records the inventory,
review scope, source hashes, and inspected verifier image identity. The original
raw patches and model receipts remain in runs-ocaml-pilot. Derived source patches
are in patch-audit; none replaces the original submitted patch.

## Reproducibility finding

The three Async dispatcher runs predate the Dune artifact-capture correction.
Each raw patch contains 33 generated paths under _build, including compiled
objects, Dune cache state, and copies of source files. These are unsuitable as a
portable source contribution. Removing only those generated paths produces a
source patch that still passes every existing check in each fresh workspace.
No supplied starting-project source was excluded. The other 18 derived patches
have exactly the same SHA-256 as their original patches.

The capture fix and its three integration cases already passed full CI. Preserve
the older raw patches as historical receipts; use the explicitly derived copies
when reviewing or replaying their source. This finding does not change their
recorded model scores or establish a new capability failure.

## Representative implementation review

Close inspection covered library code in the lexically first current run for each
of the seven original task families, plus the failed cache run. Public-test changes
were inventoried across all 21 patches, not exhaustively reviewed. No numeric
maintainability rating or mergeability certification is assigned.

| Family and inspected patch | Implementation observation | Engineer-facing interpretation |
|---|---|---|
| [Async dispatcher](patch-audit/async-bounded-dispatch-357e35a8.diff) | Two-list FIFO, explicit Open/Closing/Aborted state, one Ivar per accepted job, and completion after active/pending work drains | The specified queue and shutdown contract reproduces. Asynchronously raised exceptions are explicitly outside this task; use the cache task to examine monitor ownership. |
| [Incremental graph](patch-audit/incremental-atomic-graph-19fa2932.diff) | Prospective topology is validated, then existing formula Vars change while named nodes remain linked | All three attempts still raise a transient cyclic-edge error during valid atomic dependency reversal. Validating the final graph does not make application of its changes atomic. |
| [Interval map](patch-audit/interval-map-transaction-4ad37e00.diff) | Base comparator-aware persistent Tree split/append operations restore endpoint values; batch errors propagate without publishing partial state | This is useful Base/type-witness coverage. All three attempts reproduce the stated behavior, comparator-work, allocation, and interface checks; harder repository integration remains unmeasured. |
| [Typed frame stream](patch-audit/typed-frame-stream-0b64e84a.diff) | Typed encoding and existential decoding preserve packet kinds; incremental framing keeps a partial frame and explicit poison/reset state | The bounded two-kind protocol is saturated at this configuration. It is an easy anchor, not evidence about large versioned schemas or bin_prot migrations. |
| [Window aggregation](patch-audit/window-watermark-allocation-13b7b5e3.diff) | Fixed-size bucket arrays maintain running totals and clear only newly expired buckets | The native allocation/work contracts reproduce. Coding-run speed and emulated wall-clock time are separate from the submitted program's tested efficiency. |
| [Typed optimizer](patch-audit/typed-query-optimizer-15dd0191.diff) | Rank-polymorphic lifting preserves bound-variable scope; recursive purity/usage predicates protect effects and sharing | All three patches reproduce semantic and structural checks. The task has no optimizer runtime or allocation contract, so these passes do not establish production-scale optimization cost. |
| [Passing cache](patch-audit/async-generation-cache-6bd6f12c.diff) | Generation identity prevents stale publication; retirement occurs before subscriber callbacks; eligibility precedes subscription | This is substantive lifecycle coverage across cancellation, retries, deadlines, closure, and synchronous reentrancy. Its ownership order is important to review alongside the signatures. |
| [Failed cache](patch-audit/async-generation-cache-e06a92cd.diff) | An existing generation receives the new subscriber before its overdue deadline is checked | Fresh replay reproduces deadline_dominance failure. The new caller is incorrectly included in expiration of the old generation rather than starting new work. Most surrounding lifecycle behavior still passes. |

None of the 21 source patches edits a supplied .mli file, and every regression
and interface guard passes in fresh replay. Eighteen patches add or alter public
tests. The three graph failures despite added tests show why test quantity alone
is insufficient: the specified transaction interleaving must actually be covered.
These observations do not prove the absence of every type-safety or maintenance
issue outside the tested contracts.

## Compiler integration finding

The separate old core-only compiler patch passed its eight scored checks in
clean replay but left one affected public expect snapshot inconsistent. That
already stated maintenance obligation became the revised public_expect_workflow
guard. Its new reference/control gate and full cold reference build pass; fresh
model outcomes on the revised definition remain unavailable. See
[MODEL_PATCH_REPLAY.json](compiler-gadt-field-safety/MODEL_PATCH_REPLAY.json) and
[the admission record](compiler-gadt-field-safety/ADMISSION.md).

This is particularly useful for an engineer evaluating agents: source-level
correctness, maintenance of repository workflows, and asynchronous or transactional
invariants need distinct evidence. The [Jane Street research](JANE_STREET_RESEARCH.md)
explains the public motivation. These examples are independent task observations,
not a claim of internal representativeness or Jane Street endorsement.

## Replay a saved source contribution

From the repository root, with the pinned verifier image available:

```sh
.venv/bin/python - <<'PY'
from pathlib import Path
from scripts.validate_ocaml_pilot import exercise
root = Path("tasks/ocaml-v1")
result = exercise(
    root / "async-bounded-dispatch",
    root / "patch-audit/async-bounded-dispatch-357e35a8.diff",
    initialize_git=True,
)
print(result["verdict"])
PY
```

The helper creates a fresh network-off workspace and applies the saved source
patch. Model calls are not made. Use the receipt's task hash and source-patch
hash to detect drift before comparing a later replay with this audit.
