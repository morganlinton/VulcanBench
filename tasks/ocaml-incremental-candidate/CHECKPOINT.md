# Incremental candidate checkpoint

Completed: the final candidate passed all offline and harness gates, and
GPT-6.1 Sol medium passed 3/3 fresh native attempts. All jobs have exited.
The contextual eleven-library result is 29/33 (87.9%), so the difficulty goal
remains unmet. CALIBRATION.md contains the final result and limitations.

The owner accepted the suggested full-repository Incremental task. This work
uses the native Mac toolchain on codex/ocaml-v1, separately from the expired
overnight automation. No Docker or subagents are used. The original ocaml-v1
identity c0a1cc221d60 and three-candidate identity e0a390e323df remain unchanged.

incremental-budgeted-propagation is an original extension to the complete
128-file Incremental v0.17.0 checkout at revision
61baea591be9bfaf512bb2fcdf176753f67b2607. The source is public and retains its MIT
license. Public scaffolding supplies both generative and generic APIs. The
snapshot has 15,832 code lines with scaffolding, so navigation metadata is large.
The supplied interface defines a bounded propagation driver. The task requires
exact node quotas including recursive direct-parent work, resumable dynamic
dependencies, publication fences, deferred writes, all-driver interoperation,
state-local reentrancy rejection, and preservation of exception poisoning.
Ordinary unbounded optimizations must remain enabled. Every requirement is in
issue.md before model calls. Seven behavioral groups and two regression guards
cover the contract. All .mli files and incremental_intf.ml are guarded.

Initial gate VALIDATION_OFFLINE_INITIAL.json/log failed the regression fixture:
it expected direct-parent recomputation during initial graph setup, when every
necessary node was already queued. The regression now checks after a variable
update. The failed receipt is preserved. The implementation was not weakened to
meet the test. No model attempt occurred against that fixture.

The corrected full offline gate and full harness CI ran before calibration. Four compiling
controls challenge heap-pop quota accounting, an extra completion slice,
accepting step reentrancy, and restarting a paused cycle. Do not launch model
calls before all repeated references, controls, interfaces and standard
validation pass. Preserve every failure receipt and correct infrastructure or
fixture problems before freezing the candidate.

The second gate passed all three clean reference pairs, then rejected the
extra-empty-slice control during setup because Core's integer comparison
operator did not accept an int option. VALIDATION_OFFLINE_CONTROL_BUILD_FAILURE
preserves the receipt and log. The control now pattern matches the option.
That compiler failure does not count as evidence of semantic sensitivity.
The full gate reran before any model call.

Calibration must use three fresh attempts, GPT-6.1 Sol medium, pinned Codex
0.159.0, subscription billing, serial concurrency one, local sandbox, no agent
container and no judges. Inherit 10800-second solve and 540 configured-step
defaults without overrides. Stop on invocation or provider errors without
automatic retries. The native environment PATH must explicitly include the
pinned CLI; the host default CLI is 0.149.0. Do not reactivate the overnight
automation or retry the unresolved compiler provider refusal.

This is a one-task development pool, not independent confirmation of the full
suite. Previous results remain 7/9 for the candidate trio and 26/30 for the ten
native library tasks combined. The compiler remains pending. Requested-only
model identity, observational integrity telemetry, public-source exposure and
native execution without Docker resource/network isolation remain disclosed.
Offline reference validation denies network for the complete process tree.
HTML visual rendering remains unverified under the existing app preview limit.

A separate temporary-reference @all probe failed because test-debug/dune needs
bin/gen-explicit-dependencies.sh, absent from the pinned public checkout, and
debug generators use cp/sed conventions incompatible with this Mac. It also
revealed a test adapter requiring forwarding of the new API. That forwarding
is now supplied in both baseline and reference scaffolding. The previous full
passing gate is preserved as VALIDATION_OFFLINE_PRE_ADAPTER.json/log. The final
setup compiles src/incremental.cmxa and test/incremental_test.cmxa explicitly and
runs both public clients through @runtest. Test and public fixtures are guarded
and the stated contract includes that preservation. The temporary reference
passed this broader portable target. The gate then reran on the final snapshot.
UPSTREAM_ALL_PROBE.json records limitations; every upstream inline/debug test
does not execute. Missing debug infrastructure is not a model failure, and no
external generator is fetched.

Full harness CI passed: 1094 tests, five unrelated analyzer skips, four
deselections, one dependency warning. The focused reporter/validation tests
passed 27 cases; reporter mypy passed. CI.json/log preserve code digests.

The final complete offline gate passed with the broader portable setup:
three clean base/reference pairs, all four compiling controls rejected for
semantics, all interface and fixture guards, and standard validation PASS.
Frozen identity is 6b37b41277dd. The source, prompt, checks, metadata and controls
are now frozen for three fresh native attempts in
../../runs-ocaml-incremental-20261002T135901Z. The single-attempt dispatcher is
complete and scoped caffeinate has exited. LAUNCH.json/PROGRESS.json record task, code,
conditions and toolchain identities and retain every outcome.

All three attempts passed: 3/3 complete tasks, each 7/7 behavior groups and 2/2
guards. There are no provider errors, retries, excluded or unscored outcomes.
RECEIPT_AUDIT.json confirms unchanged code, conditions, both earlier identities,
guarded fixtures and clean patch inventories. Recorded telemetry shows no
benchmark/answer-key reads or web activity; native observation is not isolation.
The three solutions use graph-local guards and queue direct parents only during
bounded propagation. None adds an unsafe cast, registry or counter forgery.
No failing model patch needed replay. Full calibration and limitations are in
CALIBRATION.md/json and EXPANSION_CONTEXT.json. JSON/HTML cards have complete
one-task coverage. No model, validation or caffeinate workload remains active.

The four larger candidates together are 10/12 (83.3%). Contextual eleven-library
results are 29/33 (87.9%); the under-80% target remains unmet. The complete
twelve-task headline stays withheld while the compiler invocation is unresolved.
No old scoring inputs or receipts were overwritten or selectively omitted.

Suggested next step: build a full-repository atomic dependency rewiring task,
using the original atomic-graph task's 1/3 result as evidence for a harder
engineering direction, then validate and calibrate it at unchanged settings.
