# OCaml v1 difficulty calibration

Owner target, 2026-09-30: build and test a useful OCaml suite, aiming for a model
to score below 80%. The initial reference is provisionally GPT-6.1 Sol at medium
effort through Codex 0.159.0 on the existing ChatGPT subscription. This default
was stated during implementation after requesting the owner's preference.

## Native Mac execution and model cards

The owner prefers native execution to reduce Docker memory use. From the
repository root, provision the pinned toolchain and native public compiler seed:

```sh
make ocaml-native
source "$HOME/.local/share/vulcanbench/ocaml-v1-native/env.sh"
python scripts/validate_ocaml_pilot.py --sandbox local --offline --output /tmp/ocaml-native-validation.json
```

The local opam root is dedicated to this suite. Every library version must match
TOOLCHAIN.lock; compiler seed binaries are built for Darwin arm64 and never
copied from the Linux image. The offline flag denies network for the entire
native validation process and its descendants. Model CLI execution has its own
network requirements and does not use that validation-only profile.

Once the clean native gate passes, a serial development sweep uses:

The earlier compiler calibration encountered a provider content-filter refusal,
preserved unscored in PROVIDER_REFUSAL.json. Resolve that provider condition
before dispatching that task again on this configuration; do not change its
prompt or route around the refusal. The command below documents the full-suite
plan, and a dry run does not establish provider availability. Individual task
runs can proceed without claiming full-suite coverage.

```sh
export PATH="$HOME/.local/share/vulcanbench/ocaml-v1-native/codex-cli/node_modules/.bin:$PATH"
vulcanbench run --suite ocaml-v1 --harness codex --billing subscription \
  --model gpt-6.1-sol --effort medium --repeat 3 --max-concurrency 1 \
  --sandbox local --no-agent-container --no-judges --output-dir runs-ocaml-native
```

Keep native runs in a separate directory, preserving previous Docker receipts.
The pinned native Codex 0.159.0 CLI is present at that path on this machine;
toolchain provisioning does not install model CLIs on a new machine.
NATIVE_RUN_PLAN.txt preserves the dry-run plan. Its generic API-credit estimate
is not a measured cost or a subscription billing receipt.
The harness records execution environment and OCaml/Dune/opam versions.
New summaries also record the effective agent time ceiling and configured step
allowance; the latter is not necessarily an enforced CLI turn cap. Reporting
excludes explicit budget overrides and recorded limits that differ from task
defaults. Older launch receipts must be audited when those fields are absent.
Native host runs have no Docker CPU or memory ceilings. The source and assertions are
the same; execution conditions differ, so do not combine them into one score.
No native model sweep ran during provisioning. New dispatch must permit the
full solve and grading allowances; do not shorten them to fit a work deadline.

Generate the JSON report and an HTML model card from recorded runs:

```sh
python scripts/report_ocaml_calibration.py --runs-root runs-ocaml-native \
  --model codex:gpt-6.1-sol --effort medium --sandbox local \
  --output /tmp/ocaml-native-results.json --card /tmp/ocaml-native-model-card.html
```

For another model, specify its full recorded harness/model identity and effort.
Cards disclose coverage, task hashes, environment, source provenance, and
unscored/excluded receipts. Trace-only aborted invocations are listed separately
without inventing their effort, revision, environment, or a functional verdict;
audit those before publishing a final report. Cards withhold the headline when any task lacks three
current scored attempts, or when native and Docker executions are mixed without
environment selection. They report functional completion only. OCaml quality
and security factors remain unavailable. Current development results do not
establish independent confirmation or a stable leaderboard.

Environment selection currently checks the recorded local/Docker mode. It does
not automatically establish that every attempt used the same OS, architecture,
package pins, CLI version, resource limits, or setup seed. Before publishing,
audit the individual manifests and launch/provisioning receipts against the
chosen conditions. Keep a separate run directory for each configuration and
environment. The compiler and library tasks intentionally use different OCaml
versions; preserve that documented task-specific distinction. A task hash does
not pin setup metadata or all execution conditions.

## Target and meaning

Target a reference-model task pass@1 of 60% to 75%, with a median below 80% if a
multi-model panel is selected. This margin reduces the risk of a single lucky
draw crossing 80%. Difficulty labels remain provisional until measured.

The current development suite has eight distinct tasks. With exactly three
fresh scored attempts per task, strictly below 80% means at most 19 complete
passes out of 24 attempts (79.2%). When attempt counts differ, use the report's
equal-weight mean of per-task success rates rather than a pooled pass count.
Report per-task counts as well as the aggregate. The original five-task pilot
and seven-task calibration are historical stages, not the current suite.
Eight development tasks do not establish a stable ranking.

## Make tasks demanding and fair

Combine obligations that occur together in engineering: API constraints and
behavior; incremental output and dependency changes; buffering and shutdown;
streaming input and recovery; correct output and bounded allocation.

Each requirement must be available to the solver through the issue, supplied
interfaces, public examples, or pinned dependency documentation. All reference
solutions must satisfy all required checks. Errors due to absent tools, package
installation, missing specifications, or unjustified resource limits are not
evidence of difficulty.

Keep the supplied `.mli` contracts fixed for these implementation tasks. Check
the compiler contract at verification as well as behavioral outcomes. Tests
should reject plausible incomplete solutions, such as clearing the whole map
when assigning a subrange, losing a partial stream frame, closing before queued
work drains, ignoring a newly selected dependency, or retaining expired events.

## Validation before model measurement

1. Run public tests in the starting project to establish a healthy environment.
2. Run every fail-to-pass check individually at base and inspect the failure;
   require all to fail for the stated missing behavior.
3. Run every regression guard at base; require all to pass.
4. Apply the reference patch in a fresh workspace; require all checks to pass.
5. Repeat base and reference validation three times in the chosen pinned
   environment. Native validation must include the clean offline gate.
6. Validate at least one plausible incomplete patch per task and record which
   checks reject it. Exclude any control rejected only because it cannot build.
7. Verify that hidden tests and reference patches are absent from solver
   workspaces. Measure allocation limits in native OCaml, not bytecode.

## Development calibration

Run at least three attempts per task/configuration with fresh solver workspaces
and a context containing only the task issue and starting project. The builder's
conversation and reference solutions must not be visible to the tested model.
Use serial task execution and identical documented budgets across configurations.
Log model identity, effort, harness/CLI version, task hash, image identity,
resource limits, pass counts, timeout counts, and infrastructure failures.

Review failures before editing the candidate pool. Revise or replace tasks solved
by every reference configuration unless they serve as a deliberate easy anchor.
Keep difficult tasks only when the specification is complete and reference
validation is sound. Do not tighten timeouts, introduce secret requirements, or
choose a weaker configuration merely to produce a sub-80% headline.

Original pilot fixtures primarily test feature implementation. The expanded
suite should also include substantial verified OSS bug-fix tasks and larger
navigation surfaces before claiming coverage of production repository work.

## Freeze and confirmation

If the pilot is saturated, expand the candidate pool and revise the engineering
depth before freezing. Aim for approximately 20 admitted tasks for a first full
suite, subject to sourcing and calibration evidence; keep the original balanced
engineering/language focus. Do not pad the suite with copies of one task family.

Freeze the selected tasks, tests, requirements, and environment. Repeat the chosen
configuration on fresh runs after selection. Label development calibration as
such, and report the confirmation result even if it exceeds 80%. Do not reuse
selection runs as independent confirmation evidence.

For the fixed suite, show each task's success rate and uncertainty from repeated
attempts. For generalization across task families, use a task-aware interval and
state its assumptions. Repeated attempts of one task are not additional distinct
engineering problems. A strong statistical claim of being below 80% would require
the relevant confidence interval's upper bound to be below 80%; an observed point
estimate alone is weaker evidence.

## Execution

Build the sandbox and validate task wiring:

```sh
make agent-image-ocaml-compiler
.venv/bin/python scripts/validate_tasks.py tasks/ocaml-v1 --sandbox docker
.venv/bin/python scripts/validate_ocaml_pilot.py --output /tmp/ocaml-clean-validation.json
```

Model measurement needs the same OCaml toolchain in the agent phase. For Codex,
the seven original tasks use
`VULCANBENCH_AGENT_IMAGE=vulcanbench/agent-codex:ocaml-v1`; the compiler task uses
`VULCANBENCH_AGENT_IMAGE=vulcanbench/agent-codex:ocaml-compiler-v1`.
The Make target builds both matching agent/verifier pairs and their prerequisites.
Run task-specific batches with the matching image, rather than applying one
original-task agent image to the mixed suite. The compiler image contains the
public pre-fix build seed needed for its documented setup. Both agent batches
use `--agent-container`. Verification runs in network-off containers.
Model/effort and billing selection must be explicit in the saved run record.

Use the pinned Codex 0.159.0 CLI on PATH and the common task-specific command:

```sh
VULCANBENCH_AGENT_IMAGE=<matching-agent-image> .venv/bin/vulcanbench run \
  --task <task-id> --tasks-root tasks/ocaml-v1 \
  --harness codex --billing subscription --model gpt-6.1-sol --effort medium \
  --repeat 3 --max-concurrency 1 --no-judges --agent-container \
  --output-dir runs-ocaml-pilot
```

This command requests three new attempts. Do not dispatch another batch while
one is active. The historical overnight compiler calls used single-attempt
dispatches with the same settings, to preserve full budgets before the morning
stop. A provider refusal and insufficient remaining budget prevent any further
compiler calibration overnight; this command is a reproducibility recipe for
later work, not a pending launch. Preserve existing output directories.

The budgets in pilot metadata are a provisional 3-hour ceiling and 540 nominal
agent steps, serial execution, following the existing generous Frontier policy.
Codex CLI does not expose a turn-cap option, so this configuration enforces the
time ceiling but cannot enforce the nominal step ceiling. See
`docs/DECISIONS.md` for rationale and limits. Seven-task development calibration
scored 81.0% (17/21), so the under-80% target remains unmet. The expanded suite's
aggregate is withheld until the revised compiler task has complete fresh
coverage. See [CALIBRATION.md](CALIBRATION.md).


Generate a hash-filtered calibration report without making additional model calls:

```sh
.venv/bin/python scripts/report_ocaml_calibration.py --output /tmp/ocaml-calibration-review.json
```

The report includes all fresh matching attempts and their verifier/CLI receipts.
It withholds a suite score until every task has at least three attempts. Old task
revisions are listed as exclusions rather than mixed into the current score.
Missing or null functional receipts are preserved as unscored outcomes and do
not count as capability failures or satisfy coverage.
Use new output paths for later reports; preserve existing calibration receipts.
