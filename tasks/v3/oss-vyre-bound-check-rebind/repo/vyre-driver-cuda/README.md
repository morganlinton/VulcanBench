# vyre-driver-cuda

CUDA/PTX backend for vyre through `cudarc`.

This crate is the NVIDIA backend implementation point and the `0.4.2` release
fast path. It owns CUDA device probing, hardware capability reporting, PTX
lowering, and dispatch integration behind the frozen `VyreBackend` contract.

## Current Contract

The backend probes the live CUDA device for compute capability, VRAM, launch
limits, warp size, cooperative-launch support, concurrent-kernel support, and
async-engine count. Backend capability methods are derived from those device
attributes. Dispatch routes through PTX lowering first and returns structured
backend errors when a program or launch path is not supported by the current
implementation. A failed CUDA probe is a configuration or capability error; it
is never silently downgraded to CPU execution.

CUDA is release-preferred on NVIDIA systems. WGPU remains the portable GPU
fallback backend for non-CUDA deployments. Release evidence must prove CUDA
conformance, performance, feature-surface coverage, and metadata publication
for `vyre-driver-cuda@0.4.2`.

## Quick start

```rust
use vyre_driver_cuda::CudaBackend;

let backend = CudaBackend::acquire()?;
let outputs = backend.dispatch(&program, &inputs, &config)?;
```

## Architecture decisions

- **`#![allow(unsafe_code)]` is policy, not accident.** CUDA driver
  bindings (`cudarc::driver::sys::cu*`) are inherently unsafe FFI. The
  rest of the workspace keeps `unsafe_code = "deny"`; this crate
  documents its policy in `src/lib.rs` and gates every unsafe block
  through `scripts/check_unsafe_justifications.sh`.
- **PTX-only emit path.** The CUDA backend never emits SPIR-V or WGSL.
  Cross-substrate parity is verified by the conformance runner against
  the wgpu and reference backends.
- **No peer-backend deps.** The crate must not pull
  `vyre-driver-wgpu` or `vyre-driver-spirv`; the boundary is enforced
  by `OWNERSHIP.md` and `scripts/check_ownership_boundaries.sh`.

## Where to look

- `src/backend.rs`  -  `VyreBackend` impl + capability probing.
- `src/codegen.rs`  -  PTX emit pipeline.
- `docs/CUDA_BACKEND_EXECUTION_PLAN.md`  -  historical CUDA backend context.
- `docs/optimization/OWNERSHIP.toml` and `docs/optimization/AGENT_CONTRACT.md`
   -  active CUDA optimization ownership and patch proof contract.
- `OWNERSHIP.md` (workspace root)  -  boundary definition.
