# vyre-driver  -  architecture

Substrate-neutral backend orchestration. Owns the `VyreBackend`
trait, the dispatch contract, the routing table that picks a
backend per-program, and the cross-backend diagnostics layer.

Concrete backend implementations live in sibling driver crates.
This crate is the substrate they plug into; shared runtime capabilities
such as the megakernel ABI stay backend-neutral and are implemented by
each concrete driver locally.

Concrete-driver APIs, type names, and target vocabulary stay inside the
concrete driver crate that owns the target. `vyre-driver` exposes only
backend-neutral contracts and capability records. When concrete backends
need the same decision logic, the logic lives here; the
concrete crate keeps only adapter probing, target code emission, target
validation calls, and timing/dispatch mechanics.

## Modules

### `backend/` + `backend.rs`
The `VyreBackend` trait  -  the frozen contract every backend
must implement. Sealed via `backend::private::Sealed` so external
crates can't accidentally implement it without going through the
explicit per-backend crate.

`DispatchConfig` is the per-call options bag (workgroup_override,
fixpoint_iterations, grid_override, ulp_budget, max_output_bytes,
profile, label, timeout). `#[non_exhaustive]`; consumers
construct via `default()` then field-set.

### `pipeline.rs`
Cross-backend pipeline plumbing. Pre-compiled pipeline cache,
buffer-binding metadata builder, per-output layout calculation.

### `diagnostics.rs`
Backend-emitted diagnostic shaping. Consumed by downstream diagnostic
adapters and the conform runner.

### `observability.rs`
Counters, histograms, and tracing-level profile hooks.

### `persistent.rs`
Persistent-residency hot path. Lets a backend hold buffer handles
across dispatches and skip the bind-group rebuild on warm cache.

### `program_walks.rs`
Program-graph traversals shared across backends (output-buffer
indices, element-size resolution, output-layout shape, dispatch
element-count parameter words).

### `binding.rs`
Backend-neutral binding-plan construction: input/output/shared/persistent
roles, static byte lengths, element counts, and deterministic binding order.

### `fusion.rs`
Cross-dispatch fusion decision types and legality checks. Concrete backends
own only target-module stitching after `FusionDecision::Accept`.

### `specialization.rs`
Backend-neutral specialization values, ordered maps, and cache key inputs.
Concrete drivers own target-specific override lowering and pipeline hashing.

### `subgroup.rs`
Canonical subgroup operation taxonomy plus shared capability records.
Concrete drivers map these operations to their own target-native intrinsics.

### `tuner.rs`
Autotuner candidate generation, typed cache keys, best-of-N measurement
framework, feedback policy, and cache metadata. Concrete drivers implement
the backend timing trait with target timestamp mechanisms.

### `validation.rs`
Shared successful-validation cache and launch-geometry validation. Concrete
drivers provide live limits and run target compiler/adapter checks.

### `lib.rs` (OFF-LIMITS  -  substrate hot-path wires in flight)
Top-level wiring of the routing table + extern dialect bridge.

## Public types

- **`VyreBackend`**  -  frozen backend contract.
- **`BackendError`**  -  uniform error type all backends emit.
- **`DispatchConfig`**  -  per-call options.
- **`OutputBindingLayout`**  -  per-output buffer layout used by
  the pipeline cache + readback path.
- **`BindingPlan` / `BindingRole`**  -  backend-neutral buffer ABI plan.
- **`FusionDecision` / `FusionCaps`**  -  cross-dispatch fusion legality.
- **`SpecMap` / `SpecValue`**  -  neutral specialization inputs.
- **`SubgroupOp` / `SubgroupCaps`**  -  shared subgroup taxonomy.
- **`Pipeline`**  -  pre-compiled per-Program handle.

## Integration points

- Consumed by downstream scan paths.
- Consumed by `vyre-aot` for per-target lowering.
- Extension point for community backends via the sealed trait
  + the extern-registry bridge in `vyre-foundation`.
