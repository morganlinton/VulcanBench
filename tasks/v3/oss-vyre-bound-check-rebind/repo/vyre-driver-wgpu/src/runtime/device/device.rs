use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{self, Thread};
use vyre_driver::error::{Error, Result};

use crate::staging_reserve::reserve_backend_vec;

/// Snapshot of features that were actually enabled when the cached
/// device was created. Consumed by `WgpuBackend::supports_*` methods
/// so the VyreBackend capability reports are *honest*  -  a feature bit
/// is reported only if it was both advertised by the adapter AND
/// requested at device creation.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct EnabledFeatures {
    /// Wgpu timestamp queries feature.
    pub timestamp_query: bool,
    /// Wgpu timestamp writes directly on command encoders.
    pub timestamp_query_inside_encoders: bool,
    /// Wgpu subgroup feature.
    pub subgroup: bool,
    /// Wgpu subgroup barrier feature.
    pub subgroup_barrier: bool,
    /// Wgpu shader f16 feature.
    pub shader_f16: bool,
    /// Wgpu pipeline cache feature.
    pub pipeline_cache: bool,
    /// Wgpu push constants feature.
    pub push_constants: bool,
    /// Wgpu indirect first instance feature.
    pub indirect_first_instance: bool,
    /// Wgpu adapter max workgroup size limit.
    pub max_workgroup_size: [u32; 3],
    /// Wgpu adapter max storage buffer binding size limit.
    pub max_storage_buffer_binding_size: u64,
    /// Wgpu adapter max subgroup size.
    pub max_subgroup_size: u32,
    /// Wgpu adapter minimum subgroup size (I.6). `0` means the
    /// adapter did not report a subgroup size; consumers must treat
    /// subgroup-width-dependent planning as unavailable unless
    /// [`crate::capabilities::supports_subgroup_ops`] is true.
    pub min_subgroup_size: u32,
}

pub(crate) fn poll_device_once(
    device: &wgpu::Device,
) -> std::result::Result<wgpu::PollStatus, vyre_driver::BackendError> {
    device.poll(wgpu::PollType::Poll).map_err(|error| {
        vyre_driver::BackendError::new(format!(
            "wgpu device poll failed: {error}. Fix: inspect device loss and driver health before reusing this backend."
        ))
    })
}

pub(crate) fn poll_device_wait_for(
    device: &wgpu::Device,
    submission: wgpu::SubmissionIndex,
) -> std::result::Result<wgpu::PollStatus, vyre_driver::BackendError> {
    device
        .poll(wgpu::PollType::wait_for(submission))
        .map_err(|error| {
            vyre_driver::BackendError::new(format!(
                "wgpu device wait-for-submission poll failed: {error}. Fix: inspect device loss, driver health, and submission lifetime before reusing this backend."
            ))
        })
}

struct CachedRuntime {
    device_queue: Arc<(wgpu::Device, wgpu::Queue)>,
    adapter_info: wgpu::AdapterInfo,
    #[cfg(test)]
    enabled_features: EnabledFeatures,
}

static CACHED_RUNTIME: OnceLock<Result<CachedRuntime>> = OnceLock::new();

fn cached_runtime() -> &'static Result<CachedRuntime> {
    CACHED_RUNTIME.get_or_init(|| {
        #[cfg(test)]
        let ((device, queue), adapter_info, enabled_features) = init_device()?;
        #[cfg(not(test))]
        let ((device, queue), adapter_info, _enabled_features) = init_device()?;
        Ok(CachedRuntime {
            device_queue: Arc::new((device, queue)),
            adapter_info,
            #[cfg(test)]
            enabled_features,
        })
    })
}

/// Acquire the singleton device/queue pair.
///
/// ⚠ **Test / convenience helper  -  not the production path.**
///
/// Production backends construct their own `wgpu::Device` via
/// [`WgpuBackend::acquire`](crate::WgpuBackend::acquire), which routes
/// through [`init_device`] and returns a fresh device per call. Using
/// `cached_device()` from production code forces every consumer to
/// share one process-wide GPU handle, which prevents:
///
/// - running two backends against two different physical GPUs;
/// - using a dedicated discrete GPU while a test fixture is holding
///   the integrated GPU singleton;
/// - recovering from device loss (recovery swaps the backend's local
///   device; the singleton's `OnceLock` cannot be replaced in-place).
///
/// The singleton survives because a handful of test fixtures want one
/// shared GPU handle across all tests to amortize init cost. Consumers
/// that actually need a GPU runtime should construct a `WgpuBackend`
/// instead.
///
/// # Errors
///
/// Returns an error if the GPU adapter or device cannot be initialized.
#[inline]
pub fn cached_device() -> Result<Arc<(wgpu::Device, wgpu::Queue)>> {
    cached_runtime()
        .as_ref()
        .map(|runtime| Arc::clone(&runtime.device_queue))
        .map_err(Clone::clone)
}

/// Acquire adapter info for the singleton runtime device.
///
/// # Errors
///
/// Returns an error if the GPU adapter or device cannot be initialized.
#[inline]
pub fn cached_adapter_info() -> Result<&'static wgpu::AdapterInfo> {
    cached_runtime()
        .as_ref()
        .map(|runtime| &runtime.adapter_info)
        .map_err(Clone::clone)
}

/// Acquire the enabled feature snapshot for the singleton runtime device.
#[cfg(test)]
pub(crate) fn cached_enabled_features() -> Result<&'static EnabledFeatures> {
    cached_runtime()
        .as_ref()
        .map(|runtime| &runtime.enabled_features)
        .map_err(Clone::clone)
}

/// Return true when the device is the singleton cached device.
#[cfg(test)]
#[inline]
pub(crate) fn is_cached_device(device: &wgpu::Device) -> bool {
    CACHED_RUNTIME
        .get()
        .and_then(|res| res.as_ref().ok())
        .map(|runtime| &runtime.device_queue.0 == device)
        .unwrap_or(false)
}

/// Initialize a new GPU device and queue.
///
/// # Errors
///
/// Returns an actionable GPU error if no compatible adapter is available, if
/// the selected adapter is CPU-backed, or if device creation fails.
#[inline]
pub fn init_device() -> Result<(
    (wgpu::Device, wgpu::Queue),
    wgpu::AdapterInfo,
    EnabledFeatures,
)> {
    let gpu = wait_for_gpu(acquire_gpu())?;
    Ok(gpu)
}

/// Asynchronously initialize a new GPU device and queue.
///
/// # Errors
///
/// Returns an actionable GPU error if no compatible adapter is available, if
/// the selected adapter is CPU-backed, or if device creation fails.
#[inline]
pub async fn acquire_gpu() -> Result<(
    (wgpu::Device, wgpu::Queue),
    wgpu::AdapterInfo,
    EnabledFeatures,
)> {
    if let Some(index) = super::selector::adapter_index_from_env()? {
        return super::selector::acquire_gpu_for_adapter(index).await;
    }

    let instance = wgpu::Instance::default();
    let adapters = instance.enumerate_adapters(wgpu::Backends::all());
    let mut candidates = Vec::new();
    reserve_probe_vec(
        &mut candidates,
        adapters.len(),
        "GPU acquisition candidates",
    )?;
    candidates.extend(adapters.iter().filter_map(|adapter| {
        let info = adapter.get_info();
        crate::capabilities::is_real_gpu(&info).then(|| {
            let score = gpu_candidate_score(&info, adapter.features(), &adapter.limits());
            (adapter, info, score)
        })
    }));
    candidates.sort_by(|left, right| right.2.cmp(&left.2));

    let mut failures = Vec::new();
    reserve_probe_vec(&mut failures, candidates.len(), "GPU acquisition failures")?;
    for (adapter, info, _) in candidates {
        match request_device_for_adapter(adapter, "vyre device").await {
            Ok(device) => return Ok(device),
            Err(error) => failures.push(format!("{} ({:?}): {error}", info.name, info.device_type)),
        }
    }

    let mut probed = Vec::new();
    reserve_probe_vec(&mut probed, adapters.len(), "GPU acquisition probe report")?;
    probed.extend(adapters.iter().map(|adapter| {
        let info = adapter.get_info();
        format!(
            "{} ({:?}, backend={:?})",
            info.name, info.device_type, info.backend
        )
    }));
    Err(Error::Gpu {
        message: format!(
            "no real GPU adapter could create a wgpu device. Probed adapters: [{}]. Device failures: [{}]. Fix: expose a discrete, integrated, or virtual GPU through a wgpu-supported driver before running vyre.",
            probed.join(", "),
            failures.join("; ")
        ),
    })
}

pub(super) async fn request_device_for_adapter(
    adapter: &wgpu::Adapter,
    label: &'static str,
) -> Result<(
    (wgpu::Device, wgpu::Queue),
    wgpu::AdapterInfo,
    EnabledFeatures,
)> {
    let adapter_info = adapter.get_info();
    if !crate::capabilities::is_real_gpu(&adapter_info) {
        return Err(Error::Gpu {
            message: format!(
                "wgpu adapter `{}` reports device type {:?}, which is not a real GPU execution target. Fix: select a discrete, integrated, or virtual GPU adapter; CPU/software adapters are not production dispatch backends.",
                adapter_info.name, adapter_info.device_type
            ),
        });
    }
    // Opt into every feature the adapter advertises that we know how to
    // lower against. Each feature is additive: enabling it unlocks the
    // corresponding VyreBackend capability report (see
    // `WgpuBackend::supports_subgroup_ops`, `supports_f16`, etc.) and
    // costs nothing at runtime if no lowering emits the corresponding
    // intrinsic. Features we do NOT lower against (e.g. mesh shaders,
    // ray tracing) are deliberately omitted  -  enabling them would be a
    // LAW 9 evasion (claiming support that the lowering path does not
    // deliver).
    let adapter_features = adapter.features();
    let adapter_limits = adapter.limits();
    let (features, mut enabled) =
        enabled_features_for_adapter(adapter_features, &adapter_limits, adapter_info.backend);

    let device_queue = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: Some(label),
                required_features: features,
                required_limits: wgpu::Limits {
                    max_compute_workgroup_size_x: adapter_limits.max_compute_workgroup_size_x,
                    max_compute_workgroup_size_y: adapter_limits.max_compute_workgroup_size_y,
                    max_compute_workgroup_size_z: adapter_limits.max_compute_workgroup_size_z,
                    max_compute_invocations_per_workgroup: adapter_limits
                        .max_compute_invocations_per_workgroup,
                    max_compute_workgroups_per_dimension: adapter_limits
                        .max_compute_workgroups_per_dimension,
                    max_compute_workgroup_storage_size: adapter_limits
                        .max_compute_workgroup_storage_size,
                    max_storage_buffer_binding_size: adapter_limits.max_storage_buffer_binding_size,
                    // Modern adapters expose multi-GiB per-buffer caps; the
                    // wgpu spec floor is 256 MiB which is too small for
                    // batch-amortized scanners (`MAX_BATCH × num_rules
                    // × 65 536 × 4` packed-output buffer scales beyond that
                    // when MAX_BATCH grows past ~50). Take whatever the
                    // adapter reports  -  falls back to the spec floor on
                    // adapters that don't expose more.
                    max_buffer_size: adapter_limits.max_buffer_size,
                    min_subgroup_size: if enabled.subgroup {
                        adapter_limits.min_subgroup_size
                    } else {
                        0
                    },
                    max_subgroup_size: if enabled.subgroup {
                        adapter_limits.max_subgroup_size
                    } else {
                        0
                    },
                    max_storage_buffers_per_shader_stage:
                        adapter_limits.max_storage_buffers_per_shader_stage,
                    max_push_constant_size: if enabled.push_constants {
                        adapter_limits.max_push_constant_size
                    } else {
                        0
                    },
                    ..wgpu::Limits::default()
                },
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
            },
        )
        .await
        .map_err(|error| Error::Gpu {
            message: format!("failed to acquire device for adapter `{}`: {error}. Fix: check requested wgpu limits/features against the adapter and update the GPU driver if limits are unexpectedly low.", adapter_info.name),
        })?;
    let device_limits = device_queue.0.limits();
    enabled.max_workgroup_size = [
        device_limits.max_compute_workgroup_size_x,
        device_limits.max_compute_workgroup_size_y,
        device_limits.max_compute_workgroup_size_z,
    ];
    enabled.max_storage_buffer_binding_size =
        u64::from(device_limits.max_storage_buffer_binding_size);
    enabled.max_subgroup_size = device_limits.max_subgroup_size;
    enabled.min_subgroup_size = device_limits.min_subgroup_size;

    if enabled.subgroup {
        subgroup_smoke_compiles(&device_queue.0).map_err(|error| Error::Gpu {
            message: format!(
                "adapter `{}` advertises SUBGROUP but rejects the subgroup compute-pipeline smoke test: {error}. Fix: repair the wgpu feature negotiation or GPU driver; do not silently report subgroup support as disabled on a subgroup-capable adapter.",
                adapter_info.name
            ),
        })?;
    }

    Ok((device_queue, adapter_info, enabled))
}

/// wgpu only implements the persistent pipeline cache on the Vulkan and DX12
/// backends (`VK_EXT_pipeline_creation_cache_control` / `ID3D12PipelineLibrary`).
/// Apple's Metal backend (and GL) advertise the `PIPELINE_CACHE` adapter
/// feature under wgpu 25 but then fail `device_create_pipeline_cache_init`
/// with a fatal, un-catchable validation error in downstream GPU diagnostics.
/// Gate the request on a backend that actually honors it.
fn backend_implements_pipeline_cache(backend: wgpu::Backend) -> bool {
    matches!(backend, wgpu::Backend::Vulkan | wgpu::Backend::Dx12)
}

pub(super) fn enabled_features_for_adapter(
    adapter_features: wgpu::Features,
    adapter_limits: &wgpu::Limits,
    backend: wgpu::Backend,
) -> (wgpu::Features, EnabledFeatures) {
    let mut features = wgpu::Features::empty();
    let mut enabled = EnabledFeatures::default();
    if adapter_features.contains(wgpu::Features::TIMESTAMP_QUERY) {
        features |= wgpu::Features::TIMESTAMP_QUERY;
        enabled.timestamp_query = true;
    }
    if adapter_features.contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS) {
        features |= wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
        enabled.timestamp_query = true;
        enabled.timestamp_query_inside_encoders = true;
    }
    if crate::capabilities::supports_subgroup_for_adapter(adapter_features, adapter_limits) {
        features |= wgpu::Features::SUBGROUP;
        enabled.subgroup = true;
    }
    if adapter_features.contains(wgpu::Features::SUBGROUP_BARRIER) {
        features |= wgpu::Features::SUBGROUP_BARRIER;
        enabled.subgroup_barrier = true;
    }
    if adapter_features.contains(wgpu::Features::SHADER_F16) {
        features |= wgpu::Features::SHADER_F16;
        enabled.shader_f16 = true;
    }
    if adapter_features.contains(wgpu::Features::PIPELINE_CACHE)
        && backend_implements_pipeline_cache(backend)
    {
        features |= wgpu::Features::PIPELINE_CACHE;
        enabled.pipeline_cache = true;
    }
    if adapter_features.contains(wgpu::Features::PUSH_CONSTANTS) {
        features |= wgpu::Features::PUSH_CONSTANTS;
        enabled.push_constants = true;
    }
    if adapter_features.contains(wgpu::Features::INDIRECT_FIRST_INSTANCE) {
        features |= wgpu::Features::INDIRECT_FIRST_INSTANCE;
        enabled.indirect_first_instance = true;
    }

    enabled.max_workgroup_size = [
        adapter_limits.max_compute_workgroup_size_x,
        adapter_limits.max_compute_workgroup_size_y,
        adapter_limits.max_compute_workgroup_size_z,
    ];
    enabled.max_storage_buffer_binding_size =
        u64::from(adapter_limits.max_storage_buffer_binding_size);
    enabled.max_subgroup_size = adapter_limits.max_subgroup_size;
    enabled.min_subgroup_size = adapter_limits.min_subgroup_size;
    (features, enabled)
}

fn real_gpu_rank(device_type: wgpu::DeviceType) -> u8 {
    match device_type {
        wgpu::DeviceType::DiscreteGpu => 3,
        wgpu::DeviceType::IntegratedGpu => 2,
        wgpu::DeviceType::VirtualGpu => 1,
        wgpu::DeviceType::Cpu | wgpu::DeviceType::Other => 0,
    }
}

fn gpu_candidate_score(
    info: &wgpu::AdapterInfo,
    adapter_features: wgpu::Features,
    adapter_limits: &wgpu::Limits,
) -> u128 {
    let mut feature_score = 0u128;
    if crate::capabilities::supports_subgroup_for_adapter(adapter_features, adapter_limits) {
        feature_score |= 1 << 7;
    }
    if adapter_features.contains(wgpu::Features::SUBGROUP_BARRIER) {
        feature_score |= 1 << 6;
    }
    if adapter_features.contains(wgpu::Features::SHADER_F16) {
        feature_score |= 1 << 5;
    }
    if adapter_features.contains(wgpu::Features::PIPELINE_CACHE) {
        feature_score |= 1 << 4;
    }
    if adapter_features.contains(wgpu::Features::PUSH_CONSTANTS) {
        feature_score |= 1 << 3;
    }
    if adapter_features.contains(wgpu::Features::INDIRECT_FIRST_INSTANCE) {
        feature_score |= 1 << 2;
    }
    if adapter_features.contains(wgpu::Features::TIMESTAMP_QUERY) {
        feature_score |= 1 << 1;
    }
    if adapter_features.contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS) {
        feature_score |= 1;
    }

    let storage_binding_bits = u128::from(adapter_limits.max_storage_buffer_binding_size.ilog2());
    let buffer_bits = u128::from(adapter_limits.max_buffer_size.max(1).ilog2());
    let workgroup_invocations = u128::from(adapter_limits.max_compute_invocations_per_workgroup);
    let workgroup_storage_bits = u128::from(
        adapter_limits
            .max_compute_workgroup_storage_size
            .max(1)
            .ilog2(),
    );
    let storage_buffers = u128::from(adapter_limits.max_storage_buffers_per_shader_stage);

    (u128::from(real_gpu_rank(info.device_type)) << 120)
        | (feature_score << 96)
        | (storage_binding_bits << 88)
        | (buffer_bits << 80)
        | (workgroup_invocations << 56)
        | (workgroup_storage_bits << 48)
        | storage_buffers
}

fn subgroup_smoke_compiles(device: &wgpu::Device) -> std::result::Result<(), String> {
    const WGSL: &str = r#"
@compute @workgroup_size(32)
fn main(@builtin(subgroup_invocation_id) lane: u32, @builtin(subgroup_size) size: u32) {
    let total = subgroupAdd(lane + size);
    if (total == 0u) {
        return;
    }
}
"#;

    device.push_error_scope(wgpu::ErrorFilter::Validation);
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("vyre subgroup capability probe"),
        source: wgpu::ShaderSource::Wgsl(WGSL.into()),
    });
    let _pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("vyre subgroup capability probe"),
        layout: None,
        module: &module,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });
    match pop_error_scope_now(device) {
        Ok(None) => Ok(()),
        Ok(Some(error)) => Err(format!("validation error: {error}")),
        Err(error) => Err(error.to_string()),
    }
}

struct ThreadWaker(Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

struct NoopWaker;

impl Wake for NoopWaker {
    fn wake(self: Arc<Self>) {}

    fn wake_by_ref(self: &Arc<Self>) {}
}

pub(crate) fn pop_error_scope_now(
    device: &wgpu::Device,
) -> std::result::Result<Option<wgpu::Error>, &'static str> {
    device
        .poll(wgpu::PollType::Poll)
        .map_err(|_| "wgpu device poll failed before error-scope pop")?;
    let waker = Waker::from(Arc::new(NoopWaker));
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(device.pop_error_scope());
    match Future::poll(Pin::as_mut(&mut future), &mut context) {
        Poll::Ready(error) => Ok(error),
        Poll::Pending => Err(
            "wgpu error scope did not resolve after a nonblocking device poll. Fix: inspect the backend event loop; validation must not require a hot-path host wait.",
        ),
    }
}

pub(super) fn wait_for_gpu<T>(future: impl Future<Output = T>) -> T {
    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        match Pin::as_mut(&mut future).poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => thread::park(),
        }
    }
}

fn reserve_probe_vec<T>(vec: &mut Vec<T>, additional: usize, context: &'static str) -> Result<()> {
    reserve_backend_vec(vec, additional, context).map_err(|error| Error::Gpu {
        message: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cached-device helper now returns a stable singleton.
    #[test]
    fn cached_device_is_singleton() {
        let first = cached_device().expect("Fix: GPU must be available for runtime tests");
        let second = cached_device().expect("Fix: GPU must be available for runtime tests");
        assert!(
            Arc::ptr_eq(&first, &second),
            "cached_device must return the same Arc after singleton initialization"
        );
        assert!(
            is_cached_device(&first.0),
            "legacy shared APIs must still recognize cached_device-created devices"
        );
    }

    #[test]
    fn cached_adapter_info_uses_cached_runtime() {
        let info = cached_adapter_info().expect("Fix: cached adapter info must share GPU init");
        let enabled =
            cached_enabled_features().expect("Fix: cached runtime must retain capability snapshot");
        let device_queue = cached_device().expect("Fix: GPU must be available for runtime tests");
        assert!(
            !info.name.is_empty(),
            "cached adapter info must come from the initialized runtime adapter"
        );
        assert!(
            enabled.max_workgroup_size.iter().all(|axis| *axis > 0),
            "cached runtime must retain nonzero device workgroup limits for capability reporting"
        );
        assert!(
            is_cached_device(&device_queue.0),
            "cached adapter info must not replace the cached device with a second init"
        );
    }

    #[test]
    fn gpu_candidate_score_prefers_stronger_compute_adapter_within_same_class() {
        let info = wgpu::AdapterInfo {
            name: "gpu".to_string(),
            vendor: 0x10de,
            device: 0x2c02,
            device_type: wgpu::DeviceType::DiscreteGpu,
            driver: "nvidia".to_string(),
            driver_info: "test".to_string(),
            backend: wgpu::Backend::Vulkan,
        };
        let weak_limits = wgpu::Limits {
            max_storage_buffer_binding_size: 1 << 20,
            max_buffer_size: 1 << 28,
            max_compute_invocations_per_workgroup: 256,
            max_compute_workgroup_storage_size: 16 << 10,
            max_storage_buffers_per_shader_stage: 8,
            ..wgpu::Limits::default()
        };
        let strong_limits = wgpu::Limits {
            max_storage_buffer_binding_size: 1 << 30,
            max_buffer_size: 1 << 34,
            max_compute_invocations_per_workgroup: 1024,
            max_compute_workgroup_storage_size: 64 << 10,
            max_storage_buffers_per_shader_stage: 16,
            min_subgroup_size: 32,
            max_subgroup_size: 32,
            ..wgpu::Limits::default()
        };
        let weak = gpu_candidate_score(&info, wgpu::Features::empty(), &weak_limits);
        let strong = gpu_candidate_score(
            &info,
            wgpu::Features::SUBGROUP
                | wgpu::Features::SUBGROUP_BARRIER
                | wgpu::Features::SHADER_F16
                | wgpu::Features::PIPELINE_CACHE,
            &strong_limits,
        );

        assert!(
            strong > weak,
            "Fix: automatic GPU acquisition must prefer the stronger same-class compute adapter."
        );
    }

    #[test]
    fn production_device_acquisition_uses_fallible_probe_reservations() {
        let production = include_str!("device.rs")
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .expect("Fix: device production section should precede tests");

        assert!(
            !production.contains("Vec::with_capacity"),
            "Fix: centralized GPU acquisition must not use infallible capacity constructors."
        );
        assert!(
            production.contains("reserve_probe_vec"),
            "Fix: centralized GPU acquisition should reserve probe metadata through the shared staging helper."
        );
        assert!(
            production.contains("reserve_backend_vec"),
            "Fix: WGPU device acquisition should reuse the backend staging reservation policy."
        );
    }

    /// Regression guard for the Apple-Silicon GPU crash: Metal (and GL / WebGPU)
    /// advertise the `PIPELINE_CACHE` adapter feature under wgpu 25 but then fail
    /// `device_create_pipeline_cache_init` with a fatal, un-catchable validation
    /// error. `enabled_features_for_adapter` MUST gate the feature off on those
    /// backends even when the adapter advertises it; dropping the
    /// `backend_implements_pipeline_cache` guard silently reintroduces a hard macOS
    /// crash that no Linux/Windows host would surface. These are pure functions of
    /// the backend enum, so this locks the cross-OS guard without a Mac or a GPU.
    #[test]
    fn pipeline_cache_enabled_only_on_backends_that_implement_it() {
        let limits = wgpu::Limits::default();
        let advertises = wgpu::Features::PIPELINE_CACHE;

        // Backends wgpu actually implements the persistent cache on -> enable it.
        for backend in [wgpu::Backend::Vulkan, wgpu::Backend::Dx12] {
            assert!(
                backend_implements_pipeline_cache(backend),
                "{backend:?} implements the persistent pipeline cache (Vulkan/DX12)"
            );
            let (features, enabled) = enabled_features_for_adapter(advertises, &limits, backend);
            assert!(
                features.contains(wgpu::Features::PIPELINE_CACHE) && enabled.pipeline_cache,
                "{backend:?} advertises AND implements PIPELINE_CACHE -> must be enabled"
            );
        }

        // Backends that advertise the feature but crash on init -> gate OFF.
        for backend in [
            wgpu::Backend::Metal,
            wgpu::Backend::Gl,
            wgpu::Backend::BrowserWebGpu,
            wgpu::Backend::Noop,
        ] {
            assert!(
                !backend_implements_pipeline_cache(backend),
                "{backend:?} does not implement the persistent pipeline cache"
            );
            let (features, enabled) = enabled_features_for_adapter(advertises, &limits, backend);
            assert!(
                !features.contains(wgpu::Features::PIPELINE_CACHE) && !enabled.pipeline_cache,
                "{backend:?} advertises PIPELINE_CACHE but crashes on init -> must be gated OFF (Apple-Silicon crash guard)"
            );
        }

        // An implementing backend that does NOT advertise the feature -> still off
        // (no phantom enable when the adapter never offered it).
        let (features, enabled) =
            enabled_features_for_adapter(wgpu::Features::empty(), &limits, wgpu::Backend::Vulkan);
        assert!(
            !features.contains(wgpu::Features::PIPELINE_CACHE) && !enabled.pipeline_cache,
            "Vulkan without the adapter feature must not phantom-enable PIPELINE_CACHE"
        );
    }
}
