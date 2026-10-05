# Atomic rewiring v2 calibration

GPT-6.1 Sol medium completed 2/3 fresh native attempts (66.7%) on revised identity
1ce29fa16e4f. One patch passed six behavioral groups and both guards but failed
mixed_graph: it omitted invalid-child bookkeeping when removing an invalid old
child from a still-valid expert parent. A fresh offline source-patch replay
reproduced the exact behavior/guard verdicts. This is a semantic failure, not a
compiler, provider or tool-environment error. The replay adds no scored attempt.

| Attempt | Complete task pass | Behavior groups | Guards | Recorded run duration |
| --- | --- | --- | --- | --- |
| 1 | No | 6/7 | 2/2 | 504.799 seconds |
| 2 | Yes | 7/7 | 2/2 | 501.165 seconds |
| 3 | Yes | 7/7 | 2/2 | 423.856 seconds |

All three attempts were serial, fresh GPT-6.1 Sol medium requests through pinned
Codex 0.159.0 on subscription, local native execution, no agent container or
judges, and unchanged 10800-second / 540-configured-step allowances. The original
Codex workspace-write sandbox is used; no external read guard is applied. The
configured steps do not guarantee CLI turn enforcement. The requested model
identity is not independently attested. Native read integrity is observational;
Docker resource caps and network isolation are absent. Authoring artifacts were
relocated out of global /tmp before this cohort. No provider/invocation error,
automatic retry, exposure, exclusion or unscored outcome occurred in this cohort.

The full offline gate passed three clean base/reference pairs, every behavior
and regression check, five compiling semantic controls and standard validation.
The original reference is one control, so the new invalid-child assertion has
proven sensitivity to its intended bug. Full CI passed 1100 tests with five
unrelated analyzer skips, four deselections, one dependency warning and 86.38%
coverage. All 33 focused reporter/validation cases and reporter mypy passed.
The portable public preflight compiles the library and upstream test library and
runs public clients. It does not execute all upstream inline/debug tests.

Receipt/source audits preserve exact code, source, toolchain and task-condition
identities. All four earlier pools retain their hashes. No patch changed supplied
interfaces/fixtures, captured generated build files, added an unsafe cast or
counter forgery, or showed benchmark/answer-key reads. The passing patches use
the existing dependency, necessity and height machinery; the failing one misses
a real invalid-child invariant. SOURCE_REVIEW.json and RECEIPT_AUDIT.json contain
the inventories and limitations. Functional cards do not claim OCaml quality or
security analyzer scores. HTML structure/escaping is tested; visual rendering
remains unverified under the existing local-file preview limitation.

This revision preserves the original public contract and baseline, corrects a
reference bug discovered through a model's public test, and adds one assertion
of that already specified prospective-graph behavior. The original b9b088b09fa9
pool and its three native passes remain frozen and are not rescored or pooled
with this revision. This is the same task family, not another distinct task.

An earlier pass that read a benchmark-authoring script is preserved as exposed
and excluded. Three zeroes from an incompatible external macOS read guard are
preserved as infrastructure-invalid, not model capability failures. An interrupted
trace and two preflight preparations remain separate, without fabricated scores.
Raw cards and summaries are retained. CALIBRATION_ELIGIBILITY.json review files
make the reporter exclude those outcomes without mutating their raw receipts.
The corrected reporter tests prevent invalid zeroes from manufacturing difficulty.

Contextual twelve-library results are 31/36 complete passes (86.1%). The five
larger candidate families together are 12/15 (80.0%), which is not below 80%.
These are development arithmetic across cohorts, not a frozen expanded-suite
card or independent confirmation. The full thirteen-family headline remains
withheld: compiler-gadt-field-safety has no current fresh native outcomes, and
its earlier provider refusal remains unresolved. No compiler retry or expired
overnight automation was started. Public-source exposure is disclosed, without
a decontamination or Jane Street endorsement claim.

Receipts and JSON/HTML cards are in
../../runs-ocaml-atomic-v2-20261003T164402Z. All model, validation, replay and scoped
caffeinate jobs have exited successfully. The suite-wide difficulty goal remains
unmet despite this candidate's observed 66.7% completion rate.

Suggested next step: build a full-repository typed serialization candidate with
explicit exception/rollback semantics, using the earlier schema-evolution
failures to strengthen language mastery at the same calibration settings.
