# OCaml development calibration

A subsequent native Mac cohort completed three fresh attempts on each of the
seven library tasks, scoring 19/21 complete passes (90.5%). Six tasks passed
3/3 and the incremental graph passed 1/3; its two failures were on atomic
dependency reversal. Keep this separate from the Docker history below.
[Native calibration](NATIVE_CALIBRATION.md) records the conditions and audit.
The compiler remains pending, so neither cohort establishes the current
eight-task score or independent confirmation.

The seven-task candidate pool completed development calibration at **81.0% task
pass@1** (17/21 complete passes). The generation cache passed two of three fresh
attempts. The failed attempt subscribed a new request to an expired generation
before processing its overdue deadline, violating the stated lazy-expiry rule.
The under-80 target remains unmet. These results are preserved in
[CALIBRATION_SEVEN_TASK.json](CALIBRATION_SEVEN_TASK.json).

The same historical seven-task report separates the requested capability views:

| View | Distinct tasks | Complete passes / attempts | Task pass@1 |
|---|---|---|---|
| Engineering | 4 | 8/12 | 66.7% |
| Language mastery | 4 | 9/12 | 75.0% |
| Whole seven-task pool | 7 | 17/21 | 81.0% |

The Incremental task belongs to both views and is counted once in the whole
pool. The view percentages therefore cannot be averaged into a suite score.
They describe different coverage; the overall under-80 target remains unmet.
Both current eight-task views are withheld because the compiler belongs to both
and has no fresh revised-task attempts. These small view samples do not support
a broad generalization about engineering or language capability.

The six-task pool scored 83.3% (15/18), with the typed-query optimizer passing
all three attempts. Its scope, effects, and type clients add useful language
coverage but did not establish new difficulty at this configuration. That report
is preserved in [CALIBRATION_SIX_TASK.json](CALIBRATION_SIX_TASK.json).

The eighth task is an actual upstream OCaml compiler repair for shared field
classification under GADT equations. Its revised public-workflow definition
passed complete clean reference/control validation. [CALIBRATION.json](CALIBRATION.json)
tracks the eight-task manifest and withholds an aggregate: the revised compiler
has zero fresh model outcomes. The previous core-only pass is preserved under
its old hash and excluded explicitly. A provider refusal is unscored. The failed
public snapshot in patch replay is retrospective evidence, not a fresh failure.
The measured five-task result below is preserved in
[CALIBRATION_FIVE_TASK.json](CALIBRATION_FIVE_TASK.json).

Measured 2026-09-30. The five-task pilot scored **80% task pass@1** at the
provisional reference configuration: `codex:gpt-6.1-sol`, medium effort, Codex
0.159.0, existing ChatGPT subscription, three fresh attempts per task definition.
A complete task pass requires every required check and regression guard to pass.
The requested strictly-below-80% target was **not met**.

| Task | Complete passes | Observed gap | Admission decision |
|---|---|---|---|
| Async bounded dispatch | 3/3 | None observed | Possible easy anchor |
| Incremental atomic graph | 0/3 | Valid atomic dependency reversal creates a transient cycle | Retain as provisional hard candidate |
| Comparator-safe persistent interval map | 3/3 | None observed, including revised work/allocation contract | Revise or replace for hard pool |
| Typed frame stream | 3/3 | None observed | Revise or replace for hard pool |
| Event-time window | 3/3 | None observed, including allocation/work contract | Revise or replace for hard pool |
| Typed query optimizer | 3/3 | None observed, including generated effect/scope checks | Keep as language coverage; hard-pool status unproven |
| Async generation cache | 2/3 | New subscriber attaches to an overdue generation | Retain as provisional lifecycle candidate |
| Upstream compiler field safety | 0 fresh revised-task outcomes | Calibration incomplete | Revised workflow validated; old core-only pass and refusal preserved separately |

The original five selected definitions produced 12 complete passes in 15 attempts.
These are development calibration results, not independent confirmation or a
stable leaderboard. The CLI receipts identify the configured model but do not
independently report model/effort identity, so the identity confidence is
`requested-only`. Individual outcomes, task hashes, CLI receipts, and verifier
verdicts are in [CALIBRATION_FIVE_TASK.json](CALIBRATION_FIVE_TASK.json).

## Revisions and disclosure

Before the first complete batch, an additional reference audit found that a valid
atomic dependency reversal crashed the Incremental reference. The original issue
already required atomic validation against the complete prospective graph. The
reference was repaired and two missing coverage checks were added. Two originally
passing model patches failed retrospective verification of the reversal case;
those diagnostics are excluded from fresh calibration. See
[REFERENCE_AUDIT.json](REFERENCE_AUDIT.json).

The first complete calibrated pilot scored exactly 80%; its receipts are saved
in [CALIBRATION_INITIAL.json](CALIBRATION_INITIAL.json). The interval-map task was
then strengthened with explicit comparator-work and native-allocation bounds.
The efficient reference passed all checks in three fresh validations. A correct
but inefficient control passed the original behavioral checks while failing the
new efficiency gates. Three fresh model attempts also passed the revised task,
so that five-task definition still scored 80%.

The other four task definitions were unchanged during the collection revision;
their existing fresh-hash attempts were reused. Every current matching attempt
is included. Old task hashes are explicitly excluded. No weaker model, lower
effort, tighter timeout, or selective omission was used to obtain a headline.

## What is needed for a difficult full suite

The seven-task data support one repeatably discriminating task, a cache with one
observed deadline failure, and five saturated fixtures
at this configuration. Do not publish this pilot as a sub-80% hard benchmark.
Keep the infrastructure and useful regression fixtures, retain the Incremental
candidate, and use at most a small number of saturated tasks as easy anchors.

Expand a diverse candidate pool toward approximately 20 admitted tasks, with
roughly balanced engineering and language coverage. Prioritize:

1. Verified OCaml OSS bug fixes with meaningful repository navigation and changes
   across callers, implementations, and interfaces. Pin the upstream revision,
   license, issue/PR provenance, and the complete required contract.
2. Typed library or versioned serialization migrations that preserve real callers
   and old binary fixtures, with compiler-checked valid and invalid clients.
3. Async lifecycle repairs involving controlled time, retries, cancellation, and
   multiple components, with stated delivery and shutdown semantics.
4. Incremental dependency and observer-lifecycle changes that combine atomic
   edits, graph replacement, sharing, and bounded recomputation.
5. Performance repairs in substantial libraries that preserve behavior while
   meeting calibrated work/allocation bounds.

Each candidate must pass the same reference, regression, determinism, and
incomplete-control validation gate. Measure at least three development attempts
per configuration, revise saturated candidates, then freeze the selected pool
and run fresh independent confirmation. Aim for an observed 60% to 75% reference
score to leave margin below 80%. Report the confirmation result even if it is
higher, with per-task counts and task-aware uncertainty. See
[MEASUREMENT_PLAN.md](MEASUREMENT_PLAN.md).

## Conditions and limitations

Fresh source-only replay of all 21 current original-task patches reproduced every
functional score and failing-check name. The three older Async contributions
required excluding captured Dune artifacts from derived copies; their actual
source still passed. Original raw patches and scores remain preserved. See
[MODEL_PATCH_REVIEW.md](MODEL_PATCH_REVIEW.md). This is reproducibility evidence,
not additional model attempts or independent confirmation.

The pinned OCaml 5.2.1 image and full package list are documented in
[VALIDATION.md](VALIDATION.md) and [TOOLCHAIN.lock](TOOLCHAIN.lock). Runs were serial,
with a three-hour time ceiling and a nominal 540-step budget. Codex CLI cannot
apply a turn cap, so only the time ceiling is enforced for this configuration.
Agent and verifier image identities were:

- Agent: `sha256:d2841f1ab2b14a2db1ce7a651c561a80e20f423504afd5c75780a57916387945`.
- Verifier: `sha256:77987aefd1582051227ad123b7c4b9d5eb848c235ea15c413698ec9357952e4d`.

The Linux amd64 image ran under emulation on an arm64 Mac. Allocation and work
counts are the tested efficiency contracts; solve time is diagnostic and is not
representative of native hardware latency. OCaml quality/security analyzers are
not implemented, and judges were disabled. The primary score uses functional
checks, rather than the harness composite score. No confidence claim that an
unseen task population or another model/configuration would score below 80% is
supported by this small pilot.

Rebuild the current report without additional model calls:

```sh
.venv/bin/python scripts/report_ocaml_calibration.py --output tasks/ocaml-v1/CALIBRATION.json
```

The compiler task uses the separately pinned upstream OCaml 5.6.0+dev0 build and
matching compiler images in PROVENANCE.json. Its public upstream fix is exposed
in its task row of CALIBRATION.json (`decontaminated: false`). Do not infer a
training cutoff or independent confirmation from its recent date. The original
core-only batch planned three individually dispatched attempts with the full
solve ceiling. It stopped after two invocations. The revised nine-check definition
requires a 6-hour 20-minute dispatch allowance; its 09:40 UTC cutoff has passed,
so no revised-task model invocation is queued overnight.

The compiler batch stopped after the second invocation returned a provider
cybersecurity content-filter refusal. The completed first patch passed all
core-only scored checks; a clean replay reproduced that pass but found an
unmaintained public snapshot. The revised workflow guard covers this already
stated obligation. Neither retrospective replay nor the old pass is a fresh
revised-task outcome. The refusal yielded no functional verdict and is recorded
in [PROVIDER_REFUSAL.json](compiler-gadt-field-safety/PROVIDER_REFUSAL.json).
It cannot be used to claim OCaml difficulty or a sub-80%
score. Original traces and launch receipts remain. Do not change the frozen
prompt, weaken settings, or use another route to bypass that filter.
