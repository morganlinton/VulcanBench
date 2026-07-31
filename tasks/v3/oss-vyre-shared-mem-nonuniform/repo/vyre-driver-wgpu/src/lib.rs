#![allow(
    unstable_name_collisions,
    clippy::field_reassign_with_default,
    clippy::double_must_use,
    clippy::type_complexity,
    clippy::missing_errors_doc,
    clippy::too_many_arguments,
    clippy::manual_clamp,
    clippy::module_inception,
    clippy::empty_line_after_doc_comments,
    clippy::let_and_return,
    clippy::missing_safety_doc
)]
#![deny(unsafe_code)]
#![deny(missing_docs)]

//! # vyre-wgpu  -  wgpu backend for the vyre GPU compute specification

mod allocation;
mod async_dispatch;
mod backend_impl;
pub mod buffer;
mod capabilities;
mod descriptor_mapping;
mod device_buffer;
pub mod emit;
pub mod engine;
mod executable_api;
pub mod ext;
pub mod megakernel;
mod numeric;
mod padded_upload;
#[cfg(feature = "parity-testing")]
mod parity_probe;
pub mod pipeline;
mod resident_dispatch;
mod resident_download;
mod resident_resource;
mod resident_upload;
pub mod runtime;
pub mod spirv_backend;
mod staging_reserve;
mod stats;
mod thread_pool;
mod wait_backoff;

pub use device_buffer::{WgpuDeviceBuffer, WGPU_BACKEND_ID};
pub use executable_api::WgpuIR;
pub use stats::WgpuBackendStats;
use std::hash::BuildHasherDefault;
use std::sync::{atomic::AtomicBool, Arc};
use vyre_driver::shape_prediction::{ShapeFingerprint, ShapeHistory};
use vyre_driver::DispatchConfig;
use vyre_foundation::ir::DataType;
use vyre_foundation::ir::Program;
use vyre_foundation::validate::BackendValidationCapabilities;

#[derive(Clone, Debug)]
pub(crate) enum AdapterRecoveryTarget {
    Index(usize),
    Identity(crate::runtime::device::AdapterIdentity),
}

/// A real wgpu backend for vyre.
#[derive(Clone, Debug)]
pub struct WgpuBackend {
    pub(crate) adapter_info: wgpu::AdapterInfo,
    pub(crate) adapter_name: Arc<str>,
    pub(crate) device_limits: wgpu::Limits,
    pub(crate) device_queue: Arc<arc_swap::ArcSwap<(wgpu::Device, wgpu::Queue)>>,
    pub(crate) dispatch_arena: Arc<arc_swap::ArcSwap<DispatchArena>>,
    pub(crate) persistent_pool: Arc<arc_swap::ArcSwap<crate::buffer::BufferPool>>,
    pub(crate) pipeline_cache: Arc<runtime::cache::pipeline::LruPipelineCache>,
    pub(crate) wgsl_dispatch_pipeline_cache: Arc<
        dashmap::DashMap<
            [u8; 32],
            Arc<wgpu::ComputePipeline>,
            BuildHasherDefault<rustc_hash::FxHasher>,
        >,
    >,
    pub(crate) resident_pipeline_cache: Arc<
        dashmap::DashMap<
            (u64, u64, usize),
            Arc<crate::pipeline::WgpuPipeline>,
            BuildHasherDefault<rustc_hash::FxHasher>,
        >,
    >,
    pub(crate) validation_cache: Arc<vyre_driver::validation::ValidationCache>,
    pub(crate) shape_history: Arc<std::sync::Mutex<ShapeHistory>>,
    pub(crate) predicted_programs: Arc<
        dashmap::DashMap<
            ShapeFingerprint,
            PredictedProgram,
            BuildHasherDefault<rustc_hash::FxHasher>,
        >,
    >,
    pub(crate) bind_group_layout_cache: Arc<
        dashmap::DashMap<
            vyre_driver::BackendLayoutFingerprint,
            Arc<[Arc<wgpu::BindGroupLayout>]>,
            BuildHasherDefault<rustc_hash::FxHasher>,
        >,
    >,
    pub(crate) resident_handles: Arc<
        dashmap::DashMap<
            u64,
            crate::buffer::GpuBufferHandle,
            BuildHasherDefault<rustc_hash::FxHasher>,
        >,
    >,
    pub(crate) device_lost: Arc<AtomicBool>,
    pub(crate) enabled_features: crate::runtime::device::EnabledFeatures,
    pub(crate) recovery_target: AdapterRecoveryTarget,
}

#[derive(Clone, Debug)]
pub(crate) struct PredictedProgram {
    pub(crate) program: Arc<Program>,
    pub(crate) config: DispatchConfig,
}

/// Backend-owned dispatch buffer arena.
#[derive(Clone)]
pub struct DispatchArena {
    pool: crate::buffer::BufferPool,
    readback_rings: Arc<runtime::readback_ring::ReadbackRingSet>,
}

impl std::fmt::Debug for DispatchArena {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("DispatchArena { pool: size-classed }")
    }
}

impl DispatchArena {
    /// Create a dispatch arena backed by the canonical persistent buffer pool.
    #[must_use]
    #[inline]
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, config: &DispatchConfig) -> Self {
        Self {
            pool: crate::buffer::BufferPool::new(device, queue, config),
            readback_rings: Arc::new(runtime::readback_ring::ReadbackRingSet::new()),
        }
    }

    pub(crate) fn pool(&self) -> &crate::buffer::BufferPool {
        &self.pool
    }

    pub(crate) fn readback_rings(&self) -> &Arc<runtime::readback_ring::ReadbackRingSet> {
        &self.readback_rings
    }
}

impl BackendValidationCapabilities for WgpuBackend {
    fn backend_name(&self) -> &'static str {
        "wgpu"
    }

    fn supports_cast_target(&self, target: &DataType) -> bool {
        matches!(
            target,
            DataType::Bool
                | DataType::U8
                | DataType::U16
                | DataType::U32
                | DataType::U64
                | DataType::I8
                | DataType::I16
                | DataType::I32
                // I64 is backed by the SAME `vec2<u32>` emulation as U64 (see
                // `binding_helpers`/`setup` buffer lowering and the op_dispatch
                // cast path, which treat `U64 | I64` identically and sign-extend
                // the high word for a signed source). Omitting it here falsely
                // rejected `i32 -> i64` casts the emitter and hardware handle
                // correctly — a capability/coherence gap, NOT a real limit
                // (verified on the live 5090 by `widening_cast_64_parity`).
                | DataType::I64
                | DataType::F32
                | DataType::Vec2U32
                | DataType::Vec4U32
        )
    }

    fn supports_subgroup_ops(&self) -> bool {
        self.device_profile().supports_subgroup_ops
    }

    fn supports_indirect_dispatch(&self) -> bool {
        self.device_profile().supports_indirect_dispatch
    }

    fn supports_specialization_constants(&self) -> bool {
        self.device_profile().supports_specialization_constants
    }

    fn supports_distributed_collectives(&self) -> bool {
        self.device_profile().supports_distributed_collectives
    }
}

inventory::submit! {
    vyre_driver::BackendRegistration {
        id: "wgpu",
        factory: || WgpuBackend::acquire().map(|backend| {
            Box::new(backend) as Box<dyn vyre_driver::VyreBackend>
        }),
        supported_ops: vyre_driver::backend::validation::default_supported_ops_with_trap,
    }
}

inventory::submit! {
    vyre_driver::backend::BackendPrecedence {
        id: "wgpu",
        rank: 30,
    }
}

inventory::submit! {
    vyre_driver::backend::BackendCapability {
        id: "wgpu",
        dispatches: true,
    }
}

impl vyre_driver::backend::private::Sealed for crate::pipeline::WgpuPipeline {}
impl vyre_driver::backend::private::Sealed for WgpuBackend {}
