# OCaml evaluation suite handoff to Factory AI

Continuation note, October 5: generative-module-registry calibration is complete
at 3/3 fresh native passes on d3ebdb801b30. Read
[tasks/ocaml-registry-candidate/CHECKPOINT.md](../tasks/ocaml-registry-candidate/CHECKPOINT.md)
and CALIBRATION.md. Every attempt passed all eight behavior groups and two guards;
manual source/trace review and all 15 archived artifact hashes passed. Reporter
support, 38 focused checks and fresh full CI (1105 tests, 86.38% coverage) passed.
No failures required replay, and all scoped jobs exited. Raw receipts are in
runs-ocaml-registry-20261005T173757Z, which is Git-ignored. All nine earlier pool
identities and the immutable expansion blueprint remain unchanged. This original
fixture has no upstream/OSS-bug-fix quota credit. Difficulty, compiler coverage,
publication admission and independent confirmation remain unresolved. Recommended
next work: source the complementary first-class module/functor slot in a complete
pinned upstream repository, establishing baseline behavior and distinct obligations.

Continuation note, October 5: generative-module-registry authoring is complete
in tasks/ocaml-registry-candidate, identity d3ebdb801b30. Read CHECKPOINT.md and
AUTHORING.md there. Three offline base/reference pairs, eight compiling semantic
controls, helper-placement and standard validation passed; 59 focused regression
tests passed. No model calls or fresh full CI were run. Before calibration, add
reporter support, pass full CI and adapt the artifact-cleaned dispatcher to a
fresh three-attempt native cohort. All earlier pools and the expansion blueprint
remain unchanged. This original module/type-identity fixture has secondary
lifetime/transaction overlap and no upstream/OSS-bug-fix quota credit. Difficulty,
compiler coverage and publication remain unresolved. All scoped jobs exited.

Continuation note, October 5: the family-balanced expansion blueprint is frozen
in docs/ocaml-expansion-v1/PLAN.md, plan.json, INVENTORY.json and FREEZE.json.
Read it before further authoring. It targets 20 tasks in ten equally weighted
semantic families, with two slots each and a 10/10 engineering/language split.
Eleven existing definitions and nine new slots are prospective, not admitted.
All 15 latest logical tasks are grouped once; all nine historical pool identities
are pinned without rescoring. No model or task-authoring job was launched for
this planning step. The compiler remains blocked, and the overall difficulty
goal remains unmet. Next recommended work is the generative-module-registry
slot, adding module/type-identity coverage rather than more serialization weight.

Continuation note, October 4: open-union held-out workload f9e65a3ac13a is
complete at 3/3 fresh native passes. Current evidence is in
tasks/ocaml-union-heldout/CHECKPOINT.md and CALIBRATION.md. The earlier interrupted
cohort preserves two raw passes, observed cross-attempt solver-test exposure and
an interrupted trace. Fresh between-attempt artifact cleanup and manual review
passed without changing task/budgets/CLI or adding OS read isolation. This is
development-informed confirmation on a related workload, not external independent
confirmation or family admission. All scoped jobs have exited. Difficulty and
compiler coverage goals remain unresolved. Recommended next work is a frozen,
family-balanced expansion plan before more candidate authoring.

Continuation note, October 4: full-repository typed serialization is complete.
tasks/ocaml-serialization-candidate-v3/CHECKPOINT.md and CALIBRATION.md are the
current evidence. Candidate fa86068dce81 measured 2/3 fresh native complete
passes, after three offline admission pairs, seven compiling controls and full CI
(1103 tests). Any functional failure was replayed offline. All jobs have exited
and all earlier definitions remain frozen. Report this development result
separately pending an explicit composition decision about shared schema skills.
The suite-wide difficulty goal and compiler refusal remain unresolved.

Continuation note, October 3: the handoff below preserves the October 2 state.
Current completed work is in tasks/ocaml-atomic-candidate-v2/CHECKPOINT.md and
CALIBRATION.md. Full-repository atomic rewiring v2 measured 2/3 fresh native
complete passes, with its failure reproduced offline. Current contextual twelve
library families are 31/36 (86.1%); the full thirteen-family headline remains
withheld for the unresolved compiler refusal. The earlier atomic definition,
exposed pass, interrupted trace, preflight failures and three sandbox-invalid
zeroes are all preserved separately. All jobs have exited. No expired automation
or compiler retry was started. Read the latest checkpoint before operating.

Take over development of a balanced OCaml evaluation suite for engineering and
language mastery, with particular relevance to Jane Street engineers. The user
wants native execution on this Mac without Docker, useful model cards, and
meaningful difficulty that can put the calibration model below 80%. The current
eleven-library development results are 29/33 complete passes (87.9%), so that
difficulty goal remains unmet. Twelve tasks exist, including an uncalibrated
compiler task. No model, validation or dispatcher job remains active.

Factory is taking over benchmark authoring and operation. Calibration should
continue through the existing pinned Codex harness for comparability; changing
the authoring agent does not change the model being evaluated. Keep this handoff
and benchmark references out of target-model prompts and solver workspaces.

## Working copy and transfer

Use `/Users/vulcanbench/dev/VulcanBench`, branch `codex/ocaml-v1`.
The current Git HEAD is `a040271077f799bc98f602cfba18e9285b184801`.
The OCaml work is uncommitted: many harness files are modified and the task pools,
scripts, tests and sandbox definitions are untracked. Do not reset, clean or
replace this checkout. A fresh clone of the branch will not contain this work.

Run directories match the Git-ignored `runs-*/` rule. A branch transfer or normal
Git patch alone will lose their receipts. If moving to another computer, preserve
the complete working copy changes, all three task pools, source snapshots, the
run directories below, and this document. Verify Git LFS source snapshots are
actual files rather than pointers. Do not copy authentication files or secrets.
Machine-specific dependencies can be reprovisioned separately. Darwin compiler
seed binaries cannot be reused on Linux or another architecture.

Read these files first. The checkpoints' completed states supersede historical
entries that describe jobs as running:

1. [AGENTS.md](/Users/vulcanbench/dev/VulcanBench/AGENTS.md),
   [CLAUDE.md](/Users/vulcanbench/dev/VulcanBench/CLAUDE.md), and relevant OCaml
   entries in [DECISIONS.md](/Users/vulcanbench/dev/VulcanBench/docs/DECISIONS.md).
2. [Original checkpoint](/Users/vulcanbench/dev/VulcanBench/tasks/ocaml-v1/OVERNIGHT.md)
   and [measurement plan](/Users/vulcanbench/dev/VulcanBench/tasks/ocaml-v1/MEASUREMENT_PLAN.md).
3. [Three-candidate checkpoint](/Users/vulcanbench/dev/VulcanBench/tasks/ocaml-candidates/CHECKPOINT.md)
   and [calibration](/Users/vulcanbench/dev/VulcanBench/tasks/ocaml-candidates/CALIBRATION.md).
4. [Latest checkpoint](/Users/vulcanbench/dev/VulcanBench/tasks/ocaml-incremental-candidate/CHECKPOINT.md)
   and [latest calibration](/Users/vulcanbench/dev/VulcanBench/tasks/ocaml-incremental-candidate/CALIBRATION.md).

## Existing tasks and scores

All measured native attempts below used GPT-6.1 Sol medium, three fresh attempts
per task, serial concurrency one and unchanged budgets. A complete task pass
requires every behavioral check and regression guard. The score is the equal
mean of task success rates, not the fraction of individual assertions passed.

| Pool | Frozen identity | Tasks | Native complete passes |
| --- | --- | --- | --- |
| ocaml-v1 | c0a1cc221d60 | 8 defined, 7 calibrated | 19/21 across the seven library tasks, 90.5% |
| ocaml-candidates | e0a390e323df | 3 calibrated | 7/9, 77.8% |
| ocaml-incremental-candidate | 6b37b41277dd | 1 calibrated | 3/3, 100% |

The original pool is
`/Users/vulcanbench/dev/VulcanBench/tasks/ocaml-v1`. Its seven library tasks are
`async-bounded-dispatch`, `incremental-atomic-graph`, `interval-map-transaction`,
`typed-frame-stream`, `window-watermark-allocation`, `typed-query-optimizer`,
and `async-generation-cache`. Six scored 3/3. `incremental-atomic-graph` scored
1/3, with both failures on atomic dependency reversal. The eighth task is
`compiler-gadt-field-safety`, described under blockers below.

The three larger candidates are in
`/Users/vulcanbench/dev/VulcanBench/tasks/ocaml-candidates`:

- `async-durable-delivery`: ordered journal acknowledgements, worker leases,
  retries, cancellation, shutdown and reentrancy. It scored 3/3.
- `typed-schema-evolution`: GADT record decoding, compatibility, framing,
  allocation bounds and exception-safe cursor restoration. It scored 1/3.
  Both failed patches were replayed offline and reproduced failure to restore
  the cursor after a constructor mutated it and raised. Those failures are
  legitimate semantic outcomes, not build or provider errors.
- `base-nested-transactions`: sparse nested savepoints in the full 495-file
  Base v0.17.3 repository, preserving key/data identities across mutation paths.
  It scored 3/3. Upstream commit is
  `f8d1d3ad1894590cd19d11b9a89fe799caee5e57`.

The newest task is
`/Users/vulcanbench/dev/VulcanBench/tasks/ocaml-incremental-candidate/incremental-budgeted-propagation`.
It adds exact recomputation quotas and resumable drivers to the full 128-file
Incremental v0.17.0 checkout, commit
`61baea591be9bfaf512bb2fcdf176753f67b2607`. Its contract covers recursive direct
parents, dynamic dependencies, deferred writes, observer publication,
driver interoperation, reentrancy and exception poisoning. All three attempts
passed its seven behavior groups and two guards. The solutions queued parents
only during bounded calls and preserved ordinary unbounded optimizations.
Repository size alone has not made these engineering tasks difficult enough.

Across the four larger candidates, the result is 10/12 (83.3%). Across all eleven
native library tasks, it is 29/33 (87.9%). These are contextual arithmetic across
development cohorts, not a frozen expanded-suite card or independent confirmation.
Do not claim that the full suite scored below 80% from the three-candidate 77.8%
result. An earlier Docker seven-task result was 17/21 (81.0%); preserve it as a
separate environment and do not pool it with native results.

## Receipts and model cards

Preserve these complete directories:

- `/Users/vulcanbench/dev/VulcanBench/runs-ocaml-native-20261001T175608Z`
- `/Users/vulcanbench/dev/VulcanBench/runs-ocaml-candidates-20261002T011915Z`
- `/Users/vulcanbench/dev/VulcanBench/runs-ocaml-incremental-20261002T135901Z`

They contain launch/progress receipts, individual summaries and traces, final
model patches, and JSON/HTML model cards. The latter two have reusable serial
`launch.py` dispatchers. The latest directory also preserves `audit.py` and
`failure-replay.py`; there were no failures to replay in the newest cohort.
Never rerun an old launcher into its old directory. Adapt it to a fresh directory,
new pool identity and current validated code digests.

Each task pool contains detailed validation and calibration evidence. The
latest authoritative files are `VALIDATION_OFFLINE.json`, `CI.json`,
`RECEIPT_AUDIT.json`, `CALIBRATION.md/json` and `EXPANSION_CONTEXT.json` in the
Incremental candidate pool. Earlier failed fixture/control gates remain saved;
they are history, not proof of task admission. The original pool's native
proof is `NATIVE_VALIDATION_OFFLINE.json` and `NATIVE_ENVIRONMENT.json`.

## Native environment and calibration commands

The existing environment uses Python 3.12 in the repository virtual environment,
OCaml 5.2.1, Dune 3.17.2 and 96 pinned opam packages. Package identities are in
each pool's `TOOLCHAIN.lock`. Core, Async, Incremental, bin_prot and ppx packages
are already installed. Use this environment rather than system Python 3.9:

```sh
cd /Users/vulcanbench/dev/VulcanBench
source /Users/vulcanbench/.local/share/vulcanbench/ocaml-v1-native/env.sh
export PATH=/Users/vulcanbench/.local/share/vulcanbench/ocaml-v1-native/codex-cli/node_modules/.bin:"$PATH"
export PYTHONPATH=/Users/vulcanbench/dev/VulcanBench
python --version
ocamlc -version
dune --version
codex --version
```

The last command must report `codex-cli 0.159.0`. Without the explicit pinned
CLI PATH, the host default is 0.149.0. Do not change CLI/model settings merely
because Factory is the authoring agent. Keep `gpt-6.1-sol`, medium effort,
subscription billing, no judges, local sandbox, no agent container, concurrency
one, 10800-second solve allowance and 540 configured steps. The configured
steps are not a guarantee of CLI turn enforcement. Stop on provider/invocation
errors for review, without automatically retrying them or counting them as
functional failures.

The following is an example for a new, explicitly chosen output directory;
it is not a request to repeat the already-completed calibration:

```sh
vulcanbench run \
  --tasks-root /Users/vulcanbench/dev/VulcanBench/tasks/ocaml-incremental-candidate \
  --task incremental-budgeted-propagation \
  --harness codex --billing subscription --model gpt-6.1-sol --effort medium \
  --repeat 1 --max-concurrency 1 --sandbox local --no-agent-container --no-judges \
  --output-dir /Users/vulcanbench/dev/VulcanBench/runs-ocaml-NEW_UNIQUE_DIRECTORY
```

Prefer the existing dispatcher pattern for three separate serial attempts. It
checks task, metadata/control, toolchain and validated code identities before
each call, stops on errors and refreshes cards. Generate a report using:

```sh
python /Users/vulcanbench/dev/VulcanBench/scripts/report_ocaml_calibration.py \
  --suite ocaml-incremental-candidate --sandbox local \
  --runs-root /Users/vulcanbench/dev/VulcanBench/runs-ocaml-NEW_UNIQUE_DIRECTORY \
  --output /Users/vulcanbench/dev/VulcanBench/runs-ocaml-NEW_UNIQUE_DIRECTORY/MODEL_CARD.json \
  --card /Users/vulcanbench/dev/VulcanBench/runs-ocaml-NEW_UNIQUE_DIRECTORY/MODEL_CARD.html
```

The reporter supports the three existing pool names. A new pool requires adding
its CLI choice/title and regression coverage while preserving the current
defaults and identities. Audit manifests yourself: selecting local mode alone
does not prove identical OS, architecture, package pins, CLI or resource limits.
Native model execution has no Docker resource caps or enforced network isolation.

## Validation and implementation rules

For each new task, provide an openly specified issue, public repo snapshot and
API prototype, hidden verifier, reference `gold_patch.diff`, metadata and
plausible faulty controls. The harness provides only the repo and issue to the
solver, then injects hidden checks during grading. Never leak gold or hidden
checks into public scaffolding, build seeds or solver workspaces.

Before calibration, require three fresh offline base/reference pairs. Base
must pass every regression guard and fail every required behavior group; gold
must pass all behavior and guards. Each faulty control must compile, preserve
guards and fail for its intended semantics. A compiler failure is not evidence
of control sensitivity. Require standard validation PASS and applicable CI.
Preserve exact supplied interface hashes, including interface-bearing .ml files
where appropriate. Replay and diagnose failed model patches in fresh offline
workspaces without counting replay as a new scored attempt.

```sh
python /Users/vulcanbench/dev/VulcanBench/scripts/validate_ocaml_pilot.py \
  --tasks-root /Users/vulcanbench/dev/VulcanBench/tasks/NEW_POOL \
  --sandbox local --offline \
  --output /Users/vulcanbench/dev/VulcanBench/tasks/NEW_POOL/NEW_VALIDATION_RECEIPT.json
make ci
```

The offline flag uses macOS sandbox-exec to deny network for the complete
validation process tree. Use a new receipt filename rather than overwriting
historical proofs. Full CI last passed 1094 tests, five unrelated analyzer skips,
four deselections, one dependency warning and 86.38% coverage. Focused reporter /
validation tests passed 27 cases, and reporter mypy passed.

Keep the three existing measured pool identities frozen. Create a separate
candidate pool for a new task instead of silently extending their suite.json
manifests. The suite hash covers scoring inputs, but not every setup/metadata
condition; also freeze and record those conditions and code digests. Any material
task clarification requires a new version, clean validation and fresh attempts.
Preserve upstream licenses and provenance. The full Base and Incremental repos
are MIT; the two original larger fixtures use the project's Apache 2.0 license.

## Known blockers and limitations

- `compiler-gadt-field-safety` is reference-validated but has zero fresh current
  native model outcomes. Its prior provider content-filter refusal is preserved
  in [PROVIDER_REFUSAL.json](/Users/vulcanbench/dev/VulcanBench/tasks/ocaml-v1/compiler-gadt-field-safety/PROVIDER_REFUSAL.json).
  Do not retry automatically, rephrase the frozen prompt or route around that
  refusal. Resolve the provider condition before any further invocation. Do not
  treat the old core-only pass or unscored refusal as current task coverage.
  Full eight-task and expanded twelve-task headlines remain withheld.
- The compiler under test is upstream OCaml 5.6.0+dev0 at
  `364874344779d2e410e99eb634bea8e7e2b70159`, not the library toolchain and not
  OxCaml. It uses a separately built Darwin arm64 public baseline seed, single-job
  rebuilds, and 1200-second setup/check headroom. The seed contains no hidden
  tests or reference patch. See provenance and DECISIONS before changing it.
- Incremental's upstream `@all` target is not portable/self-contained on this
  Mac: a generator is missing from the public checkout and debug scripts assume
  GNU cp/sed behavior. The validated preflight explicitly compiles
  `src/incremental.cmxa` and `test/incremental_test.cmxa`, then runs two supplied
  public clients through `@runtest`. Do not claim all upstream inline/debug tests
  execute. Details are in the latest pool's `UPSTREAM_ALL_PROBE.json`.
- The schema task has an unresolved specification question about propagation
  of a constructor-raised public Error.Decode exception. Cursor rollback is
  specified and tested; the propagation policy for this particular exception
  is not. Do not retrospectively alter grading or existing scores.
- Jane Street relevance is inferred from public sources, not internal access or
  endorsement. OSS tasks disclose public-source exposure and do not claim
  decontamination. Research is in each pool's research document.
- These are development measurements with no independent confirmation. Model
  identity is requested-only and integrity telemetry is observational. Cards
  measure functional completion; OCaml quality/security analyzers are absent.
  HTML data and escaping are tested, but visual rendering remains unverified
  after a local-file preview restriction. Do not bypass that restriction.

## User preferences and suggested next work

Never author U+2014 or U+2013 punctuation; use commas, colons, parentheses, plain
hyphens or separate sentences. Preserve third-party source and receipt evidence.
End every final reply with one concise, concrete suggested next step. Work
autonomously within the authorized build/test scope and avoid repeated permission
questions. Do not use ultra effort, spawn subagents or message other people under
the current work instructions. The overnight automation ended October 1 at
9 AM America/Los_Angeles; do not restart or extend it. Later work was separately
authorized and has now completed.

The recommended next candidate is atomic dependency rewiring in the full
Incremental repository. The original small atomic-graph task's 1/3 result gives
evidence for this direction, while budgeted propagation and full Base navigation
both scored 3/3. This new candidate has not been built or tested yet.

Start by reviewing the original atomic-graph failures and Incremental's actual
dependency/height machinery. Write an explicit prospective-batch contract, with
atomic rejection of invalid/cyclic edits and preservation of observers and
propagation invariants. Decide and document error precedence and callback effects
before writing checks. Build a correct reference and compiling faulty controls,
validate offline, freeze, then calibrate three fresh serial medium attempts at
unchanged budgets. Do not manufacture difficulty with hidden requirements,
weaker model settings, shorter timeouts, broken references or selective omission.
Only after sufficient difficulty and independent confirmation should an expanded
suite be frozen for publication.
