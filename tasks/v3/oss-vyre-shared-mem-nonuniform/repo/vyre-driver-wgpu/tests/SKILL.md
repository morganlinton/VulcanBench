# tests/SKILL.md  -  vyre-driver-wgpu

Read `../../.internals/skills/testing/SKILL.md` first for the category contract.

## Purpose

`vyre-driver-wgpu` is the **wgpu backend**: GPU runtime (device
acquisition, buffer pool, pipeline cache, shader compilation,
dispatch), IR-to-naga lowering, async readback + streaming engines.
Every GPU execution path in vyre eventually lives here.

## Critical invariants

- **Parity with CPU reference.** Every op that has both a CPU
  reference and a wgpu lowering produces byte-identical output on
  every witnessed input. Divergence = backend bug.
- **`validate_with_cache` is a single-atomic-load fast path.** No
  re-hashing, no DashMap probe after 0.6.
- **Pipeline cache key covers every dimension that changes
  lowering outcome.** `workgroup_size[0..3]`, every binding attr,
  feature flags. Missing dimensions = silent miscache hazard.
- **Capability queries never over-promise.** Returning `true` from
  `supports_subgroup_ops` requires that the lowering actually
  emits subgroup intrinsics AND the adapter supports them.
- **Honest deadline enforcement.** `DispatchConfig.timeout` must
  surface a structured error on overrun, with a `tracing::warn`
  event.

## Adversarial surface

- `Program` with workgroup `[0, 0, 0]`  -  rejected, no panic
- `Program` with 10 000 buffers  -  bounded, structured error if cap
  exceeded
- Concurrent `WgpuBackend::dispatch` from 8 threads  -  no data
  races, no poisoned mutexes, stats still consistent
- Adapter that advertises SUBGROUP but fails to compile a subgroup
  shader  -  capability report must flip to `false` after first
  failure, not stay `true` silently
- Readback with `Maintain::Wait` on a dropped queue  -  structured
  error
- Streaming `push_chunk` racing with `finish()`

## Current gaps

- True LRU on the pipeline cache  -  today deterministic-but-
  arbitrary eviction. Gap test: "after 512 unique pipelines, the 256
  hottest remain".
- `BindGroupCache` reuse in `record_and_readback`  -  currently
  creates fresh bind groups per dispatch. Gap test: "dispatch 100×
  same pipeline + same buffers → bind_group_created_count == 1".
- Device-loss recovery  -  `try_recover` currently returns
  `UnsupportedFeature`. Gap test: "simulated device-lost callback
  invalidates cached pipelines then recovers".
- `supports_bf16` / `supports_tensor_cores` / `supports_async_compute`
  return `false`  -  gap tests document that each requires a
  lowering path before flipping to `true`.

## Cross-crate contracts

- Implements `vyre_driver::VyreBackend`  -  every defaulted method
  exercised by the `Mock/FullBackend` contract test in
  `vyre-driver/tests/backend_contract.rs`
- Implements `vyre_driver::CompiledPipeline`  -  dispatch must be
  bit-identical to `VyreBackend::dispatch` per the contract
- Consumes `vyre_foundation::Program`  -  round-trip through
  `to_wire` / `from_wire` must produce bit-identical GPU output

## Bench targets

- `dispatch_small_program` (single kernel, 64 workgroup_size, tiny
  inputs)  -  latency baseline
- `dispatch_throughput`  -  Bytes/s across 1 KB / 64 KB / 1 MB inputs
- `pipeline_cache_hit` vs `pipeline_cache_miss`  -  per-call cost
- `buffer_pool_acquire`  -  target O(1), sub-100 ns
- `bind_group_cache_hit`  -  once the #8 gap closes, add baseline
- `validate_with_cache_hit`  -  target sub-10 ns (atomic load only)

## Fuzz targets

- `pipeline_disk_cache::compute_cache_key`  -  arbitrary program +
  arbitrary fingerprint → no panic
- `lowering::naga_emit`  -  arbitrary program → emit naga module →
  no panic (naga's own validator does the rest)

## What NOT to test here

- Wire format  -  `vyre-foundation/tests`
- IR semantics / CPU reference  -  `vyre-reference/tests`
- Op metadata  -  `vyre-spec/tests`
- Driver-tier trait contracts  -  `vyre-driver/tests/backend_contract.rs`

## Running

```bash
./cargo_full test -p vyre-driver-wgpu
./cargo_full test -p vyre-driver-wgpu --test adversarial
./cargo_full test -p vyre-driver-wgpu --test property
./cargo_full test -p vyre-driver-wgpu --test gap
./cargo_full test -p vyre-driver-wgpu --test integration
./cargo_full bench -p vyre-driver-wgpu
cd vyre-driver-wgpu/fuzz && ../../cargo_full fuzz run pipeline_cache_key
```
