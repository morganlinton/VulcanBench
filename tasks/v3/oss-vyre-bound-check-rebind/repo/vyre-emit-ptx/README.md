# vyre-emit-ptx

PTX text emitter for vyre's `KernelDescriptor`.

Consumes a substrate-neutral `vyre_lower::KernelDescriptor` and
produces NVIDIA PTX text suitable for `nvrtcCompileProgram` or
`cuLinkAddData`. Independent code path from
`vyre-emit-naga`/`vyre-emit-spirv`: PTX is a different IR family
(register machine vs SSA-typed) so the lowering doesn't share
machinery with the naga-based emitters.

## Quick start

```rust
use vyre_lower::{
    BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody,
    KernelDescriptor, KernelOp, KernelOpKind, LiteralValue, MemoryClass,
};
use vyre_foundation::ir::DataType;
use vyre_emit_ptx::{emit_optimized, ComputeCapability};

let desc = KernelDescriptor {
    id: "store_seven".into(),
    bindings: BindingLayout {
        slots: vec![BindingSlot {
            slot: 0,
            element_type: DataType::U32,
            element_count: Some(1),
            memory_class: MemoryClass::Global,
            visibility: BindingVisibility::WriteOnly,
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

let ptx = emit_optimized(&desc).unwrap();
assert!(ptx.contains(".visible .entry"));
```

## API

- `emit(&desc) -> Result<String, EmitError>`: lower as given,
  default `ComputeCapability::SM_70`.
- `emit_with_target(&desc, capability) -> Result<String, EmitError>`
  : same as `emit` but pick the target SM (sm_60/70/75/80/86/89/90).
- `emit_optimized(&desc)` / `emit_optimized_with_target(&desc, cap)`
  : recommended. Run `vyre_lower::rewrites::run_all` first; debug
  builds also run `verify` via `debug_assert!`.
- `emit_optimized_with_stats(&desc)` /
  `emit_optimized_with_target_with_stats(&desc, cap)`: same as the
  optimized variants but also return `OptimizationStats` (op count
  delta, bindings dropped, fixed-point iterations).

## Substrate-specific patterns

`patterns/` contains PTX-only optimizations:

- `tensor_core_fragment`: wmma/mma intrinsics on Volta+.
- `ldmatrix_cp_async`: Ampere+ async global → shared copies.
- `predicated_execution`: fold short branches into predicated ops.
- `instruction_scheduling`: issue-slot-aware instruction order.
- `vec_load_fusion`: adjacent `LoadGlobal`+1 chains → `ld.global.v2/v4`.
- `vec_store_fusion`: adjacent `StoreGlobal`+1 chains → `st.global.v2/v4`.

Run `patterns::audit(&desc, target)` for a unified `PtxAuditReport`
covering all 6 patterns. Run `patterns::audit_optimized(&desc, target)`
to audit the post-`run_all` form: answers "what PTX-specific
optimizations remain after the standard pipeline?".

These complement the substrate-neutral analyses + rewrites in
`vyre-lower`.

## Compute capabilities

`ComputeCapability::SM_60` … `SM_90`. Lower bounds: SM_70 enables
warp-vote intrinsics; SM_75 enables independent thread scheduling
features; SM_80+ enables async copies and tensor cores.

## Errors

- `EmitError::UnsupportedOp(op)`  -  descriptor uses a `KernelOpKind`
  the PTX emitter hasn't lowered yet.
- `EmitError::InvalidBinding { slot, reason }`  -  binding can't be
  represented as a PTX `.param` or `.global` of the given type.
- `EmitError::InvalidDescriptor(s)`  -  descriptor malformed.

## See also

- `vyre-lower`: IR + rewrite stack + verify.
- `vyre-emit-naga` / `vyre-emit-spirv`: wgpu/Vulkan-targeting siblings.

## License

MIT OR Apache-2.0.
