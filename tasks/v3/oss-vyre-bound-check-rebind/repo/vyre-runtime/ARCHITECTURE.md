# vyre-runtime  -  architecture

The runtime layer that wires per-backend dispatchers into a
single substrate-neutral surface. Owns the megakernel run loop,
pipeline cache, replay infrastructure, and async dispatch.

## Modules

### `megakernel/`
The persistent-kernel scheduler. Sub-modules:

- `wgpu_dispatch.rs` (OFF-LIMITS  -  sheaf wire just shipped)  - 
  wgpu-specific dispatcher.
- `scaling.rs`  -  heterophilic-cluster detection + sheaf
  diffusion + autotuner.
- `protocol.rs` / `protocol_api.rs`  -  the ring-buffer wire
  format every megakernel tenant follows.
- `io.rs`  -  I/O queue protocol for in-kernel async I/O.
- `handlers.rs`  -  opcode-dispatch handler emitter.
- `scheduler.rs`  -  work-item priority partitioning,
  fairness budgets.
- `builder.rs`  -  Program-builder helpers (persistent_body_jit,
  build_program_jit).

### `pipeline_cache.rs`
Pipeline-cache layer above each backend's per-cache. LRU +
disk-persistent.

### `routing/`
Backend selection  -  given a Program + adapter caps, pick the
backend that supports every op the program uses.

### `scheduler.rs`
Top-level dispatch scheduler. Coordinates between routing and
the megakernel runner.

### `tenant.rs`
Tenant abstraction  -  multiple clients can share a megakernel by
publishing into different opcode partitions.

### `uring/`
io_uring + zero-copy SSD ingest. Used by the cve-corpus loader
and the demo runners.

### `replay.rs`
Captures + replays a dispatch trace for offline debugging.

## Public types

- **`WgpuMegakernelDispatcher`**  -  runtime wrapper around the
  wgpu backend's persistent-kernel dispatch.
- **`Megakernel`**  -  protocol API entry point (encode_control,
  encode_empty_ring, publish_slot, read_done_count).
- **`MegakernelIoQueue`**  -  the in-kernel async I/O queue.

## Integration points

- Downstream fused-dispatch paths route through this layer when they
  opt into megakernel residency.
- `vyre-aot` calls into this for the runtime side of the AOT
  artifact loader.
