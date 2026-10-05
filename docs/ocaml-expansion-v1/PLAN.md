# OCaml family-balanced expansion plan v1

Frozen planning baseline: October 5, 2026, America/Los_Angeles.
Status: prospective authoring and selection blueprint. No expanded suite is
admitted or scored. This freezes family capacity, weighting, coverage goals,
existing-definition identities and the selection process. Future task code and
checks must be frozen individually when they exist, then together for the final
confirmation suite. No model invocation is requested by this document.

## Composition and weights

Target 20 tasks in 10 families, exactly two slots per family. Each family has
10% weight; each task has 5% weight. Five engineering families and five language
families give 10 slots per primary view. Existing tasks labeled both retain
that metadata; their planned slot names the primary coverage being assessed.
Secondary skills are recorded, but do not create extra family weight.

| Family | Primary view | Slot A | Slot B |
| --- | --- | --- | --- |
| Async lifecycle and delivery | Engineering | Existing async-durable-delivery | Existing async-generation-cache |
| Incremental graph consistency | Engineering | Existing incremental-atomic-rewiring v2 | Existing incremental-budgeted-propagation |
| Mutable collection transactions | Engineering | Existing base-nested-transactions | New coordinated bulk-index mutation task |
| Event-time aggregation and allocation | Engineering | Existing window-watermark-allocation | New bounded event-time join task |
| Persistent storage and recovery | Engineering | New interrupted-write recovery task | New checkpoint/compaction consistency task |
| Typed serialization and framing | Language | Existing bin-prot-tagged-transaction v3 | Existing typed-frame-stream |
| Typed syntax and binders | Language | Existing typed-query-optimizer | New typed transformation across recursive binders |
| Comparator-aware persistent structures | Language | Existing interval-map-transaction | New augmented persistent ordered structure |
| Modules and abstract type identity | Language | New generative-module-registry task | New first-class module/functor integration task |
| Compiler representation and runtime safety | Language | Existing compiler-gadt-field-safety, blocked | New reproducible compiler/runtime task to source |

The table reserves eleven existing definitions (ten calibrated library tasks
and one compiler task) and nine new tasks. These are proposed slot occupants,
not admitted publication tasks. New task descriptions are sourcing briefs, not
claims that an upstream bug exists or that the named design is already sound.

Two tasks in a family need complementary obligations, not cosmetic variants.
For example, storage recovery must exercise actual persisted-byte recovery and
checkpoint consistency, rather than duplicate the mock journal acknowledgements
in durable delivery. A bulk-index task must exercise coordinated invariants
beyond the existing table's savepoints. Recursive-binder work must add a real
scope/type obligation beyond renaming the query optimizer. Record secondary
skills and supplied dependencies in each task's prospective overlap review.

## Existing-definition selection

Selection uses coverage and contract quality, with known development outcomes
explicitly disclosed. It was made after those outcomes, so it is not an
independent selection experiment or a confirmation score.

- Full Incremental atomic rewiring v2 occupies the atomic slot. The small
  incremental-atomic-graph remains historical diagnostic evidence, not an extra
  graph slot. Original full atomic and corrected v2 are versions of one task.
- Full bin_prot tagged decoding v3 occupies the migration/native-codec slot.
  The older typed-schema-evolution has an unresolved policy for callback-raised
  public Decode exceptions. Its 1/3 result stays preserved, without retroactive
  clarification or selection solely for its lower score.
- typed-frame-stream supplies the complementary incremental framing obligation.
  bin-prot-open-union remains supporting development evidence. Its completed
  Tagged dependency was supplied, so it cannot earn an additional independent
  serialization family weight. Serialization v1, v2 and v3 are task revisions.
- async-bounded-dispatch remains a reserve easy anchor. Durable delivery and
  generation-safe cache cover a broader combination of delivery and lifecycle
  obligations within the same two-slot Async capacity.
- Existing 3/3 tasks are not silently deleted to lower the score. They remain
  prospective anchors where their coverage justifies a slot. A slot replacement
  requires a recorded coverage/contract reason and a prospective plan revision,
  with all earlier results and identities preserved.

Every latest logical task, including reserves and superseded/supporting tasks,
is assigned exactly one primary family in plan.json. Historical definition
versions are pinned separately in FREEZE.json. This grouping supersedes the
older contextual use of "twelve library families": that arithmetic described
12 task workloads, not these 10 prospective semantic families. Historical
31/36 (86.1%) remains contextual development arithmetic, without reweighting or
rescoring it. Tagged v3 remains 2/3 and open union remains 3/3 separately.
No score for the unbuilt 20-slot plan is calculated.

## Sourcing and admission

Prefer actual repository integration where it is relevant. The final composition
must contain at least eight tasks using complete pinned upstream repositories,
including at least four verified OSS bug fixes. An original extension does not
count as an upstream bug fix. Reproducing a defect on the pinned baseline and
validating the correction is required before assigning that classification.
These targets are future requirements, not claims about the existing inventory.
Public-source exposure, licenses and supplied prior solutions are disclosed;
repository size and generated fixture count do not establish difficulty.

Admission requires an explicit public issue and fixed interfaces, a correct
reference, hidden semantic checks and compiling faulty controls. Freeze callback
ordering, error precedence, rollback, allocation bounds and relevant module/type
identity behavior before target calls. Every checked obligation must be public.

Require three fresh offline base/reference pairs: every base behavior group
fails for its missing feature and every base guard passes; gold passes all
behavior and guards. Each control must compile, preserve guards and fail its
intended semantic check. Protect actual interfaces/fixtures/dependencies without
freezing harmless helper placement. Require standard validation and applicable
CI. Verify native allocation limits under native compilation. Record meaningful
upstream-test omissions rather than claiming a complete upstream test run.

Then run three fresh serial development attempts per task at the pinned settings.
Preserve successes, semantic failures, timeouts, provider/invocation failures and
interrupted/exposed traces. Replay semantic failures offline without adding
scored attempts. Infrastructure faults or exposure do not establish difficulty.
A task can be an intentional easy anchor; admission does not require it to fail
the calibration model. Do not pick only the lowest-scoring version or discard
legitimate passes to reach the difficulty target.

## Confirmation and reporting

After all 20 slots have admitted, exact task definitions, freeze a new runnable
suite manifest with task hashes, all metadata/setup/control hashes, source
provenance and execution conditions. This plan is not that runnable manifest.
Selection/development attempts are not reused as final confirmation attempts.
Run three fresh attempts for every selected task at unchanged settings, for
60 planned scored outcomes, and report confirmation even if it exceeds 80%.
No outcome-driven substitution is allowed within that frozen confirmation.

For task t, p_t is complete passes divided by scored attempts. For each family,
p_f = (p_A + p_B) / 2. Overall score = sum(p_f) / 10. With exactly three
attempts per task, this equals total complete passes / 60. A point estimate
strictly below 80% requires at most 47/60 passes; 48/60 is exactly 80%.
The preferred 60% to 75% development range remains a design target, not a
selection rule for dropping passing tasks. Report per-task and per-family
counts and uncertainty assumptions. Three attempts do not establish that a
confidence interval's upper bound is below 80%.

Missing slots, missing scored attempts or an unresolved provider condition
withhold the overall headline. Never renormalize over surviving families or
turn unscored provider errors into failures. Partial tables retain intended
weights and coverage. The compiler slot stays blocked by the preserved provider
refusal; no automatic retry, prompt rewrite or routing around that refusal.
Resolve that condition before invocation, or make an explicit prospective plan
revision with its coverage loss disclosed. A compiler-free library subview may
be shown as a named partial view, never as the full plan's score.

A fresh workload authored after development feedback, including open union, is
not independent external confirmation. Publication requires a separately
reviewed frozen composition, clean confirmation receipts and an explicit account
of remaining authoring, public-source and model-identity limitations. Functional
completion does not imply OCaml quality/security analyzer coverage.

## Operating conditions and artifact hygiene

Keep Codex CLI 0.159.0, requested GPT-6.1 Sol medium, subscription, OCaml 5.2.1,
Dune 3.17.2 and the same 96 library package pins. Compiler tasks retain their
separately documented compiler revision/seed. Native workspace-write,
concurrency one, no container/judges, 10800-second solve allowance and 540
configured steps remain fixed; the CLI does not independently enforce a turn
cap. Keep documented task-specific setup/check headroom. No new package,
effort/budget change, expired automation or automatic provider retry is implied.

Authoring/reference/check material stays in ignored runs directories, not global
tmp or solver workspaces. Before each target call verify source, scoring,
metadata/control, toolchain and validated code hashes. After each call quarantine
shared temporary artifacts only when positively attributed to solver commands
or compilation, preserving paths/hashes and a cleanup receipt before the next
call. Stop for review on unattributed new OCaml artifacts, leftover task
workspaces or observed earlier-attempt artifact reads. Audit traces manually:
telemetry missed the open-union cohort's cross-attempt smoke-test reuse.

The retired union cohort's two raw passes and interrupted trace remain preserved;
the clean cohort remains 3/3. Cleanup is disclosed artifact hygiene, not OS read
isolation. Do not add the incompatible outer macOS sandbox guard. Native target
execution retains observational integrity, no enforced network isolation and no
Docker resource caps. Offline validation separately denies network.

## Revision and next authoring priority

This v1 blueprint is immutable once its FREEZE.json is written. Change capacity,
weights, slot occupants or admission/reporting policy through a separate v2
plan, documenting the evidence and known outcomes available at revision time.
Do not edit the v1 bytes or an existing scoring pool to make a new result appear
prospective. Preserve the full working-copy/run receipts; this plan does not
create a commit, publish a suite or launch a model.

Recommended next step: Author generative-module-registry for the unfilled
modules/type-identity family. First specify safe heterogeneous key equality,
abstract payload boundaries, first-class module packaging and handle lifetime
semantics, then validate a reference and controls before calibration. This adds
coverage absent from the current selected workloads rather than another
serialization variant. Keep secondary transaction overlap explicit.
