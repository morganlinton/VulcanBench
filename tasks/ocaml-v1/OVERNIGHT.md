# Overnight OCaml suite work

Authorized by the owner on September 30, 2026: continue improving and testing
the OCaml suite overnight. Stop the overnight automation by 9 AM PDT October 1.

## Current operational state

The daytime native sweep completed all 21 library-task attempts at 2026-10-01
19:09 UTC (12:09 PM PDT). It scored 19/21 complete passes (90.5%): six tasks
passed 3/3 and the incremental graph passed 1/3. Both failures were on atomic
dependency reversal. Receipt audit at 21:29 UTC verified all run budgets,
versions, regression guards, integrity flags, and clean patch inventories.
NATIVE_CALIBRATION.json/md preserve the result and source observations. No
new model calls ran during the audit. The dispatcher has exited, and the
compiler remains pending; the eight-task score is still withheld. The following
launch and pre-sweep states are historical.

The overnight effort has ended. A separate daytime calibration was authorized
by the owner after asking for next steps and is active from 2026-10-01 17:56 UTC.
Its output directory is runs-ocaml-native-20261001T175608Z. LAUNCH.json and
PROGRESS.json there record the actual conditions and live state. Seven library
tasks have three fresh serial attempts queued; the compiler is not queued while
its provider refusal remains unresolved. The dispatcher uses the pinned native
CLI, full task-default budgets, and no Docker. It checks validated code and
scoring digests before every attempt and stops on invocation errors without an
automatic retry. JSON/HTML development cards refresh after each completed
attempt, with the full eight-task headline withheld. Native setup receipts and
the historical zero-attempt readiness snapshot below remain unchanged.

Final handoff audit at 2026-10-01 15:23 UTC: MEASUREMENT_PLAN now uses the current
eight-task arithmetic, documents the limits of local/Docker report filtering,
and directs subsequent reports to new paths rather than overwriting receipts.
HANDOFF_AUDIT.json confirms that every CI-bound code digest and the suite scoring
identity remain unchanged. No model call or additional validation run was
needed. Full execution-condition equivalence still requires manifest and launch
receipt review before publishing a model card.

As checked at 2026-10-01 15:08 UTC (8:08 AM PDT), native Mac execution is ready
for development runs. All eight tasks passed both a full host gate and a
separate full offline gate: three fresh base/reference pairs per task, all
regression guards, all 15 compiling faulty controls rejected, and standard
validation PASS. NATIVE_VALIDATION.json/log, NATIVE_VALIDATION_OFFLINE.json/log,
and NATIVE_ENVIRONMENT.json preserve the proof. No task scoring hash changed.
Compiler setup uses portable paths and a separately built Darwin arm64 public
pre-fix seed; no hidden tests or reference patch are in the seed.

Final CI passed 1092 tests, five unrelated analyzer skips, four deselections,
86.39% coverage, lint/format/no-em-dash, and 117-file mypy. CI_NATIVE_FINAL.json/log
preserve code digests and checks. Earlier CI and setup receipts remain intact.
Validation session 67008 and CI session 47030 completed successfully. No workload from this native setup remains active or queued. No Docker model
or validation workload was started during this phase.

README and MEASUREMENT_PLAN document native provisioning, serial local runs,
and functional JSON/HTML cards. The native Codex 0.159.0 package is preserved
under ~/.local/share/vulcanbench/ocaml-v1-native/codex-cli (no auth copied).
NATIVE_RUN_PLAN.txt is a 24-attempt dry run, not model execution or a measured
subscription cost. New summaries record effective solve time and configured
step allowance. Reports exclude altered budgets, stale tasks, integrity flags,
and other selected environments; incomplete coverage has no headline score.
Trace-only aborted invocations remain visible audit context, never invented
failures or coverage. HTML data/escaping are tested; visual rendering remains
unverified because the in-app browser blocked local-file previews. Embedded
font licenses are preserved in the card and asset tree.

No fresh native model calls ran because full solve/grading budgets do not fit
before the owner's 9 AM stop. Historical seven-task calibration remains 17/21
(81.0%), and the current eight-task score remains withheld. The under-80% target
is unproven. The revised compiler has zero fresh model outcomes, and its earlier
provider refusal is unresolved. Do not rephrase its frozen prompt, route around
the refusal, shorten budgets, or count replay as fresh model calibration.

The suite remains an eight-task development pool, with no independent
confirmation and no OCaml quality/security factors. Public upstream retrieval
and requested-only model identity limits remain disclosed. Current native
readiness does not establish a stable leaderboard or publication-quality visual
card. Keep the eight scoring definitions frozen for subsequent model comparisons
and stop the overnight automation by 9 AM PDT (16:00 UTC) October 1.

## Native readiness work history

The following entries are historical; active-session and setup statements below
are superseded by the completed state above.

At 2026-10-01 14:53 UTC, native offline compiler controls are all rejected
with regression guards intact after all three baseline/reference pairs. Its
standard validator is finishing in session 67008. All seven library tasks have
already passed the full offline gate. The final frozen-code CI is session 47030,
writing /tmp/ocaml-native-ci-limits.log. Do not change code during that CI run.

Fresh run summaries now record effective agent timeout and configured step
allowance. OCaml reporting excludes budget override ablations and recorded
limits that differ from task defaults. Historical summaries are preserved and
lack these fields, so their launch receipts remain the budget evidence. Focused
agent/report tests passed. CI_NATIVE_REPORTING.json/log preserve the preceding
1090-pass CI; the final CI should include two additional budget-cohort tests.
Only documentation and final receipt recording remain after the active jobs.
No model calls are queued and no Docker workloads were started.

At 2026-10-01 14:41 UTC, the no-network native gate has passed all seven
library-task gates and two compiler base/reference pairs. Session 67008 remains
active. A final CI run also started after the validation offline CLI refactor,
font-license preservation, and trace-only invocation reporting were complete;
its receipt will be saved separately as CI_NATIVE_FINAL.json/log. No task
scoring hash changed and no model call ran.

The card preview now preserves two trace-only aborted invocation identities
(one old compiler provider refusal and one historical graph invocation). They
are visible audit context, never scored failures or coverage. Effort, revision,
and environment are not invented when a summary is absent. Fresh native runs
remain a separate zero-attempt cohort. Final card rendering is still visually
unverified because local-file browser navigation was blocked.

As of 2026-10-01 14:30 UTC, all eight native host gates passed: three clean
base/reference pairs per task, every regression guard, all 15 compiling faulty
controls rejected, and standard validation PASS. NATIVE_VALIDATION.json/log
preserve this first host run without network enforcement. Session 26348 exited
successfully. A separate full native no-network gate is active as session 67008,
writing NATIVE_VALIDATION_OFFLINE.json and /tmp/ocaml-native-validation-offline.log.
Do not start model calls; their full budgets do not fit before the stop deadline.

The pinned Darwin Codex 0.159.0 public package files are preserved under
~/.local/share/vulcanbench/ocaml-v1-native/codex-cli (no auth copied). Native
24-attempt sweep planning passed as NATIVE_RUN_PLAN.txt. The generic dry-run
API-credit estimate is not a subscription billing receipt. The earlier compiler
provider refusal remains unresolved; do not route around it or change its prompt.
README and MEASUREMENT_PLAN now document native provisioning, serial local
execution, and functional JSON/HTML cards with complete-coverage gates.

Native readiness work at 2026-10-01 14:11 UTC: Homebrew opam/pkg-config installed,
all TOOLCHAIN.lock package versions matched in a dedicated local root, and a
Darwin arm64 public pre-fix compiler seed built successfully. Native environment
and seed digests are preserved in NATIVE_ENVIRONMENT.json. The compiler setup
now accepts an explicit helper path and uses its working directory. No prompt,
reference, scoring assertion, guard, or task hash changed.

Native validation session 26348 passed the full seven-library-task gates and
three compiler base/reference pairs; compiler controls and standard validation
are still running. This initial host gate did not enforce network denial. Run a
separate complete gate with --sandbox local --offline after it finishes; the
macOS network-denial probe rejected a socket connect with PermissionError.
Do not pool this environment with historical Docker model scores.

CI session 93486 passed 1089 tests, five unrelated skips, four deselections,
86.39% coverage, lint/format/no-em-dash and 117-file mypy. CI_NATIVE.json/log
preserve it. Final offline CLI refactoring followed the CI start; final lint and
actual offline gate remain necessary. Reporting now accepts model/effort and
sandbox selection and can write a functional HTML model card. Preview JSON/HTML
are generated from the incomplete historical Docker calibration with the
headline correctly withheld. Local-file visual browser preview was blocked by
browser URL policy; no bypass was attempted. HTML semantics/escaping are tested,
but visual rendering is unverified. No model call or Docker workload was launched
in this native setup. Full model budgets do not fit before the overnight stop.

Owner steering at 2026-10-01 13:44 UTC: requested readiness for model testing
and model cards, and prefers execution directly on this Mac because Docker uses
too much memory. Prefer native execution for subsequent owner runs. The harness
already supported `--sandbox local --no-agent-container`, but at that inspection
this Mac had no `opam`, `ocamlc`, or `dune` on PATH. Compiler setup also assumed
`/usr/local/bin/ocaml_compiler_seed.py` and `/workspace`; its Linux build seed
cannot be reused as a native macOS seed. Native toolchain provisioning,
portable compiler setup, and clean native reference/control validation remain
required native readiness work at that inspection. Do not launch another
Docker model sweep in response to the readiness question. No model call or
installation was made during this inspection.

Eight tasks are runnable with the validated Docker environments as a development
pool. Published results must disclose development status, functional-only
scoring, and complete current-hash coverage. At the 13:44 inspection the reporting
CLI selected GPT-6.1 Sol medium without model/effort selection flags, and existing
Frontier report-card renderers were suite-specific. Native readiness work above
adds those flags and a functional HTML card. The historical 81.0% and incomplete
expanded calibration below remain unchanged.

As of 2026-10-01 13:02 UTC (6:02 AM PDT), all eight tasks are active and
validated. The revised compiler task passed three clean base/reference pairs,
three compiling semantic controls with all four guards intact, and standard
validator PASS. It was readmitted at
`tasks/ocaml-v1/compiler-gadt-field-safety`, frozen hash 0e66b01f. The suite has
50 behavior checks, 20 regression/interface/workflow guards, and 15 rejected
compiling controls. VALIDATION_WORKFLOW.json preserves the new compiler gate;
VALIDATION.json merges all eight proofs and records source/control hashes.

Seven original tasks have historical development calibration 17/21 (81.0%),
so the difficulty target remains unmet. The current eight-task aggregate is
withheld. The revised compiler has zero fresh model outcomes; its old pass is
explicitly excluded under its old hash. Provider refusal remains unscored.
No compiler model workload is queued, and its full-budget dispatch cutoff passed.

The old core-only compiler definition passed the complete reference/control gate
and fresh public base/gold tests. Its first model attempt passed all eight scored
checks. A clean patch replay agreed with that score, but the existing public
basic/patmatch_split_no_or.ml expect test failed because the model left its
snapshot unchanged. The other three affected public tests passed. The issue
already required maintaining public expectations. The revised grader adds a
public_expect_workflow guard, running all four affected existing tests; prompt,
reference, starting source, model settings, and solve budgets are unchanged.

Preserved core-only artifacts: suite-level CALIBRATION_COMPILER_CORE_ONLY.json,
VALIDATION_COMPILER_CORE_ONLY.json, SUITE_COMPILER_CORE_ONLY.json; task-level
METADATA_CORE_ONLY.json, PROVENANCE_CORE_ONLY.json, CORE_ONLY_TESTS,
MODEL_PATCH_REPLAY.json, and old validation/public receipts. The old scored pass
is diagnostic evidence, not a fresh failure on the new hash. The second model
invocation was a provider cybersecurity content-filter refusal with no score,
preserved as PROVIDER_REFUSAL.json and raw run/launch artifacts. No third call
ran. Do not rephrase the frozen prompt or route around the filter. No model
workload is active and the overnight compiler dispatch cutoff has passed.

All three controls passed public expect tests after expectation-only updates;
program bodies were preserved. CONTROL_PUBLIC_EXPECTATIONS.json and original
CORE_ONLY_CONTROLS remain. Strict session 90057 completed successfully, and the
renewed admission guarded every verdict name and all provenance before moving
the task. The original admission script remains obsolete.

Finishing session 10720 completed successfully. Full CI passed at 11:57 UTC:
1084 passes, five unrelated analyzer skips, four deselections, 86.38% coverage,
and successful lint/format/no-em-dash/117-file mypy. CI_WORKFLOW.json/log preserve
this result in the suite. Current narratives, README, and local links are checked.
The build targets now include both OCaml agent/verifier image prerequisites.

The supplemental cold reference audit completed successfully at 12:16 UTC.
It configured and built world.opt without installing the public baseline seed
(1048.8 seconds), then passed all nine unchanged assertions and the new upstream
GC regression. COLD_REFERENCE.json/log preserve commands, outputs, timings, and
the current task hash. This is one cold full build, not repeated-cold determinism
or full-upstream-corpus coverage. All validation/CI workers are finished; no model
workload is active or queued.

A supplemental source replay also completed for all 21 current original-task
model patches. Every functional score and exact failing-check vector reproduces.
Three older Async patches contained 33 generated Dune paths each; derived source
copies excluded only those paths and still passed. The other 18 source patches
are byte-identical to originals. Raw model patches and scores remain preserved.
MODEL_SOURCE_REPLAY.json/log and patch-audit/ preserve the new proof and derived
patches. MODEL_PATCH_REVIEW.md/json record inventory and qualitative library-code
inspection of one representative per family plus the failed cache implementation.
This is source reproducibility and review, not new model calibration or an
independent confirmation. All current task hashes still match.

Do not launch further compiler calls overnight or count retrospective replay or
refusal as new scores. Next work after CI: finish the reviewable handoff, preserve
receipts, record remaining difficulty/coverage limits, and stop by 9 AM PDT.
The sections below preserve earlier superseded history and session references.

## Original five-task checkpoint

The five-task pilot is validated, with 29 behavior checks, 11 regression guards,
and seven rejected incomplete-solution controls. Development calibration of
GPT-6.1 Sol at medium effort has 12 passes in 15 fresh attempts (80%). The
strictly-under-80 target is not yet met. Incremental graph dependency reversal
fails all three attempts; the other four tasks pass all three.

The working branch is `codex/ocaml-v1`. Changes are uncommitted and reviewable.
Both OCaml Docker images are built. Full CI last passed with 1055 tests passing.
See CALIBRATION.md, VALIDATION.json, and MEASUREMENT_PLAN.md for evidence and
limitations. Preserve the existing reports and run receipts when adding tasks.

## Next work

Build deeper tasks combining OCaml type contracts with engineering concerns.
Prefer useful public Jane Street library usage and substantial repository work.
Keep original fixtures clearly labeled and verify all provenance claims.
Validate each task with three clean base/reference repetitions and plausible
compiling incomplete-solution controls before model measurement.

Use the existing GPT-6.1 Sol medium configuration, three fresh attempts per task,
serial concurrency, and the same three-hour ceiling. Do not use ultra, weaken the
model, tighten the timeout, hide requirements, or discard inconvenient results.
The existing development calibration is not independent confirmation.

Use `.venv/bin/python`, because system Python is too old for the harness.
The isolated Codex 0.159.0 executable is at
`/tmp/vulcanbench-codex-0.159.0/node_modules/.bin/codex`.
Set `VULCANBENCH_AGENT_IMAGE=vulcanbench/agent-codex:ocaml-v1` and run with
`--agent-container --harness codex --billing subscription --model gpt-6.1-sol
--effort medium --repeat 3 --max-concurrency 1 --no-judges --only-missing`.
Run output belongs in the ignored `runs-ocaml-pilot` directory. Do not inspect
or expose staged authentication files. Never modify task inputs during a batch.

No messages to third parties or subagents are authorized. Follow AGENTS.md and
CLAUDE.md, including the prohibition on em and en dashes. Update this checkpoint
with concrete results, open questions, and the next independent action.

## First overnight expansion

The candidate pool now includes `typed-query-optimizer`. This is an original
typed DSL, with capture-avoiding rank-polymorphic renaming and substitution,
literal beta reduction, dead binding removal, effect-sensitive projections,
and preservation of sharing. Seven behavior checks include 1500 deterministic
generated programs, result/effect equivalence with injected exceptions, and
structural idempotence. Three guards preserve the interpreter, fixed interfaces,
and valid/invalid compiler clients. Two controls drop projection effects or
ignore nested binding depth. The reference passed an initial complete run.

Strict three-repetition validation passed, with output at
`/tmp/vulcanbench-ocaml-optimizer-validation.json` and its adjacent `.log` file.
The standard validator also passed. Both controls compiled and preserved all
guards. Dropped projection effects failed projection and generated-equivalence
checks; incorrect binding-depth tracking failed the dead-binding check.
All six tasks are now represented in VALIDATION.json (36 behavior checks,
14 guards, nine rejected controls). The five-task report and
manifest are preserved as CALIBRATION_FIVE_TASK.json and SUITE_FIVE_TASK.json.
The expanded manifest's calibration is currently incomplete. Rebuild the report
after validation and after every completed calibration batch.

A serial three-attempt calibration batch completed for the optimizer only,
using the existing five tasks' matching cached attempts. Its log is
`/tmp/vulcanbench-ocaml-overnight-calibration.log`; unified-exec session 67844 was
returned at launch. All three attempts passed, taking 238 to 274 seconds each.
The expanded six-task development score is 15/18 (83.3%), with target unmet.
The preserved report is CALIBRATION_SIX_TASK.json. The three patches passed a
basic unsafe-marker scan; this is not a maintainability or safety certification.
Before any later launch, check the log and whether a
`vulcanbench run --suite ocaml-v1` process is still active. Do not start a second
batch or change any task inputs while this one is active. When it finishes,
rebuild CALIBRATION.json and update CALIBRATION.md with all fresh outcomes.
Keep development results distinct from independent confirmation.

## Second overnight expansion

`async-generation-cache` is the seventh original candidate. It uses the actual
pinned Async_kernel Time_source and Monitor APIs. Its nine behavior checks cover
coalescing and lazy TTL, overlapping invalidated generations, per-subscriber
cancellation, retry limits, generation deadlines, closure across detached work,
synchronous reentrancy, asynchronous/synchronous exceptions, and timer ownership.
Two guards preserve Policy/Request/empty service behavior and all supplied
interfaces. The starting library has five modules, including a batch Client.

An initial complete reference run passed every check. Three controls permit stale
publication, cancel all coalesced subscribers, or leave obsolete time-source
alarms registered while claiming zero pending timers. The first strict run found
an unused-field warning in the starting Alarm stub, before any model calls.
The stub was repaired and its reference/control patches regenerated. Strict
fresh validation then passed in session 65120, with receipts/logs at
`/tmp/vulcanbench-ocaml-generation-cache-validation.json` and `.log`.
Every base/reference check passed three repetitions; all three controls compiled,
preserved guards, and were rejected. The standard validator passed. Receipts are
merged into VALIDATION.json: seven tasks, 45 behavior checks, 16 guards, and
12 rejected controls.

The cache's serial three-attempt model batch is running in session 39590, with
log `/tmp/vulcanbench-ocaml-generation-cache-calibration.log`. Never overlap
launches or change the task inputs while it runs. After completion, rebuild the
current report and audit all matching patches. Preserve every outcome.

The manifest now has seven candidates, so CALIBRATION.json correctly withholds
an aggregate while the cache has zero attempts. The prior six-task report is
preserved. Do not mistake the six-task 83.3% result for current complete coverage.

The report now includes engineering and language views, with distinct-task and
attempt denominators. Tasks tagged both enter both views but only once in the
suite. A view score is withheld unless every member has three attempts; a fully
covered view can be shown when another view is still incomplete. Failing check
names distinguish behavior and regression failures. Six report tests, Ruff,
and report-script mypy passed.

Harness report tests passed (three tests), Ruff passed, and report-script mypy
passed after making the report's task-count limitation reflect the actual pool.
The first focused pytest command inherited the full-project coverage threshold,
so it exited for low aggregate coverage despite passing its three selected
tests. The focused run was repeated with --no-cov and passed. The earlier full
CI result remains valid for the unchanged integration code.

Source research checked public GitHub issue/PR evidence for Base, Core,
Async, Incremental, bin_prot, and their PPX libraries. bin_prot issue 33 and
PR 34 describe context-aware deserialization, but were closed without a merge;
the draft explicitly lacked test evidence. They are a potential original
migration-task lead, not an admitted upstream fix. The historical Core_kernel
union-find PR 57 demonstrates GADTs and allocation improvements but is from
2016 and unmerged. Its age and current toolchain mismatch make it a poor
decontamination candidate. No verified OSS repair has been admitted yet.

## Verified OSS source under investigation

OCaml PR 15114 fixes field classification when a pattern row's GADT equations
are used for shared field accesses. It merged September 30, 2026. The unsafe
classification can omit a native GC root for a string. The proposed task would
use actual compiler source and span matching, type-environment handling, native
GC behavior, and preservation of immediate-field optimization. This is more
substantial repository work than another small fixture.

Verified provenance files are `/tmp/ocaml-gadt-field-source.json` and
`/tmp/ocaml-compiler-source-15114-pr.json`, with the full diff/issue API records
in adjacent files. The base revision is
`364874344779d2e410e99eb634bea8e7e2b70159`; the reference head is
`a47e535288e8e713a4c2ebf9b93d5a069057a3c4`. The snapshot has 1633 files and
version 5.6.0+dev0. Preserve its LGPL 2.1 license and linking exception. Recent
publication is not proof of model training cutoffs or absence of public-patch
retrieval. Keep those limitations explicit before admission.

The base source is at
`/tmp/ocaml-gadt-field-source/ocaml-364874344779d2e410e99eb634bea8e7e2b70159`.
An offline, 2-CPU, 2-GiB compiler build probe is running in session 25361 and
container `vulcanbench-ocaml-source-probe`. Its log is
`/tmp/vulcanbench-ocaml-compiler-probe.log`. It runs configure and make -j2
world.opt, without changing the existing sandbox or task manifest. Check its
result before further work. If viable, design independent native-GC and field
classification tests, compiling controls, and a reproducible warmed baseline
that contains no gold patch or hidden tests. Do not admit this source task until
its complete reference/guard/control validation succeeds. Budget any compiler
build setup separately from solve difficulty and document run-condition changes
in docs/DECISIONS.md. Do not weaken the existing solver configuration or budget.

## Seven-task results and compiler candidate checkpoint

The cache batch completed: two passes and one failure, with all three fresh
receipts preserved. Its failed attempt attached a new subscriber to an overdue
generation before applying the lazy deadline rule. The complete seven-task
development score is 17/21 (81.0%), with the under-80 target unmet. The report
and manifest are preserved in CALIBRATION_SEVEN_TASK.json and
SUITE_SEVEN_TASK.json. CALIBRATION.json and narrative reports now reflect
completed seven-task coverage. No independent confirmation has run.

The offline compiler probe completed successfully. The first compiler image
also built successfully, and a direct offline baseline check failed all five
proposed behavior checks while passing all three guards. Candidate files are
at `tasks/ocaml-candidates/compiler-gadt-field-safety`, outside the active suite.
The full upstream PR diff is the private reference. Three controls cover a
record-only fix, blanket pointer classification, and retained row-local record
equations. Nine tests of the seed utility passed, with Ruff and utility mypy.

The initial strict validation failed before reference execution: the GitHub
release archive has only 1633 files and excludes public upstream tests that the
full reference patch updates. This packaging failure is preserved in the
candidate's VALIDATION_ARCHIVE_MISSING_TESTS.json. No model attempt was made.
The candidate now uses all 4984 tracked files from a depth-1 checkout at the
pinned base revision. Source mtimes use the pinned commit timestamp, and gzip
metadata is deterministic. Its snapshot and reference hashes are recorded in
PROVENANCE.json; original LGPL and other source notices are preserved.

Strict validation of the complete snapshot is running in session 70594, with
log `/tmp/vulcanbench-ocaml-compiler-full-validation.log` and receipt path
`/tmp/vulcanbench-ocaml-compiler-full-validation.json`. Check completion before
changing tests or candidate inputs. This validator is serial and repeats fresh
base/reference checks three times, then controls and standard validation.

A second compiler verifier image build is running in session 96496, log
`/tmp/vulcanbench-ocaml-compiler-image-v2.log`. It adds exclusion of generated
artifacts from captured model patches. When finished, rebuild the agent target
from `sandbox/Dockerfile.ocaml-compiler` without an explicit platform argument:
the local base manifests reject an explicit linux/amd64 request even though
both resulting image configurations report amd64. That failed build is logged
at `/tmp/vulcanbench-ocaml-compiler-agent-image.log` before a successful retry.
The existing first agent compiler image is built but does not yet have the
patch-exclusion update. Revalidate against the final image before calibration.

The Dockerfile-specific ignore file permits only the seed utility in the build
context. Root .dockerignore otherwise excludes scripts and initially prevented
COPY; this was fixed without broadening the root context. Both images contain
only public pre-fix artifacts, with no hidden tests or gold patch. Compiler checks
use 600 seconds for rebuild headroom; the solve ceiling and model settings are
unchanged. The decision and separate compiler-under-test version are recorded
in docs/DECISIONS.md. After all gates pass, move the candidate into ocaml-v1,
merge validation receipts, preserve the seven-task report, then calibrate three
fresh serial attempts using the compiler agent image. Withhold an expanded
aggregate until complete coverage. Public-patch retrieval remains an explicit
limitation of this OSS source.

The complete-snapshot validation also failed during gold setup, before grading.
Its receipt is preserved as VALIDATION_FULL_SEED_V1.json. A direct diagnostic
rebuild reproduced `Unbound module Types` in typing_recovery.ml with -j2;
the single-job retry completed. The cause is not established. Use -j1 builds
and 1200-second setup/check headroom for this candidate, then require a new
three-repetition clean validation. Do not treat partial retry completion as a
valid reference receipt. The decision is recorded in docs/DECISIONS.md.

Final verifier and agent compiler images are built, with identities
`sha256:8ff6b3901a42211033f049d3192392acd06004082c3cac1ec9f7028062aed301`
and `sha256:473af5a7da6130015ceb61eeec034b966e93de633a868d21e3f29059013d5f5f`.
They contain the patch-exclusion update. Additional hidden coverage checks
abstract immediate annotations and both tuple/constructor row orders. The source
guard also preserves all 152 tracked runtime inputs, as required by the issue.

Full CI passed in session 83330: 1071 tests passed, five unrelated analyzer skips,
four deselections, and 86.38% coverage. Lint and 117-file mypy passed. The validator
now records setup timing and output, including failed setup receipts, and retains
a completed baseline result before attempting gold. Its lint and mypy passed.

The diagnostic gold workspace is `/tmp/ocaml-compiler-gold-debug`; its complete
initial behavior/guard check log is `/tmp/vulcanbench-ocaml-compiler-gold-checks.log`.
This is a reused debugging workspace, not a clean validation receipt. Next: check
those results, repair any grader defect, then run strict validation against the
final images and unchanged candidate inputs before admission or model calls.

The initial diagnostic gold passed all eight current hidden behavior/guard checks,
including abstract immediate annotations and runtime preservation. Five public
upstream tests also passed after supplying `OCAMLSRCDIR=/workspace`. Without this
override the relocated public build of ocamltest searched its original absolute
build path. The failed and corrected public-test receipts are preserved in the
candidate as PUBLIC_TESTS_RELOCATION_FAILURE.json and PUBLIC_TESTS_GOLD.json.
The issue's public-test recipe was clarified with this required environment
override. No assertion or grading contract changed in that clarification.

Fresh single-job strict validation is now running in session 43852, log
`/tmp/vulcanbench-ocaml-compiler-serial-validation.log` and report
`/tmp/vulcanbench-ocaml-compiler-serial-validation.json`. It uses the final
compiler images and current behavior/interface/runtime checks. Wait for complete
three-repetition reference/control validation before admission. The five public
upstream checks are supplemental diagnostic evidence from a reused gold
workspace, not replacements for the fresh validation gate.

Fresh single-job base/reference repetition 1 passed. The strict validator remains
active; do not overlap another validation or model batch. The candidate's
ADMISSION.md maps requirements to tests and records the remaining gate.
The agent and verifier seeds have identical artifact SHA-256
`f9d630894c2985f5b595d35efe2de9b3ec335001bcabb6e6f9821408ff1a1ef2`.
The public seed has 4735 generated artifacts; its matching.ml and env.ml source
hashes match the pinned Git checkout. These identities are in PROVENANCE.json.

The morning stop takes precedence over launching more work. Do not start a new
attempt unless the full three-hour solve ceiling plus setup/grading headroom
fits before 9 AM PDT. If fewer than three fresh attempts can finish, retain the
partial receipts and withhold the expanded aggregate. Do not shorten the solve
budget, score a user-deadline interruption as capability failure, or launch work
that knowingly extends the overnight effort past the stop time.

Checkpoint saved 2026-10-01 07:48 UTC (12:48 AM PDT). The current suite is
seven calibrated original tasks at 81.0%. The compiler candidate remains outside
the manifest. Strict validation session 43852 is active after repetition 1
passed; check its log/report before launching anything else. No compiler model
attempt has been launched. All changes remain uncommitted on codex/ocaml-v1.

## Final compiler build-target revision

At 08:11 UTC all three fresh single-job base/reference repetitions had passed.
The blanket-pointer control then failed while linking unused ocamldoc: cached
Unix and rebuilt Stdlib__Printf had inconsistent implementation digests. This
was an incremental build-cache problem outside the compiler task, not rejection
by the intended behavioral tests. The full receipt is preserved in the candidate
as VALIDATION_OPT_OPT_CONTROL_FAILURE.json, including all six completed
base/reference runs and failed setup output.

The documented setup/grade recipe now builds only the core compiler, standard
library, both native frontends, and the public expect helper:
`make -j1 coreall opt-core ocamlc.opt ocamlopt.opt testsuite/tools/expect`.
No assertion, interface/runtime guard, compiler feature, model setting, or solve
budget changed. A complete gate with this final recipe is running in session
95604. Log/report paths are `/tmp/vulcanbench-ocaml-compiler-core-validation.log`
and `/tmp/vulcanbench-ocaml-compiler-core-validation.json`. Do not reuse the old
recipe's repetitions to bypass validation of the new commands.

An audit helper also used an overly broad `Error:` substring check. Python's
`AssertionError:` from expected behavior failures would falsely look like an
OCaml compiler rejection. This was found by inspection, not used to admit a
control. The helper now recognizes line-start OCaml Error diagnostics, including
fatal warnings and ANSI coloring. Eight focused tests, Ruff, and validator mypy
passed. Task grading itself is unchanged. Preserve original validation receipts.

The current calibration report now exposes source, upstream provenance, and
decontamination notes per task. A public-patch exposure test verifies that this
disclosure neither excludes an outcome nor changes its weight. The report,
validation-audit, and compiler-seed test files passed 24 focused tests together;
report Ruff and mypy passed. The seven-task score remains 17/21 (81.0%).

The new compiler source snapshot was covered by the repository's generic archive
ignore. Added narrow exceptions for OCaml active/candidate repo snapshots, using
the existing generic Git LFS attribute. The 7-MiB pinned snapshot is now visible
to Git and reviewable. No task grading input changed during the current gate.

Three clean base/reference pairs now pass with the final core/public-helper
recipe (about 19 minutes for all six workspaces). The three compiling controls
and standard validator are still running in session 95604. No model calls yet.
Compiler calibration will use three single-attempt dispatches, with unchanged
model, effort, resources, and full solve budget. Reserve six hours and fifteen
minutes before each start for the complete configured setup/solve/grading bounds;
last allowed dispatch is 09:45 UTC (2:45 AM PDT). See docs/DECISIONS.md. This may
leave coverage incomplete, which must not become a below-80 claim.

All three final-recipe controls now compile, preserve guards, and fail their
intended behavior checks. The standard validator is still running. After it
passes, run `/tmp/validate_ocaml_compiler_public.py` with `PYTHONPATH=$PWD` to
check four affected public tests at fresh base and those four plus the new GC
regression at fresh gold. This strengthens the earlier reused-workspace public
receipt without changing grading inputs. Its log/report paths should be
`/tmp/vulcanbench-ocaml-compiler-public-clean.log` and adjacent `.json`.
`/tmp/admit_ocaml_compiler.py` is prepared but has NOT run; it asserts both gates
pass before preserving their reports, moving the task, and expanding the active
manifest. No compiler model attempt has been launched.

A conditional public-test launcher is waiting in session 33322. It runs
`/tmp/validate_ocaml_compiler_public.py` ONLY after the complete core JSON says
valid and the standard validator says PASS. While core validation is active it
only waits, so no second validation/model workload overlaps. It refuses to
dispatch after 09:25 UTC. Public log/report paths are the ones above. Check both
sessions and reports before any admission/model call. The prepared admission
script remains unexecuted.

`/tmp/calibrate_ocaml_compiler_serial.py` is prepared but NOT launched. After
admission and narrative updates, run it with `PYTHONPATH=$PWD`; redirect output
to `/tmp/vulcanbench-ocaml-compiler-calibration.log`. It dispatches one fresh
single-task CLI attempt at a time until three matching receipts exist, using
the compiler agent image and existing GPT-6.1 Sol medium settings. It reserves
six hours and fifteen minutes before each start, refuses drift, stops on CLI/
infrastructure failure, preserves `/tmp/vulcanbench-ocaml-compiler-launches.json`,
and regenerates the full calibration report after each completed call. Never
launch it before both validation reports pass and the candidate is admitted.

The serial launcher started at 09:15:22 UTC in session 24397, host PID 91204.
Its first single-task CLI child was PID 91206. Inputs are frozen at scoring hash
`d77a74a0e7702725f23d050f11390fa8b30cc44e48151aa9e146d4225aa9fe9c`.
Its log and launch JSON show the current attempt. Public/core gates are done,
so no validation workload overlaps it. Task metadata was finalized before
launch and must not change until every dispatched attempt ends.

The patch-replay utility uses the optional `initialize_git=True` exercise mode
so final.patch applies against the exact harness Git baseline, including its
ignore rules. This mode defaults off for ordinary reference validation; grading
checks are unchanged. Patch capture now ignores `_ocamltest` and `_ocamltestd`
alongside `_build`. Three focused integration cases passed; Ruff formatting,
Ruff lint, and mypy passed for the loop/validator changes. The source archive
and all task grading inputs are unchanged. Both reference and model result
replay remain supplemental to the primary saved receipts.

The model replay audit also runs the four affected existing public tests as
supplemental diagnostics, without changing the original scored eight checks.
It separately records primary replay consistency and public regression health.
If public snapshots were not maintained, retain the original functional receipt
and disclose the engineering gap; do not relabel retrospective diagnostics as
fresh model failures. A scored grading revision would require fresh references,
controls, and model attempts on its new hash. The utility's optional supplemental
commands default empty; ordinary validation behavior is unchanged. Ruff and mypy
passed after this addition.

A deadline audit establishes the configured solve-plus-verification maximum at
5 hours 20 minutes: `_verify_with_budget` caps each command by the remaining
10800-second deadline once. Add 20-minute setup and about 20-minute orchestration
margin for a six-hour dispatch allowance. No actual timeout or model setting
changed. The active original launcher retains its 09:45 UTC cutoff. If it stops
with fewer than three matching receipts, review its outcome/error first, then
`/tmp/calibrate_ocaml_compiler_serial_resume.py` may dispatch the missing attempts
before 10:00 UTC with the proven six-hour allowance. Never overlap launchers.
The resumed script writes `/tmp/vulcanbench-ocaml-compiler-launches-resumed.json`
to preserve the original receipt; use a separate resumed calibration log too.
See docs/DECISIONS.md. No resumed launcher has been executed yet.

The first control-public preparation stopped because upstream promote intentionally
returns failure for the old mismatch even after updating the source expectation.
That receipt is preserved as CONTROL_PUBLIC_EXPECTATIONS_PROMOTION_EXIT.json.
The retry (34718) verifies unchanged program bodies, then runs each public test
normally after promotion and requires PASS. No grading assertion changed for this
orchestration correction. All original controls remain in CORE_ONLY_CONTROLS.

A conditional revised-gate launcher is waiting in session 90057. It reads the
CONTROL_PUBLIC_EXPECTATIONS.json receipt, tolerates an in-progress JSON write,
and launches the complete strict gate only when all three controls have passed
normal public tests and preserved their program bodies. It refuses preparation
errors and has a 12:00 UTC dispatch cutoff. New strict log/report paths are
`/tmp/vulcanbench-ocaml-compiler-workflow-validation.log` and adjacent `.json`.
The preparation and strict validation are serial; no model call is queued.

IMPORTANT: `/tmp/admit_ocaml_compiler.py` was for the old core-only admission and
must NOT be reused for this revision. A renewed admission must consume the new
workflow-validation report and assert the verdict names match the current five
F2P/four P2P specification for every clean row/control. Preserve the old task
hash and all core-only evidence; new hash is
`0e66b01f1e6719bd88831eee1dfa7cdfc65ff60c2f05094777ecf4f4ae4d79cf`.
After the revised gate, run full CI sequentially and restore the compiler to the
active manifest only if the whole new gate passes. Expanded calibration stays
incomplete with zero fresh revised-task model outcomes. Do not count the old
model replay/public failure or provider refusal as new-task scores.

Checkpoint 2026-10-01 10:50 UTC (3:50 AM PDT): public-control preparation has
completed blanket-pointer and record-only public expectations; row-local control
is still building. Conditional strict gate 90057 has not dispatched yet.
Renewed admission is prepared in `/tmp/admit_ocaml_compiler_workflow.py` and
checks the new report, every scored verdict name, all three repetitions/controls,
standard PASS, provenance hashes, and normal public-test preparation. It will not
consume the old core-only report. The task inputs remain frozen at 0e66b01f.

Session 39313 runs `/tmp/finish_ocaml_workflow.py`, waiting for the new strict
receipt. Only after a valid complete report does it run renewed admission,
regenerate CALIBRATION.json, refresh current narratives, then run full CI
sequentially. Log: `/tmp/vulcanbench-ocaml-workflow-finish.log`. CI log/report:
`/tmp/vulcanbench-ocaml-workflow-ci.log` and adjacent `.json`. No model call is
queued. If preparation or the gate fails, inspect the preserved receipt; do not
admit or use the old script. The waiting finisher refuses a missing receipt after
14:30 UTC. Check actual gate, finish, and CI outcomes before claiming completion.

Added the missing `agent-image-ocaml` Make target and prerequisites for compiler
verifier/agent targets. `make -n agent-image-ocaml-compiler` now shows the complete
base, OCaml verifier, OCaml agent, and compiler-image build sequence. Dockerfiles,
image contents, source and grading inputs did not change. The latest CI will
include this reversible build-entry-point correction.

The calibration reporter now keeps missing/null functional receipts in an
explicit unscored_runs list without counting them as failures or coverage.
Both null and absent-score regression cases passed; all nine report tests,
Ruff, format checks, and report mypy passed. Regenerated current seven-task
CALIBRATION.json still reports 17/21 (81.0%). Existing outcome files were not
changed. The queued full CI includes these additional two regression cases.


The waiting finishing wrapper was restarted in session 10720 before any strict
report existed, to tolerate a partial JSON write when polling the final receipt.
The old wait was intentionally interrupted; its log remains preserved. No
validation or task input changed. The current finishing log is
`/tmp/vulcanbench-ocaml-workflow-finish-retry.log`; all admission/CI gates are
unchanged. After CI, update the current validation narrative with the actual
result and preserve the CI JSON/log in the suite's receipts. Do not claim CI
passed based only on the earlier 1071-test run.

Checkpoint 2026-10-01 12:01 UTC (5:01 AM PDT): full CI completed successfully at
11:57:27 UTC, with 1084 passes, five unrelated analyzer skips, four deselections,
and 86.38% harness coverage. CI_WORKFLOW.json and CI_WORKFLOW.log preserve the
actual timed receipt and output in the suite. Lint, format, no-em-dash, and
117-file mypy passed. Later documentation-only edits passed the no-em-dash check
and git diff --check. All eight current narrative documents' local links exist.
README.md now provides an entry point and repeatable build/validation commands.
Calibration explains historical capability views and overlapping denominators;
this does not change the score or make the whole pool a below-80 benchmark.

A supplemental cold reference audit is running in session 49377 after CI, with
log `/tmp/vulcanbench-ocaml-compiler-cold-reference.log` and adjacent `.json`.
It configures and builds world.opt in one fresh offline workspace without
installing the public baseline seed. Then it runs the nine unchanged assertions
and the new public GC regression. It permits one hour for the cold build and
20 minutes for each of the two subsequent commands, dispatching before 13:00 UTC.
This is one supplemental cold build, not repeated-cold determinism or a new model
attempt. Grading/solver budgets and frozen task hash remain unchanged. Preserve
its final report, record success or the actual failure, and do not conflate it
with scored calibration. No model or other validation workload overlaps.


Checkpoint 2026-10-01 12:20 UTC (5:20 AM PDT): cold session 49377 completed
successfully. Its raw receipt and log were copied to compiler-gadt-field-safety/
COLD_REFERENCE.json and COLD_REFERENCE.log. Full world.opt build passed without
seed installation, in 1048.8 seconds; the nine unchanged assertions passed in
7.5 seconds and the new upstream GC test passed in 2.3 seconds. The frozen hash
still matches. Validation/admission/README now distinguish this single cold audit
from the three repeated seed-based clean admission runs. No fresh model result
was invented or imported from reference/replay diagnostics.

Current reviewable handoff: README.md links the charter, public Jane Street
research, exact validation and CI proofs, calibration history, and reproducible
commands. Eight tasks are validated; seven are calibrated at 81.0%; the revised
compiler has zero fresh outcomes. Under-80 remains unproven and independent
confirmation is outstanding. No reference, CI, or model worker remains active.
Remaining improvement priorities are deeper diverse OSS/library tasks, calibrated
selection beyond the saturated fixtures, fresh compiler calibration after the
provider refusal is resolved, and independent confirmation after freezing. Do
not start a knowingly over-budget batch or bypass the refusal to fill the column.
Preserve all receipts and stop the overnight automation by 9 AM PDT.

Final consistency check at 12:22 UTC passed: eight manifest/full entries match
all validation rows and current report hashes. Current matching model coverage is
exactly the preserved 21 original-task outcomes and 17 passes; the expanded score
and below-80 flag are null. All current documentation links exist. No worker or
Docker container remains active. Changes remain uncommitted and reviewable on
codex/ocaml-v1. Do not repeat completed tests merely to occupy the remaining hours;
start new work only when it addresses a substantive open question and fits the
morning stop with full budgets. Keep unchanged heartbeat runs quiet.


Checkpoint 2026-10-01 13:02 UTC (6:02 AM PDT): saved source replay of all 21
current original-task patches completed successfully in session 50808. It used
fresh offline workspaces, serial execution, unchanged check/setup budgets, and
current fixed task hashes. All 17 passes and four failures reproduce exact scores
and failed-check names. Graph failures are atomic_dependency_reversal in all
three runs; cache failure is deadline_dominance in one run. Every regression guard
passes. Three older dispatcher patches needed generated _build exclusions; 18
others have unchanged raw/source hashes. Original patches were not rewritten.
The initial scratch launcher had a Task.id attribute typo before starting any
workspace; its error log is preserved, and corrected Task.task_id execution
produced the complete receipt. This was orchestration, not a task/model failure.

MODEL_SOURCE_REPLAY.json/log, 21 derived patch-audit files, and
MODEL_PATCH_REVIEW.md/json are reviewable. The qualitative review inspected
library implementations in the first lexical run per original family and the
failed cache run; it inventoried public-test edits in all 21, without a numeric
maintainability rating. Eighteen patches edit public tests; none edits a supplied
.mli, and fresh guards pass. The inspected verifier image still matches the
pinned original identity. All nine current documentation link sets exist;
no-em-dash and git diff --check pass. This documentation/data-only addition does
not invalidate the 1084-test CI receipt or alter frozen task definitions.
No new model call or score was created. Expanded calibration remains withheld.
No worker remains active. Keep unchanged runs quiet and end by 9 AM PDT.
