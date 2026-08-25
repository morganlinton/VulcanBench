# vyre-emit-spirv

SPIR-V binary emitter for vyre's `KernelDescriptor`.

Routes through `vyre-emit-naga` to produce a `naga::Module` first,
then uses `naga::back::spv::Writer` to output a SPIR-V binary. This
shares the lossless lowering work with the wgpu/naga path: both
backends consume the same `naga::Module`, and avoids forking a
second `KernelOp` → SPIR-V translation table.

## Quick start

```rust
use vyre_lower::{
    BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody,
    KernelDescriptor, KernelOp, KernelOpKind, LiteralValue, MemoryClass,
};
use vyre_foundation::ir::DataType;
use vyre_emit_spirv::{emit_optimized, SPIRV_MAGIC};

let desc = KernelDescriptor {
    id: "store_seven".into(),
    bindings: BindingLayout {
        slots: vec![BindingSlot {
            slot: 0,
            element_type: DataType::U32,
            element_count: None,
            memory_class: MemoryClass::Global,
            visibility: BindingVisibility::ReadWrite,
            name: "out".into(),
        }],
    },
    dispatch: Dispatch::new(64, 1, 1),
    body: KernelBody {
        ops: vec![
            KernelOp { kind: KernelOpKind::Literal, operands: vec![0], result: Some(0) },
            KernelOp { kind: KernelOpKind::Literal, operands: vec![1], result: Some(1) },
            KernelOp {
                kind: KernelOpKind::StoreGlobal,
                operands: vec![0, 0, 1],
                result: None,
            },
        ],
        child_bodies: vec![],
        literals: vec![LiteralValue::U32(0), LiteralValue::U32(7)],
    },
};

let words = emit_optimized(&desc).unwrap();
assert_eq!(words[0], SPIRV_MAGIC);
```

## API

- `emit(&desc) -> Result<Vec<u32>, EmitError>`: lower as given.
  Output is SPIR-V words (the canonical representation per the spec).
- `emit_optimized(&desc) -> Result<Vec<u32>, EmitError>`:
  recommended. Runs `vyre_lower::rewrites::run_all` first; debug
  builds also run `verify` via `debug_assert!`.
- `emit_optimized_with_stats(&desc) -> Result<(Vec<u32>, OptimizationStats), EmitError>`:
  same as `emit_optimized` plus optimization metrics.
- `emit_bytes(&desc) -> Result<Vec<u8>, EmitError>`: convenience
  for runtime loaders that want little-endian bytes.
- `emit_optimized_bytes(&desc)` /
  `emit_optimized_bytes_with_stats(&desc)`: bytes-axis variants
  that also run the optimization pipeline first.
- `emit_from_naga_module(&module) -> Result<Vec<u32>, EmitError>`:
  lower-level entry point if you want to apply naga-level rewrites
  between `vyre_emit_naga::emit` and SPIR-V conversion.

## Constants

- `SPIRV_MAGIC: u32 = 0x07230203`: the SPIR-V magic word per the
  spec. Useful for sanity checks on emit output.

## Substrate-specific patterns

`patterns/` contains SPIR-V-specific analyses:

- `subgroup_capabilities`: walks the descriptor for subgroup ops
  (Ballot, Shuffle, Add, LocalId, Size) and produces a
  `SubgroupCapabilities { basic, ballot, shuffle, arithmetic }` flag
  set so the host knows which `VkSubgroupFeatureFlagBits` to require
  on the pipeline.
- `workgroup_size_validation`: checks `dispatch.workgroup_size`
  against `VULKAN_BASELINE` (or a custom `DeviceLimits` profile).
  Catches per-dim overflow, product overflow, zero-dim violations.

Run `patterns::audit(&desc)` for a unified `SpirvAuditReport` with
both patterns. Run `patterns::audit_optimized(&desc)` to audit the
post-`run_all` form (subgroup capability detection may report fewer
required caps after dead-code elimination).

## Validation

`naga::valid::Validator` runs before `naga::back::spv::Writer`, so
any module that gets to `Writer` is structurally valid per Naga's
spec compliance. The emitted SPIR-V binary therefore satisfies
SPIR-V's structural requirements as far as Naga's coverage goes;
optional external `spirv-val` validation can be wired into your CI.

## Errors

- `EmitError::NagaEmit(naga_err)`: the upstream `vyre_emit_naga`
  call failed. Most often `UnsupportedOp(...)` for KernelOpKinds
  Naga can't represent.
- `EmitError::NagaValidation(s)`: Naga's validator rejected the
  module. Indicates a bug in this crate or in vyre-emit-naga.
- `EmitError::WriterConstruction(s)` / `WriterWrite(s)`: Naga's
  SPIR-V writer failed.

## See also

- `vyre-lower`: IR + rewrite stack + verify.
- `vyre-emit-naga`: the upstream emit path this crate routes through.
- `vyre-emit-ptx`: independent PTX emitter for CUDA.

## License

MIT OR Apache-2.0.
