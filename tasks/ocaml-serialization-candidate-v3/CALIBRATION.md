# Typed serialization calibration

GPT-6.1 Sol medium completed 2/3 fresh native attempts (66.7%) on
frozen candidate fa86068dce81. This is a standalone development calibration,
not independent confirmation, suite publication or a combined suite headline.

| Attempt | Complete pass | Behavior groups | Guards | Duration |
| --- | --- | --- | --- | --- |
| 1 | No | 7/8 | 2/2 | 192.275 seconds |
| 2 | Yes | 8/8 | 2/2 | 232.233 seconds |
| 3 | Yes | 8/8 | 2/2 | 290.994 seconds |

All three attempts used pinned Codex CLI 0.159.0, requested GPT-6.1 Sol medium,
subscription billing, native local workspace-write execution, serial concurrency
one, no container/judges, and unchanged 10800-second / 540-configured-step budgets.
Configured steps are not independently enforced as CLI turn limits. Requested
model identity is not independently attested. No automatic retry was used.

Functional failures were replayed from their exact source patches in fresh
native workspaces under network denial. All behavior and guard verdict masks
reproduced; replay adds no scored attempts. Failed groups:

- Attempt 1: wire_compatibility.

The offline admission gate passed three fresh base/reference pairs, seven compiling
controls with intended semantic failures, protected interface/encoder/fixture hashes
and standard validation. The first failed control gate is preserved separately.
The reference allocated 968 OCaml heap bytes for the 2 MiB unknown-field probe.
Full CI passed 1103 tests with 5 unrelated skips, 4 deselections,
one dependency warning and 86.38% coverage. All 36 focused reporter/
validation tests and reporter mypy passed. Native preflight compiles the bin_prot
library and runs upstream fixture checksums and public clients. The upstream test
library requires missing float_array and was not built; no claim that every
upstream inline test ran. The pinned 96-package toolchain was unchanged.

Receipt/source audits verify frozen code, conditions, public source, all five completed
earlier pool identities and both partial serialization definitions. There were no exclusions or unscored outcomes in this
cohort and no observed benchmark/answer-key reads, protected fixture/interface
edits, generated build-file patches or newly added unsafe casts. SOURCE_REVIEW.json
records the final implementation review. Native telemetry is observational,
without OS benchmark read isolation, network isolation or Docker resource caps.
Authoring material remained in the ignored checkout directory, outside global /tmp.

The complete bin_prot v0.17.0 source retains licenses and notices. This is an
original public-source extension, not an upstream bug or a training-cutoff claim.
Jane Street relevance is inferred from public sources, without endorsement. The
large checkout includes generated fixtures; size alone is not difficulty. The
contract explicitly requires unchanged user exception propagation, including
Tagged.Decode, with caller cursor restoration and bounded native Atom subviews.

Prior twelve-library contextual results remain 31/36 (86.1%). This candidate
shares record-migration skills with the earlier small schema family and adds
native readers and stream integration. Its score stays separate until a frozen
suite composition chooses replacement or additional weighting. The suite-wide
below-80% goal remains unestablished. The compiler refusal is unresolved; no
compiler retry or expired automation was started. Functional cards do not claim
OCaml quality/security analyzer scores. HTML structure/escaping is tested, but
visual rendering remains unverified under the existing preview limitation.

Receipts and JSON/HTML cards: ../../runs-ocaml-serialization-20261004T175256Z. All model, validation, replay
and scoped caffeinate jobs have exited.

Two earlier definitions remain preserved separately. Original 055b3d9fbec1
and v2 040052f00a9c each produced one complete pass before interruption on their
second attempts. The original prefix guard exceeded the public contract and was
narrowed in v2. V2 source review then revealed a reference short-payload Utils
integration gap, reproduced offline. V3 keeps the issue/public source unchanged,
fixes the reference exact payload view and adds the old reference as a seventh
control plus short/empty stream assertions. HELPER_PLACEMENT_AUDIT.json confirms
valid helper placement still passes. Raw passes, interrupted traces and cards
are not rescored or pooled with v3.

Suggested next step: Run independent confirmation on a held-out serialization task with the same native settings before admitting this family to a frozen expanded suite.
