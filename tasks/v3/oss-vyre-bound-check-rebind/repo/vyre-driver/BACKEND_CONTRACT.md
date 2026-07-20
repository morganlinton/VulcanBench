# vyre-driver backend contract (v0.4.1 release)

This document is the long-form specification of the `VyreBackend` trait
defined in `vyre-driver/src/backend.rs`. It exists because after vyre 0.4.1
the trait surface is *frozen*  -  the only post-0.6 changes permitted are
additive defaulted methods. Every backend authored in later versions implements this trait by
inheriting all defaults and overriding only the methods where the
backend is more capable than the conservative floor.

The contract is deliberately *seal-at-both-ends*: the trait has a
private `__vyre_backend_sealed` method that forces every external impl
to route through an opt-in feature gate, and every additive method has
a conservative default so adding one is non-breaking for internal
impls. Adding a new *required* method, or changing the signature of an
existing method, is a major-version break and is not in scope before
2.0.

## Why this trait is frozen

A backend is an interchange surface. Upgrading the driver crate must
not ripple into a downstream backend crate. vyre's thesis is that a
frontend emits IR without knowing the backend and a backend executes
IR without knowing the frontend; anything that couples the two is
architectural rot.

The failure mode we are preventing: vyre 0.7 adds
`fn supports_cooperative_matrix(&self) -> bool` as a *required*
method. Every downstream backend breaks. The user files an issue.
We tell them to update their Cargo.toml. They walk away.

The solution is the one every seasoned library author uses: defaulted
methods with conservative bodies. `cargo update` on the backend crate
is a no-op for method additions forever.

## Dispatch core

| Method | Default | Override when |
| --- | --- | --- |
| `id(&self) -> &'static str` | required | always |
| `version(&self) -> &'static str` | `"unspecified"` | ship a real crate version |
| `supported_ops(&self) -> &HashSet<OpId>` | `default_supported_ops()` | backend does not execute every core op |
| `dispatch(...)` | required | always |
| `dispatch_borrowed(...)` | `dispatch` with one owned-vec allocation | backend binds borrowed bytes directly |
| `compile_native(...)` | `Ok(None)` (framework wraps in a passthrough pipeline) | backend caches compiled state across dispatches |

Every override is strictly more honest / more performant; never
silently degrades correctness.

## Capability queries

Every query defaults to the conservative ("no" or minimal-limit)
value. A backend must report **honestly**: returning `true` from a
capability query is a *promise* that the lowering path emits the
corresponding intrinsic and the current adapter supports it. "Feature
bit is set but lowering emits scalar fallback" = capability is
`false`.

| Query | Default | Concrete backend behavior |
| --- | --- | --- |
| `supports_subgroup_ops` | `false` | probed from the backend-owned device capability surface |
| `supports_f16` | `false` | probed from the backend-owned device capability surface |
| `supports_bf16` | `false` | probed from the backend-owned device capability surface |
| `supports_tensor_cores` | `false` | probed or kept false when the backend has no native matrix path |
| `supports_async_compute` | `false` | true only when the backend owns a real asynchronous queue |
| `supports_indirect_dispatch` | `false` | true only when the backend can execute indirect launch records |
| `is_distributed` | `false` | true only for multi-node backends |
| `max_workgroup_size` | `[1,1,1]` | probed or conservatively bounded by the backend |
| `max_storage_buffer_bytes` | `0` | probed or conservatively bounded by the backend |

"probed" = queried at device construction from the underlying adapter
or runtime, cached, returned verbatim.

## Lifecycle hooks

Every hook has a safe default that a backend without the concept can
use as-is.

| Hook | Default | Concrete backend behavior |
| --- | --- | --- |
| `prepare(&self)` | `Ok(())` | warm backend-owned runtime state |
| `flush(&self)` | `Ok(())` | drain backend-owned queues or streams |
| `shutdown(&self)` | `Ok(())` | release backend-owned runtime state |
| `device_lost(&self)` | `false` | report backend-owned liveness state |
| `try_recover(&self)` | `UnsupportedFeature` | rebuild backend-owned runtime state and invalidate compiled handles |

Recovery is opt-in by default because silently re-acquiring a device
without invalidating the caller's `CompiledPipeline` handles is a
correctness hazard. A backend that implements recovery must also
invalidate every `CompiledPipeline` it handed out before the loss.

**Operational caveat: `try_recover` latency is bounded by the slowest
in-flight dispatch.** A backend that guards device state with shared
locks must ensure recovery cannot starve behind active dispatches.
Production deployments that need bounded recovery must cap dispatch
duration via `DispatchConfig::timeout` and force-cancel on expiry so
writer starvation is impossible.

## Seal

```rust
#[doc(hidden)]
fn __vyre_backend_sealed(&self) {}
```

This private defaulted method is the forward-compat lever. If vyre 0.8
truly needs a required method, we can add it alongside a sealed
breakage of this marker  -  forcing external impls to explicitly migrate
via a feature gate rather than silently miscompiling. Internal impls
internal implementations rely on the default body and never notice.

## Authoring a new backend

Given a new concrete backend:

```rust
use vyre_driver::{BackendError, CompiledPipeline, DispatchConfig, VyreBackend};
use vyre_foundation::ir::Program;

pub struct MyBackend {
    state: BackendState,
    caps: MyCaps,
}

impl VyreBackend for MyBackend {
    fn id(&self) -> &'static str { "my-backend" }
    fn version(&self) -> &'static str { env!("CARGO_PKG_VERSION") }

    fn dispatch(
        &self,
        program: &Program,
        inputs: &[Vec<u8>],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        cuda::launch(&self.context, program, inputs, config).map_err(|error| {
            BackendError::new(format!("Fix: cuda dispatch failed: {error}"))
        })
    }

    // Honest capability reports.
    fn supports_subgroup_ops(&self) -> bool { self.caps.warp_ops }
    fn supports_f16(&self) -> bool { self.caps.f16 }
    fn supports_tensor_cores(&self) -> bool { self.caps.tensor_cores }
    fn supports_async_compute(&self) -> bool { true }
    fn supports_indirect_dispatch(&self) -> bool { true }
    fn max_workgroup_size(&self) -> [u32; 3] { self.caps.max_workgroup }
    fn max_storage_buffer_bytes(&self) -> u64 { self.caps.max_buffer }

    // Lifecycle.
    fn prepare(&self) -> Result<(), BackendError> { cuda::warm(&self.context).map_err(map_cuda_error)?; Ok(()) }
    fn flush(&self) -> Result<(), BackendError> { cuda::sync(&self.context).map_err(map_cuda_error)?; Ok(()) }
    fn shutdown(&self) -> Result<(), BackendError> { cuda::destroy(&self.context).map_err(map_cuda_error)?; Ok(()) }
    fn device_lost(&self) -> bool { cuda::check_device(&self.context).is_err() }
}
```

No changes to `vyre-driver` or any other backend crate. The trait
carries the full surface forward.

## Stability

Every method above is covered by a per-backend integration test in
`vyre-driver-<backend>/tests/` that verifies the capability report
matches observable runtime behavior. "Claims to support subgroup ops
but the scalar path is used" must fail that integration test.

The `MockBackend` in `vyre-driver/tests/mock_backend.rs` exercises
every default (all conservative values). The `FullBackend` exercises
every non-default override path. Together they guarantee the trait
remains defaultable and overridable for the lifetime of 0.6.
