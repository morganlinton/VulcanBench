# OCaml atomic rewiring candidate v2

A separate development pool containing incremental-atomic-rewiring, an original
extension to the full Jane Street Incremental v0.17.0 checkout. This pool follows
the completed factory handoff and preserves the identities of ocaml-v1,
ocaml-candidates and ocaml-incremental-candidate. It is not an expanded suite
publication or independent difficulty confirmation.

The task replaces several expert nodes' dependency lists against one prospective
graph. It exercises graph validation, ownership transfers, backlink and index
maintenance, callback ordering, observer publication, no-op work suppression,
necessity, and propagation through ordinary Incremental nodes. Its error order,
top-scope restriction and callback/resource preconditions are in the public
issue. Hidden checks add no requirements beyond that contract.

Use the existing native OCaml 5.2.1, Dune 3.17.2 and 96-package TOOLCHAIN.lock.
The preflight builds the Incremental library and upstream test library and runs
two public clients. The checkout's incomplete/platform-specific debug generator
infrastructure prevents claiming that every upstream inline/debug test executes.
The installed package source restored only the three files modified by the older
candidate's bounded-driver scaffolding; provenance records those paths. The
restored source retains its MIT license and public-source training exposure.

Validation requires three fresh offline base/reference pairs, each base failing
all seven behavioral groups and preserving both guards, and each reference
passing every check. Five complete compiling faulty controls cover sequential
application, eager callbacks, no-op recomputation, skipping ordinary nodes in cycle detection, and stale invalid-child bookkeeping. CONTROL_EXPECTATIONS.json records their intended sensitivity.
Interface hashes include all .mli files and incremental_intf.ml; supplied public
and upstream test fixtures are guarded too.

Calibration uses three fresh serial GPT-6.1 Sol medium attempts through pinned
Codex 0.159.0, subscription billing, local sandbox, no agent container or judges,
and unchanged 10800-second / 540-configured-step solve allowances. A provider or
invocation error stops the dispatcher without automatic retry. The compiler's
unresolved provider refusal and the expired overnight automation remain separate.
Native model execution has no Docker resource caps or network/filesystem
isolation. Requested model identity and integrity telemetry are observational.
Functional model cards do not claim OCaml quality or security analyzer coverage.

This is a prospective revision of the same task family, not another distinct
suite task. REVISION.json explains the reference invalid-child-count fix and
one added assertion of the unchanged public prospective-graph contract. The
original pool and model receipts remain frozen and are not rescored. Require a
new full offline gate, full CI and three fresh unchanged native attempts before
reporting the revised task's calibration. Preserve the earlier authoring exposure
and sandbox-invalid outcomes separately; they are not clean capability evidence.
Use the original pinned Codex workspace-write execution, with authoring artifacts
relocated out of global /tmp and observational trace review for benchmark reads.
