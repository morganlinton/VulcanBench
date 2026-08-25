# vyre-intrinsics  -  architecture

Tier-2 hardware-intrinsic ops. Every op here requires both a
dedicated naga emitter arm AND a dedicated `vyre-reference`
eval arm  -  composition cannot lower it.

## Modules

### `hardware/`
The 9 Cat-C ops: `bit_reverse_u32`, `popcount_u32`, `fma_f32`,
`inverse_sqrt_f32`, `workgroup_barrier`, `storage_barrier`, plus
subgroup ops backed by Naga 25+ subgroup lowering:
`subgroup_ballot`, `subgroup_shuffle`, `subgroup_add`.

### `harness.rs`
Hardware-conform differential harness. Cross-checks every
intrinsic's CPU reference against its naga-emitted GPU output.

### `region.rs`
Region-wrap helpers shared with `vyre-harness::region`.

### `category_check.rs`
Compile-time gate that asserts every op in this crate is
genuinely Cat-C (requires hardware emission). A composition-
expressible op landing here trips the build.

## Public types

- **(per-op)** Each hardware op exports a constructor `op_id() →
  Program` and a `cpu_ref(args) → output` for the conform suite.
- **`HardwareConformHarness`**  -  differential runner.

## Integration points

- Plugged into `vyre-driver` via inventory registration so any
  backend that supports the intrinsic picks it up.
- Consumed by `vyre-libs` Tier-3 compositions that need a
  hardware accelerator under their composition.
