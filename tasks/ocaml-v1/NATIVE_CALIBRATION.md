# Native OCaml development calibration

The separate native Mac sweep finished October 1, 2026 at 12:09 PM PDT.
GPT-6.1 Sol medium, Codex 0.159.0, subscription billing, serial concurrency,
three fresh attempts per selected task, and the full task-default solve budgets
produced **19/21 complete task passes (90.5%) across the seven library tasks**.
This is a subset result, not a score for the eight-task suite.

| Library task | Complete passes |
|---|---|
| async-bounded-dispatch | 3/3 |
| incremental-atomic-graph | 1/3 |
| interval-map-transaction | 3/3 |
| typed-frame-stream | 3/3 |
| window-watermark-allocation | 3/3 |
| typed-query-optimizer | 3/3 |
| async-generation-cache | 3/3 |

Both graph failures were on `atomic_dependency_reversal`. They validated the
prospective topology but changed formula bindings independently. The passing
patch used an Incremental Expert barrier to detach replaced edges before
attaching replacements. These observations describe the inspected patches,
not a maintainability or production-readiness score.

Receipt audit verified the frozen scoring/code digests, effective solve budgets,
OCaml 5.2.1, Dune 3.17.2, medium effort, and pinned CLI version for all 21 runs.
Every regression guard passed. No run was marked contaminated, no submitted
patch edited an interface or captured generated build paths, and no invocation
or agent errors were recorded. No extra model calls or patch replay were made
during this audit.

The compiler task remains pending after its earlier provider refusal. The
eight-task aggregate stays withheld. Historical Docker calibration remains
17/21 (81.0%) in its own cohort. The under-80% target remains unproven, and six
library tasks passing 3/3 each supports expanding engineering depth before
calling this a hard benchmark. Keep this measured version frozen while
developing future candidates under separate revisions.

[Audit and complete run inventory](NATIVE_CALIBRATION.json) preserve the evidence.
The run directory `runs-ocaml-native-20261001T175608Z` contains `LAUNCH.json`,
`PROGRESS.json`, all original patches/traces, and the final `MODEL_CARD.json`/HTML.
The eight-task card displays incomplete coverage and no headline score.
The earlier `NATIVE_READINESS.json` is a preserved pre-sweep snapshot.

These are development results with requested-only model identity, no independent
confirmation, and no OCaml quality/security factors. HTML visual rendering
remains unverified. Native execution conditions differ from Docker conditions.
