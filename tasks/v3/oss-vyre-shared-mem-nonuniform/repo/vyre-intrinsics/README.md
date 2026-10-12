# vyre-intrinsics

Category-C hardware intrinsics for vyre: the ops that cannot be
written as `fn(...) -> Program` compositions over existing `Expr` /
`Node` primitives because they require dedicated backend emitter arms
and dedicated `vyre-reference` evaluation arms.

If an op *can* be expressed purely as composition of existing
primitives, it belongs in `vyre-libs` or a consumer crate, NOT here.
The split is documented in `docs/migration-vyre-ops-to-intrinsics.md`.

## Invariants

1. **Every intrinsic has a CPU reference.** Each op here implements
   `CpuOp` / `CategoryAOp` from `vyre-foundation::cpu_op`; the
   conform runner diffs every backend dispatch against that
   reference byte-for-byte.
2. **Every intrinsic has a dedicated Naga emitter arm.** There is no
   fallback-to-composition path: if the backend can't emit it, the
   program fails validation up front with a `Fix:` hint.
3. **Every intrinsic has an `IntrinsicDescriptor` in the inventory
   registry.** `inventory::submit!` wires the descriptor at link time
   so the harness, spec tooling, and wire decoders discover it
   automatically.
4. **Composite builders do not belong here.** A `fn` that returns a
   `Program` by composing existing IR variants lives in `vyre-libs`
   (shared) or the caller's own crate. `vyre-intrinsics` is strictly
   Category-C: hardware-bound ops.
5. **Feature flags are additive.** `hardware` and `subgroup-ops` are
   on by default. Enabling a feature never removes or changes the
   signature of an already-enabled intrinsic.
6. **Region chains wrap every composition.** The `region` module is
   the mandatory wrap helper every tier uses; consumers of this crate
   get it re-exported so they don't hand-roll it.

## Boundaries

`vyre-intrinsics` owns:

- `subgroup_add`, `subgroup_ballot`, `subgroup_shuffle`: wave-level
  collectives backed by Naga 25+ subgroup lowering.
- `workgroup_barrier`, `storage_barrier`: concurrency fences.
- `bit_reverse_u32`, `popcount_u32`: bit intrinsics mapping 1:1 to
  hardware (`reverseBits`, `countOneBits`).
- `fma_f32`: fused multiply-add, byte-identical to `f32::mul_add`.
- `inverse_sqrt_f32`: hardware `inverseSqrt()` via naga.

`vyre-intrinsics` does NOT own:

- Composite builders (atomics, lzcnt/tzcnt, clamp, hashes). Those
  moved to `vyre-libs` in Migration 2–3 and live there now.
- Register allocation, backend dispatch, or pipeline caching:
  that's `vyre-driver` + the backend crates.
- IR schema or validation: those live in `vyre-foundation`.

## Three worked examples

### 1. Use a bit intrinsic in a compute kernel

```rust
use vyre_foundation::ir::{Expr, Program};
use vyre_intrinsics::region::wrap;

fn popcount_program(input: &Expr) -> Program {
    let body = wrap(|_ctx| {
        // popcount_u32 is registered via inventory::submit! in the
        // hardware module; the macro-generated builder is what the
        // caller invokes.
        vyre_intrinsics::hardware::popcount_u32(input.clone())
    });
    Program::from_entry(body)
}
```

### 2. Use subgroup ops

```toml
[dependencies]
vyre-intrinsics = "0.4.2"
```

Then in code:

```rust
use vyre_intrinsics::hardware::subgroup_ballot;
```

The default feature set includes `subgroup-ops`; a backend that cannot
lower subgroup collectives rejects the program during validation.

### 3. CPU-reference dispatch for a custom intrinsic test

```rust
use vyre_foundation::cpu_op::{CpuOp, structured_intrinsic_cpu};
use vyre_intrinsics::hardware::fma_f32;

let expected = structured_intrinsic_cpu::<fma_f32::Cpu>(&[2.0, 3.0, 1.0]);
assert_eq!(expected, 7.0);
```

## Extension guide: adding a Category-C intrinsic

1. Verify the op cannot be expressed as a composition of existing
   `Expr` variants. If it can, write a `fn() -> Program` helper in
   `vyre-libs` instead. Category-C is a narrow door; ~9 ops total.
2. Add a submodule under `hardware/` (or another feature-gated
   module). Implement:
   - The builder function returning `Program` / `Expr`.
   - The `CpuOp` / `CategoryAOp` implementation for bit-identical
     reference evaluation.
   - The dedicated emitter arm in the concrete driver that owns the
     target lowering.
3. Register with `inventory::submit!(IntrinsicDescriptor { ... })`
   : the harness discovers you automatically.
4. Declare algebraic laws (`AlgebraicLaw`) if your op has any; the
   conform harness re-derives them at every CI run and rejects
   implementations that break them.
5. Add a conform fixture under `conform/vyre-conform-runner/fixtures`
   so every backend is diffed against your CPU reference from day
   one.
6. Document the op in `docs/migration-vyre-ops-to-intrinsics.md` so
   the classification rule stays authoritative.

See `hardware/popcount.rs` for the minimal template and
`hardware/subgroup_ballot/subgroup_ballot.rs` for the subgroup pattern.
