# Compiler field-safety candidate

Status: admitted after complete validation of the revised public expect workflow.
Three fresh base/reference pairs, all three compiling faulty controls, and the
standard harness gate passed. VALIDATION_WORKFLOW.json preserves this proof.
Source, prompt, and reference are unchanged from the core-only definition. The
revised task has zero fresh model outcomes. The old core-only pass, replay gap,
and provider refusal remain separate evidence. No sub-80% or training-cutoff
claim is made.

The starting snapshot contains all 4984 tracked files at OCaml revision
`364874344779d2e410e99eb634bea8e7e2b70159`, including the upstream test corpus.
The reference is the full diff through `a47e535288e8e713a4c2ebf9b93d5a069057a3c4`
from [PR 15114](https://github.com/ocaml/ocaml/pull/15114). Upstream licensing and
notices are retained. PROVENANCE.json records revision, hashes, source size,
public build-seed identity, and both images.

| Requirement | Independent check |
|---|---|
| Shared record fields retain native GC roots | 64 minor-collection repetitions, both row orders, nested mutable records |
| Known tuple fields are precise | Integer, boolean, character, and string Lambda classifications |
| Constructor fields are precise | Mixed immediate/pointer arguments at offsets zero through three |
| Extension argument offsets are correct | Descriptor at zero, arguments at offsets one through four |
| Manifest aliases and immediate annotations remain useful | Tuple alias, constructor alias, abstract immediate type |
| GADT matching remains safe | Tuple/constructor row orders, tuple shape visible only through an equation |
| Match evaluation behavior is preserved | Inlined records, mutation, unboxed representation, extension payload, guard effects |
| Normal type refinement is preserved | Accepted GADT client and rejected mismatched client |
| Supplied boundaries remain intact | Existing interfaces and all 152 tracked runtime inputs |
| Public expect maintenance | All four affected upstream expect tests execute successfully |

The guards permit helper declarations in Env without removing its existing
declarations. Other supplied interfaces and runtime source remain fixed. Grading
checks behavior and emitted field kinds, without requiring the reference's helper
name or exact implementation.

Three incomplete solutions must compile and preserve all regression guards while
failing their intended behavior requirements:

- Record-only repair omits tuple/constructor/extension precision.
- Blanket pointer reads preserve GC safety but lose immediate-field precision.
- Row-local record equations retain the root omission despite other improvements.

Initial diagnostic gold passed all eight hidden checks and five affected upstream
tests. The public-test recipe requires `OCAMLSRCDIR=$PWD` because the baseline
ocamltest build was relocated. PUBLIC_TESTS_GOLD.json preserves the corrected
supplemental results. This reused workspace is not a clean validation receipt.

The release-archive packaging failure and the first full-snapshot setup failure
are preserved in their named JSON receipts. Two parallel rebuilds failed during
native compilation with an unbound Types module; a single-job retry completed.
The cause is unconfirmed. Three fresh single-job base/reference repetitions
passed, but a control hit an unrelated ocamldoc link error caused by cached Unix
and rebuilt Stdlib implementation digests. VALIDATION_OPT_OPT_CONTROL_FAILURE.json
preserves the complete receipt. The final recipe builds the compiler and public
expect helper without linking unused documentation tools. Repeat the complete
reference/control/standard gate for that recipe before moving the task into
ocaml-v1. No behavior or interface assertion was weakened.

All validation is offline with fresh non-root Docker workspaces. The solver
configuration remains GPT-6.1 Sol medium, three attempts, serial execution, and
a three-hour ceiling. Compiler setup and individual grading commands have
20-minute rebuild headroom; see docs/DECISIONS.md. The compiler under test is the
pinned upstream 5.6.0+dev0, distinct from the seven original fixtures' 5.2.1 image.

Once admitted, preserve the seven-task reports, withhold an expanded aggregate
until all eight tasks have complete coverage, and retain every matching model
outcome. Treat the measured score as development calibration rather than
independent confirmation. Do not launch an overnight attempt without enough
remaining time for the full solve budget and setup/grading headroom before the
owner's 9 AM PDT stop time. If three attempts cannot complete under that rule,
leave calibration incomplete rather than tightening a budget or inventing a score.

## Historical core-only admission receipts

VALIDATION_CORE.json records three fresh base/reference pairs, all three compiling
controls rejected with guards intact, and standard harness PASS with gold 1.0,
base 0.0, and three deterministic reference runs. PUBLIC_TESTS_CLEAN.json records
four affected public tests at fresh base and those four plus the new native GC
regression at fresh gold. These complement the older preserved diagnostic and
failure receipts. This is not a claim that the entire upstream test corpus or a
cold full world.opt reference build has been validated.

Inspect and replay each model patch in a fresh offline workspace after calibration.
The primary harness grades the solver workspace; fresh replay verifies that the
captured source patch survives rebuilding without solver-generated artifacts.

## Public-workflow coverage revision

Clean model-patch replay reproduced the original eight-check pass but failed
basic/patmatch_split_no_or.ml because expected bool-field reads were not updated.
The other affected public tests passed. The already stated expectation-update
obligation is now checked by public_expect_workflow. This is a grading revision,
not a fresh model failure. The old definition, tests, controls, reports, and
refusal receipts remain reproducible. Compiling controls may update public expect
output to match their own implementation, while independent private checks still
reject their semantic defects. No test program bodies or interface/runtime guards
are removed. Repeat the complete validation gate before renewed admission.


## Renewed workflow admission

VALIDATION_WORKFLOW.json records three complete fresh base/reference pairs,
all three semantic controls with all four regression guards intact, and standard
validator PASS. CONTROL_PUBLIC_EXPECTATIONS.json records normal public-test
success after expectation-only promotion; original controls remain in
CORE_ONLY_CONTROLS. All scored verdict names match the current five behavior
checks and four guards. The current source hash and control hashes are recorded
in the suite receipt. Core-only model evidence is not a revised-task score.

## Supplemental cold reference build

COLD_REFERENCE.json records one fresh offline configure/world.opt build with no
public-seed installation, followed by all nine unchanged assertions and the new
upstream GC regression. Every command passed at the current frozen task hash.
The full build took 1048.8 seconds under emulation. This confirms a full cold
reference build independently of warmed artifacts; it does not establish repeated
cold determinism or success across the entire upstream test corpus. The repeated
admission gate and model-calibration limitations remain separate.
