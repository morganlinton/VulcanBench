# vyre-driver-cuda  -  architecture

CUDA backend. Implements `VyreBackend` against the NVIDIA CUDA
runtime + driver APIs.

## Modules

### `backend.rs` (OFF-LIMITS  -  submodular eviction just shipped)
Backend-trait implementation. Owns the device handle, the stream
pool, and the dispatch hot path. Currently mid-edit; do not
co-edit with this turn's work.

### `binding.rs`
Buffer-binding pass that maps `BufferDecl` records onto CUDA
stream-attached buffer slots.

### `codegen.rs`
PTX/CUBIN emission. Lowers the Program's typed IR into PTX via
the CUDA driver's NVRTC API; caches the resulting `cubin` blob
keyed on the conformance certificate.

### `device.rs`
Device discovery, capability probing (compute capability, max
shared mem per block, register file size).

### `pipeline.rs`
Kernel-launch parameter computation: workgroup count, dynamic
shared memory request, argument packing.

### `stream.rs`
Per-stream handle pool; lets the dispatcher overlap H2D copies
with kernel execution.

## Public types

- **`CudaBackend`**  -  backend-trait implementation. Acquired via
  `CudaBackend::acquire()` which probes for a CUDA-capable
  device.
- **`StreamPool`**  -  internal; not exposed across the trait
  boundary.

## Integration points

- Plugs into `vyre-driver`'s registration via inventory.
- Cooperates with `vyre-runtime megakernel` when the program is a
  megakernel (PTX is emitted with the persistent-loop body the
  scaling layer asked for).
