# vyre-driver-wgpu

wgpu backend for vyre IR: implements `vyre::VyreBackend` on any wgpu-capable GPU (Vulkan, DX12, Metal, WebGPU).

This crate is the `0.4.2` portable GPU fallback backend. It is a GPU fallback
for systems where CUDA is not the target path, not a CPU fallback.

```
cargo add vyre vyre-driver-wgpu
```

## Example

```rust
use vyre::ir::Program;
use vyre::{DispatchConfig, VyreBackend};
use vyre_wgpu::WgpuBackend;

let backend = WgpuBackend::new()?;
let program: Program = my_program();
let inputs: Vec<Vec<u8>> = vec![b"input data".to_vec()];
let config = DispatchConfig::default();

let outputs: Vec<Vec<u8>> = backend.dispatch(&program, &inputs, &config)?;
```

## Features

- Pipeline cache: reuses compiled WGSL pipelines across dispatches by content hash.
- Buffer pool: reuses GPU buffer allocations across dispatches.
- Validation cache: skips repeated capability checks for already-validated programs.
- Lowering happens internally. Consumers never pass WGSL strings; the crate lowers `Program` through `lowering::lower_with_features`.
- Release evidence must prove feature-surface coverage, backend metadata, and conformance for `vyre-driver-wgpu@0.4.2`.

## Requirements

- A wgpu-capable GPU. This crate does NOT silently fall back to CPU. Absence of a GPU is surfaced as an actionable error, not a degradation.
- `wgpu = 25.x`. Pinned: major wgpu version bumps are a `vyre-driver-wgpu` major bump.

## MSRV

Rust 1.85.

## License

MIT OR Apache-2.0.
