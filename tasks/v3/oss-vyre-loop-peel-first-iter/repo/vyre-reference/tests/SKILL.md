# tests/SKILL.md  -  vyre-reference

Read `../../.internals/skills/testing/SKILL.md` first for the category contract.

## Purpose

`vyre-reference` is the **pure-Rust CPU interpreter**. Every GPU
backend's conformance proof is anchored against the byte-for-byte
output this crate produces for every witnessed input. Determinism
is the contract.

## Critical invariants

- **Determinism.** Same `Program` + same inputs + same reference
  version → same output bytes, forever. No HashMap iteration
  order, no random ordering, no thread-scheduler dependence.
- **Bit-identical to spec.** Every arithmetic / bitwise / atomic op
  follows the exact semantics documented in `vyre-spec` +
  `docs/memory-model.md`. Reference is the oracle.
- **Sequential workgroup ordering.** The reference runs workgroups
  in index order; GPU can legally interleave, but the reference
  fixes ONE legal interleaving so certificates are reproducible.
- **Zero unsafe.** The interpreter is pure safe Rust (unsafe is
  denied crate-wide).
- **No panic on any validated program.** If `validate(p)` returns
  `Ok`, `run(p, inputs)` never panics (errors may arise from
  runtime conditions like OOB; those are structured errors).

## Adversarial surface

- Program with maximum-size workgroup memory  -  budget enforced
- Program with 10 000 loop iterations  -  bounded, no overflow
- Atomic operations on the same slot from every invocation in a
  workgroup  -  serialization correct
- Shared memory write from invocation N visible to invocation N+1
  (sequential semantics)
- Barrier semantics  -  invocations replay from the barrier
- Integer overflow on every arithmetic op  -  spec-defined wrap

## Active coverage

- Workgroup execution uses the hashmap interpreter oracle with
  persistent locals for cheap subgroup snapshots.
- `NodeStorage` graph execution is covered by randomized DAG
  tests that compare against an independent recursive oracle.
- Dual-reference coverage is enforced by the registry tests for the
  bitwise primitives that currently publish independent references.

## Cross-crate contracts

- Backend conformance uses `vyre-driver::shadow::ReferenceExecutor`
  to wire this interpreter without creating a driver/reference
  dependency cycle.
- Consumes `vyre_foundation::Program`, `vyre_foundation::ir::*`
- Consumes `DialectLookup` (post-0.6)  -  routes `Expr::Call` through
  the registry rather than matching on op-id strings
- Output `Value` is consumed by conform runners + byte-identity
  proofs in every backend

## Bench targets

- `run_arena_reference`  -  programs / sec for small programs
- `run_hashmap_reference` (differential oracle)  -  programs / sec;
  target 10× slower than arena reference is acceptable (that's
  the oracle's whole point)
- Throughput under shared-memory loads

## Fuzz targets

- `run_fuzz`  -  arbitrary valid `Program` + arbitrary inputs → no
  panic, never silent wrong answer
- `differential_fuzz`  -  same Program through `run_arena_reference`
  and `run_hashmap_reference` → outputs match

## What NOT to test here

- GPU dispatch  -  concrete driver tests
- Wire format  -  `vyre-foundation/tests`
- Op metadata  -  `vyre-spec/tests`

## Running

```bash
./cargo_full test -p vyre-reference
./cargo_full test -p vyre-reference --test adversarial
./cargo_full test -p vyre-reference --test property
./cargo_full test -p vyre-reference --test gap
./cargo_full test -p vyre-reference --test integration
./cargo_full bench -p vyre-reference
cd vyre-reference/fuzz && ../../cargo_full fuzz run differential_fuzz
```
