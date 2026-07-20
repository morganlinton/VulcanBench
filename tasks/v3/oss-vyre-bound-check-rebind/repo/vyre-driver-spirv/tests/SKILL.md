# tests/SKILL.md  -  vyre-driver-spirv

Read `../../.internals/skills/testing/SKILL.md` first for the category contract.

## Purpose

`vyre-driver-spirv` is the **SPIR-V emitter shim**. It consumes a
validated `naga::Module` and emits SPIR-V bytes via `naga::back::spv`.
Keep this crate thin: it validates, writes, returns bytes.

## Critical invariants

- **Byte-identity with naga's reference SPIR-V output** for every
  module the wgpu backend produces. Any divergence is either a
  naga bug (upstream report) or a shim misconfiguration.
- **Rejects invalid modules.** `naga::valid::Validator` runs before
  every emit; invalid modules return an actionable string error.
- **No side effects.** Emitter is pure: `fn(module: &Module) ->
  Result<Vec<u32>, String>`. Same input → same bytes, forever.

## Adversarial surface

- `naga::Module` with validation errors  -  structured string error,
  no panic
- Module with no entry point  -  rejected
- Module with features the current naga SPIR-V writer doesn't
  support  -  rejected with a pointer to the missing feature

## Current gaps

- Target SPIR-V version is hardcoded via `spv::Options::default()`.
  Gap: accept a `SpvVersion` argument so Vulkan 1.2 / 1.3 callers
  can pick.
- No differential test against `glslangValidator` or `spirv-val`
   -  Vulkan SDK validator is the independent oracle.

## Cross-crate contracts

- Consumes validated `naga::Module` values produced by backend-neutral lowering
- Produces SPIR-V bytes consumed by: any Vulkan-compute runner
  (ash, vulkano) outside the santh tree; conform byte-identity
  tests

## Bench targets

- `emit_spv`  -  bytes emitted / sec across small / medium / large
  modules

## Fuzz targets

- `emit_spv_fuzz`  -  arbitrary `naga::Module` constructed via
  proptest → never panic, always return `Result`

## What NOT to test here

- Runtime-driver dispatch behavior
- Naga lowering correctness outside this SPIR-V writer boundary

## Running

```bash
./cargo_full test -p vyre-driver-spirv
./cargo_full test -p vyre-driver-spirv --test adversarial
./cargo_full test -p vyre-driver-spirv --test property
./cargo_full test -p vyre-driver-spirv --test gap
./cargo_full test -p vyre-driver-spirv --test integration
./cargo_full bench -p vyre-driver-spirv
```
