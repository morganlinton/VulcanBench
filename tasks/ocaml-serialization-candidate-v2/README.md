# OCaml typed serialization candidate v2

A separate development pool for bin-prot-tagged-transaction, an original extension
to the complete Jane Street bin_prot v0.17.0 source at commit
2cd58a8cd74b5fa1f28d63cbc4deaf6665b82525. Existing measured pools remain frozen.
Upstream licenses and notices are preserved. Public-source exposure is disclosed;
there is no decontamination, internal Jane Street access or endorsement claim.

The task combines heterogeneous GADT record construction, versioned field defaults,
explicit callback exception identity, cursor rollback, zero-copy bounded native
Atom readers, exact structural error positions, opaque unknown-field allocation,
and standard bin_prot framed stream integration. Its public issue specifies
callback ordering and exception policies before calibration. It does not alter
old schema-evolution checks or reinterpret earlier scores. The full source includes
large generated upstream test fixtures; repository size alone is not difficulty.

The reference and six compiling semantic controls must pass the complete native
offline admission gate: three fresh base/reference pairs, base failure of every
behavior group, preserved regression guards, protected supplied interface/encoder
and public/upstream fixture hashes, and standard validation. The initial gate is
preserved: one overly broad unbounded-Atom control broke public migration at setup.
Its replacement exposes the same missing subview bound while preserving public
migration. The whole-buffer copy control materializes an OCaml string so the heap
allocation probe measures its intended mistake. CONTROL_AUDIT.json verifies
intended sensitivity independently of the aggregate functional score.

The pinned native environment is OCaml 5.2.1, Dune 3.17.2 and the same 96 opam
packages. float_array is absent, so the upstream test library is preserved but not
built. Preflight compiles src/bin_prot.cmxa, runs the upstream generated-fixture
checksum rule, and runs the supplied public smoke/migration clients. No claim that
all upstream inline tests execute. No Docker or new packages are required.

Calibration requires three fresh serial GPT-6.1 Sol medium requests through
Codex CLI 0.159.0, subscription billing, local sandbox, no agent container or
judges, and unchanged 10800-second / 540-configured-step solve budgets. Preserve
all attempts and stop on provider/invocation errors without automatic retry.
Use original workspace-write sandboxing: the earlier external read guard is
incompatible with target shell tools. Keep authoring material out of global /tmp.
Native integrity checks are observational, without OS benchmark read isolation,
Docker resource caps or enforced network isolation. Model identity is requested-only.

This is development calibration, not independent confirmation or suite publication.
Typed record migration overlaps the older schema family; native arbitrary readers
and stream integration add distinct work. Report this candidate separately until
an explicit frozen composition decides whether to replace or additionally weight
that earlier family. The unresolved compiler refusal and expired automation remain
separate and are not restarted.

Revision: the original 055b3d9fbec1 guard overprotected a source prefix. Its first
native attempt passed all checks and is preserved; its second trace was interrupted.
This revision changes only the guard to protect actual supplied declarations and
files. HELPER_PLACEMENT_AUDIT.json proves a harmless decoder-helper insertion
passes every behavior and guard. Require entirely fresh revised attempts; do not
pool versions. REVISION.json records the contract correction.
