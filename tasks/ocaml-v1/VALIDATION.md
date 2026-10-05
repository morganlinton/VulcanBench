# OCaml pilot validation

All eight candidates passed the task-validation gate in network-off, non-root
Docker workspaces. Seven original fixtures were validated on September 30; the
upstream compiler task was readmitted October 1 after complete repeated
validation of its revised public expect workflow. There are 50 behavior checks,
20 regression/interface/workflow guards, and 15 rejected compiling faulty controls.
Model difficulty is separate: seven-task development calibration scored 81.0%;
the eight-task aggregate is withheld pending complete fresh compiler coverage.
See [CALIBRATION.md](CALIBRATION.md).

## Native Mac validation

After the owner requested Docker-free execution, all eight tasks also passed a
complete Darwin arm64 gate, followed by a separate full gate with network denied
for the validation process and its descendants. Each gate used three fresh
baseline/reference pairs, rejected all 15 compiling controls with their guards
intact, and passed standard validation. The source, prompt, assertions, guards,
and every scoring hash are unchanged. Only compiler setup paths became portable;
setup is not included in the harness task hash and is disclosed separately.

[NATIVE_VALIDATION.json](NATIVE_VALIDATION.json) preserves the first host gate;
[NATIVE_VALIDATION_OFFLINE.json](NATIVE_VALIDATION_OFFLINE.json) preserves the
network-denied gate. [NATIVE_ENVIRONMENT.json](NATIVE_ENVIRONMENT.json) records
the complete matching package versions and a separately built native public
baseline seed (4714 generated artifacts, built in 397.2 seconds). These are
reference-validation measurements, not model performance or cross-platform
timing comparisons. Linux seed artifacts were not reused on the Mac.

The final frozen-code CI passed 1092 tests, with five unrelated analyzer skips,
four deselections, 86.39% coverage, and successful lint/format/no-em-dash and
117-file mypy. [CI_NATIVE_FINAL.json](CI_NATIVE_FINAL.json) records the tested
code digests; its log and the preceding CI receipts are preserved. No fresh
native model attempts were made. Historical Docker model calibration is a
separate cohort. HTML card data/escaping are checked, but visual rendering is
unverified because the in-app browser rejected local-file previews.

## Task checks

| Task | Fail-to-pass checks | Regression/interface guards | Base behavior | Reference behavior | Incomplete control |
|---|---|---|---|---|---|
| async-bounded-dispatch | 5 | 2 | All required changes fail, guards pass | All pass | LIFO pending queue rejected |
| incremental-atomic-graph | 7 | 2 | All required changes fail, guards pass | All pass | Ignored selected branch rejected |
| interval-map-transaction | 7 | 3 | All required changes fail, guards pass | All pass | Lost restoration and full-boundary scan rejected |
| typed-frame-stream | 5 | 2 | All required changes fail, guards pass | All pass | Dropped partial frame rejected |
| window-watermark-allocation | 5 | 2 | All required changes fail, guards pass | All pass | Late boundary off-by-one and per-event allocation rejected |
| typed-query-optimizer | 7 | 3 | All required changes fail, guards pass | All pass | Dropped projection effects and incorrect binding-depth tracking rejected |
| async-generation-cache | 9 | 2 | All required changes fail, guards pass | All pass | Stale publication, subscriber-wide cancellation, and obsolete alarms rejected |
| compiler-gadt-field-safety | 5 | 4 | All required changes fail, guards pass | All pass | Blanket pointer reads, record-only repair, and row-local record equations rejected |

Every base/reference check was repeated three times in fresh workspaces. Controls
passed their regression guards, compiled successfully, and were rejected for
their intended behavioral defects. The standard harness task validator also
passed every task. Solver-workspace checks confirmed hidden tests and reference
patches were absent before verification.

The original Linux native-code allocation reference measured 0.002 allocated bytes/event and 105119
bucket visits in each of three repetitions of the specified workload. This is
below the 32-byte/event bound and the specified work bound. A correct-but-wasteful
control allocated 72.002 bytes/event, passed all four behavioral checks and both
regression guards, and failed only the allocation/work check. These are allocation
and operation measurements, not latency measurements.

The sandbox uses OCaml 5.2.1, Dune 3.17.2, and the package list in
[TOOLCHAIN.lock](TOOLCHAIN.lock), resolved from opam-repository revision
`f294d7f729ac7f68f786bfa2cf2bb0811d9e6e95`. The validation image identity is
`sha256:77987aefd1582051227ad123b7c4b9d5eb848c235ea15c413698ec9357952e4d`.
This local validation ran the Linux amd64 image under emulation on an arm64 Mac.
Do not treat its wall-clock timings as representative of native hardware.

The compiler uses upstream 5.6.0+dev0, with the matching images and seed hashes
in [PROVENANCE.json](compiler-gadt-field-safety/PROVENANCE.json). Its public
workflow guard passes at base, gold, and all three compiling faulty controls.
Control expected output was updated without changing test programs. Independent
behavior checks still reject the intentional faults. The complete revised receipt
is [VALIDATION_WORKFLOW.json](compiler-gadt-field-safety/VALIDATION_WORKFLOW.json).
Earlier core-only validation and model replay remain separately preserved.
The affected public tests are covered, not the entire upstream corpus.

Detailed per-command receipts and verdicts are in [VALIDATION.json](VALIDATION.json).
Reproduce the full audit with:

```sh
.venv/bin/python scripts/validate_ocaml_pilot.py --output /tmp/ocaml-validation.json
```

Full local CI passed October 1 after the latest harness/report changes:
1084 tests passed with 86.38% harness coverage, five unrelated analyzer skips,
four deselections, and lint,
format, no-em-dash, and 117-file mypy success. The raw log and timed receipt are
preserved as [CI_WORKFLOW.log](CI_WORKFLOW.log) and [CI_WORKFLOW.json](CI_WORKFLOW.json).
The skips reflect unavailable Clang/Cppcheck/ESLint tools. Patch-capture tests
cover Dune and upstream OCaml test-generated directories. Report tests also
verify that null/missing functional receipts cannot count as failures or coverage.

## Model source-patch reproducibility

All 21 current original-task source patches reproduce their functional scores and
exact failed-check vectors in fresh offline workspaces. Three older Async patches
include Dune build artifacts; explicitly derived source-only copies exclude those
artifacts and still pass. The other 18 patches are byte-identical to their originals.
The saved receipt is [MODEL_SOURCE_REPLAY.json](MODEL_SOURCE_REPLAY.json), with
engineering observations in [MODEL_PATCH_REVIEW.md](MODEL_PATCH_REVIEW.md).
This is supplemental patch replay, not fresh model calibration or independent
confirmation. The historical 17/21 result and withheld expanded score are unchanged.

## Reference audit and revision

An additional audit found that the initial Incremental reference crashed when a
valid transaction reversed two dependencies simultaneously. The issue already
required atomic validation against the complete prospective graph, so this was a
reference defect and a coverage gap. The reference now builds immutable graph
generations, preserves subscriptions, and memoizes arithmetic across generations.
Named nodes are created in the top scope to allow repeated dynamic selection.

Two new fail-to-pass checks cover atomic dependency reversal and repeated dynamic
branch switching. All seven behavior checks and both regression guards pass in
three fresh reference repetitions. The original model patches were reverified
against the added cases as a retrospective diagnostic; those results are not
fresh model attempts. Revised-task calibration uses its new scoring hash.

## Collection-work revision

The first complete development calibration scored exactly 80% (12/15 complete
passes), preserved in [CALIBRATION_INITIAL.json](CALIBRATION_INITIAL.json). The
collection task passed all three initial attempts, so it was revised to add an
explicit comparator-work and native-allocation contract. The issue now states
the fixture size, measurement conditions, and limits; fresh attempts use the new
scoring hash. No model configuration or time ceiling was weakened.

The persistent tree reference uses 37 to 57 comparisons and 3320 to 3680 allocated
bytes per measured local edit, repeated three times. The correct-but-inefficient
control passes all five original behavior checks and every regression guard,
but fails both newly required efficiency checks. It uses 12290 comparisons and
541136 allocated bytes on the first edit. Bounds are 512 comparisons and less
than 32768 bytes, with a separate 64-comparison lookup limit. These measurements
bound the named operations; they are not a proof of every aspect of runtime
complexity or a wall-clock performance claim.

## Remaining limitations

- Seven tasks are small original fixtures; one uses a substantial upstream
  compiler repository. Broader OSS task coverage and calibrated task selection
  remain necessary for the full suite.
- Fixed-interface checks prevent changing supplied signatures. They do not by
  themselves prove the absence of unsafe casts or establish maintainability;
  submitted patches still need inspection for those requirements.
- The original fixtures have public smoke tests plus examples in the issue.
  The compiler task scores its affected upstream expect workflow. Richer public
  workflows remain an expansion goal for the other task families.
- OxCaml extensions are outside this pilot.

## Upstream compiler validation

The actual OCaml compiler source is pinned to revision
`364874344779d2e410e99eb634bea8e7e2b70159` (5.6.0+dev0). The complete source
snapshot contains all 4984 tracked files, including the public test corpus.
PROVENANCE.json pins the full public reference, source archive, build seed, and
both images. The verifier is
`sha256:8ff6b3901a42211033f049d3192392acd06004082c3cac1ec9f7028062aed301`.

The revised nine-check definition passed three clean base/reference pairs,
three compiling faulty controls with all four guards intact, and the standard
validator. Its public workflow executes all four affected upstream tests.
Earlier fresh public-test workspaces also passed the new reference GC regression.
All supplied interfaces and 152 tracked runtime
inputs remain guarded. Env may add helpers while retaining old declarations.
Named failure receipts preserve earlier source-archive and build-cache problems.
No assertion was removed to get a valid reference. These results do not certify
the entire upstream test suite.

A supplemental fresh offline reference workspace configured and built world.opt
without installing the baseline seed. The full build passed in 1048.8 seconds
under emulation; all nine unchanged assertions and the new public GC regression
then passed. [COLD_REFERENCE.json](compiler-gadt-field-safety/COLD_REFERENCE.json)
preserves commands, outputs, timings, and the same frozen task hash. This is one
cold full build, not three cold repetitions or a hardware-performance claim.

The solver's generated `_ocamltest` and `_ocamltestd` directories are excluded
from captured patches along with Dune's `_build`; all three patch-capture cases
passed focused tests. The preserved core-only model replay exposed the public
snapshot gap that prompted the new workflow guard. Fresh revised-task model
calibration and replay remain outstanding; the old replay is not a new score.
