//! Layer 0 GPU runtime: device, buffers, shader compilation, and dispatch.

/// Adapter-caps probe (C-B10).
///
/// Projects a `wgpu::Adapter` into the substrate-neutral
/// [`vyre_foundation::optimizer::AdapterCaps`] passes read to adapt.
pub mod adapter_caps_probe;
/// AOT shader specialization cache.
pub mod aot;
/// Tiered caching for device buffers and shader pipelines.
pub mod cache;
/// GPU device abstraction and initialization.
pub mod device;
/// Indirect dispatch path (C-B4).
///
/// Submits `ComputePass::dispatch_workgroups_indirect` on a
/// GPU-resident `[u32; 3]` workgroup-count buffer.
pub mod indirect;
/// Pre-recorded persistent dispatch command buffers.
pub mod prerecorded;
/// Async readback ring (C-B5).
///
/// N-deep staging ring; dispatch i writes to `ring[i % N]`; copies
/// submit immediately, readbacks map asynchronously. Overlaps
/// dispatch i+1 with readback i's copy.
pub mod readback_ring;
/// Runtime backend auto-picker (C-B11).
///
/// Walks `inventory::iter::<BackendRegistration>`, filters by
/// Program dialect support, picks by precedence. `VYRE_BACKEND=<id>`
/// forces a specific backend.
pub mod router;
/// Runtime wire-format serialization for multi-part programs.
pub mod serializer;
/// Shader pipeline compilation and caching.
pub mod shader;
/// LRU cache access tracker for buffer eviction policies.
pub use cache::lru::AccessTracker;
/// Cache tier policies and access statistics.
pub use cache::{AccessStats, CacheError, LruPolicy};
/// Initialize a cached GPU device wrapper.
pub use device::{cached_adapter_info, cached_device, init_device};
/// Compile a compute pipeline from WGSL source.
pub use shader::compile_compute_pipeline::{
    compile_compute_pipeline, compile_compute_pipeline_with_layout,
};

/// Build a bind group entry binding `buffer` at `binding` index.
#[must_use]
#[inline]
pub fn bg_entry(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
}
