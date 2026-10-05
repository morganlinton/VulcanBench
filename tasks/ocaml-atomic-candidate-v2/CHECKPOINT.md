# Atomic rewiring v2 checkpoint

Current status: complete. The corrected reference and all five controls passed
the full offline gate; full CI passed 1100 tests. Fresh native calibration is
2/3 complete tasks, with one semantic failure reproduced offline. All jobs have
exited. CALIBRATION.md is the authoritative current result and limitation note.

The public contract and baseline are unchanged from b9b088b09fa9. A supplemental
reference audit exposed retained invalid-child bookkeeping when removing an
invalid old child from a still-valid expert. The original reference failed an
offline reproduction. This revision resets the count after clearing obsolete
edges and adds the existing prospective-graph requirement to mixed_graph.
Original definitions and model receipts remain frozen. This is the same task
family, not an additional distinct task for aggregate weighting.

Admission and fresh calibration are pending. Require three offline base/reference
pairs, compiling controls with intended failures, interfaces/fixtures, standard
validation and full CI. Then three fresh serial GPT-6.1 Sol medium attempts with
Codex 0.159.0, subscription, local sandbox, original workspace-write CLI policy,
no agent container or judges, 10800-second / 540-step defaults. Stop on invocation,
tool-environment or benchmark-material exposure findings without automatic retry.
Authoring artifacts stay outside global /tmp. Native read integrity is observation,
not OS isolation. No compiler provider-refusal retry or expired automation runs.

The final clean offline gate passed: three clean pairs, all seven behavior
checks, both guards, five compiling controls rejected as intended, and standard
validation PASS. CONTROL_AUDIT.json records each intended failure. Frozen pool
identity is 1ce29fa16e4f. All four earlier measured identities are unchanged.
Full CI passed 1100 tests, with 5 unrelated analyzer skips, 4 deselections,
1 dependency warning and 86.38% coverage. CI.json records validated code
digests. Calibration is ready for three fresh serial attempts in the directory
recorded by RUN_DIRECTORY.txt. No model invocation occurred before these gates.

The revised cohort completed in runs-ocaml-atomic-v2-20261003T164402Z. Attempts
1, 2, 3 scored 0.8571, 1.0, 1.0 functionally, giving 2/3 complete task passes.
The first failed mixed_graph because removal did not fix invalid-child counts.
A fresh offline patch replay reproduced all behavior and guard outcomes; it is
not a new scored attempt. Both passing solutions handle the count with existing
state machinery. Source and receipt audits found no guarded fixture changes,
unsafe casts, counter forgery or observed benchmark/answer-key reads. Source,
conditions, toolchain and validated code stayed fixed through every dispatch.

Earlier pools remain c0a1cc221d60, e0a390e323df, 6b37b41277dd and b9b088b09fa9.
The latest task is the same family as b9b088b09fa9, so its three earlier passes
are not double-counted. Twelve current library families give contextual 31/36
(86.1%). Five larger candidate families are 12/15 (80.0%). Neither establishes
a full-suite below-80% result or independent confirmation. The thirteen-family
headline is withheld while the compiler provider refusal remains unresolved.
All jobs have exited; no automation, compiler retry or subagent was started.

Suggested next step: build a full-repository typed serialization candidate with
explicit constructor-exception rollback, then validate and measure at the same
settings to address the remaining language-mastery difficulty gap.
