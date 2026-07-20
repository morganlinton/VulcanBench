# vyre-driver-wgpu  -  architecture

WebGPU/wgpu backend. The reference VyreBackend implementation;
the conform suite measures every other backend's behaviour
against this one's CPU-reference oracle.

## Modules

### `lib.rs` (OFF-LIMITS  -  validation cache + invalidate_impacted methods active)
Top-level wiring of the backend trait, the public `WgpuBackend`
type, and the registration token.

### `runtime/` + `runtime.rs`
Adapter discovery, device creation, queue management. Caches the
adapter info so the conformance certificate stays stable across
runs.

### `engine/` + `engine.rs`
The dispatch hot path: command-encoder allocation, bind-group
layout caching, queue submission, fence-wait, readback.

### `lowering/`
vyre IR → naga (WGSL AST) lowering. Per-Node and per-Expr arms.
The Node::Region wrapping invariant is enforced here.

### `pipeline.rs` + `pipeline_*.rs`
Pipeline cache (compiled compute pipelines keyed on
program-fingerprint). Variants:
- `pipeline_binding.rs`  -  per-binding metadata.
- `pipeline_bindings.rs`  -  bind-group layout.
- `pipeline_compound.rs`  -  multi-output pipelines.
- `pipeline_disk_cache.rs`  -  on-disk pipeline persistence.
- `pipeline_persistent.rs`  -  persistent-residency hot path.

### `buffer/`
Buffer pool, residency tracker, GpuBufferHandle lifecycle.

### `megakernel.rs`
Megakernel-specific dispatch helpers (the runtime wrapper lives
in `vyre-runtime::megakernel`).

### `async_dispatch.rs`
Async submission path that overlaps submit with the next
preparation pass.

### `capabilities.rs`
Adapter-cap probe  -  returns the adapter's max workgroup size,
storage-buffer count, subgroup support, etc.

### `config.rs`
Backend config knobs (backend selection, validation level,
disk-cache path).

### `ext.rs`
Extension hooks for vendor-specific intrinsics.

### `spirv_backend.rs`
SPIR-V emission shortcut for the wgpu backend's Vulkan path.

### `bin/`
Standalone binaries (debug helpers, conform probes).

## Public types

- **`WgpuBackend`**  -  backend-trait implementation. Acquired via
  `WgpuBackend::acquire()` or `::new()`.
- **`PipelineCache` / `LruPipelineCache`**  -  pipeline cache
  surface.
- **`GpuBufferHandle`**  -  persistent-buffer handle.
- **`OutputBindingLayout`**  -  re-exported from vyre-driver for
  call-site convenience.

## Integration points

- Default portable GPU backend.
- The conform runner uses this backend's CPU reference as the
  oracle.
- Downstream fused-dispatch paths target this backend's standard
  binding layout.
