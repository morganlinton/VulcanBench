//! Adapter selection + enumeration (C5 refactor).
//!
//! The legacy [`super::device::cached_device`] singleton picks the
//! first adapter `wgpu::Instance::request_adapter` returns  -  fine for
//! a single-GPU dev box, useless for multi-GPU servers that need to
//! choose a specific device by vendor, index, or power preference.
//!
//! This module ships the explicit selection API:
//!
//! * [`enumerate_adapters`]  -  list every adapter wgpu reports.
//! * [`AdapterCriteria`]  -  match by device type, vendor, name
//!   substring, or power preference.
//! * [`select_adapter`]  -  pick one matching the criteria (returns
//!   the first match; callers wanting all matches iterate
//!   [`enumerate_adapters`] themselves).
//! * [`init_device_for_adapter`]  -  build a device+queue bound to the
//!   chosen adapter.
//! * `VYRE_ADAPTER_INDEX`  -  env override used by the backend
//!   auto-picker to route programs to a specific device without
//!   patching code.
//!
//! The legacy `cached_device()` still serves the default case: one
//! singleton device, first compatible adapter. Callers that want
//! multi-GPU now select an adapter by index before constructing a
//! device/queue pair.

use vyre_driver::error::{Error, Result};

use crate::staging_reserve::reserve_backend_vec;

/// Stable adapter identity used for deterministic recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AdapterIdentity {
    name: String,
    vendor: u32,
    device: u32,
    device_type: wgpu::DeviceType,
    driver: String,
    driver_info: String,
    backend: wgpu::Backend,
}

impl AdapterIdentity {
    pub(crate) fn from_info(info: &wgpu::AdapterInfo) -> Self {
        Self {
            name: info.name.clone(),
            vendor: info.vendor,
            device: info.device,
            device_type: info.device_type,
            driver: info.driver.clone(),
            driver_info: info.driver_info.clone(),
            backend: info.backend,
        }
    }

    fn matches(&self, info: &wgpu::AdapterInfo) -> bool {
        self.name == info.name
            && self.vendor == info.vendor
            && self.device == info.device
            && self.device_type == info.device_type
            && self.driver == info.driver
            && self.driver_info == info.driver_info
            && self.backend == info.backend
    }
}

/// Criteria used by [`select_adapter`].
#[derive(Debug, Default, Clone)]
pub struct AdapterCriteria {
    /// Prefer an adapter whose `device_type` matches.
    pub device_type: Option<wgpu::DeviceType>,
    /// Prefer an adapter whose vendor id matches.
    pub vendor: Option<u32>,
    /// Prefer an adapter whose name contains this substring
    /// (case-insensitive).
    pub name_contains: Option<String>,
    /// Prefer an adapter with this power policy.
    pub power: Option<wgpu::PowerPreference>,
}

/// Human-readable adapter probe details for GPU acquisition failures.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdapterProbeReport {
    /// Adapters visible to wgpu during the centralized probe.
    pub probed: Vec<String>,
    /// Feature, limit, or device-request reasons that prevented use.
    pub missing: Vec<String>,
}

impl AdapterCriteria {
    /// Build criteria for a high-performance discrete GPU.
    #[must_use]
    pub fn high_performance() -> Self {
        Self {
            device_type: Some(wgpu::DeviceType::DiscreteGpu),
            power: Some(wgpu::PowerPreference::HighPerformance),
            ..Self::default()
        }
    }

    /// Build criteria for a low-power integrated GPU (laptop
    /// battery savings).
    #[must_use]
    pub fn low_power() -> Self {
        Self {
            device_type: Some(wgpu::DeviceType::IntegratedGpu),
            power: Some(wgpu::PowerPreference::LowPower),
            ..Self::default()
        }
    }
}

/// List every adapter the wgpu instance reports.
#[must_use]
pub fn enumerate_adapters() -> Vec<wgpu::AdapterInfo> {
    match try_enumerate_adapters() {
        Ok(adapters) => adapters,
        Err(_) => Vec::new(),
    }
}

/// List every adapter the wgpu instance reports with fallible metadata staging.
///
/// # Errors
///
/// Returns `Error::Gpu` when probe-result metadata cannot be reserved.
pub(crate) fn try_enumerate_adapters() -> Result<Vec<wgpu::AdapterInfo>> {
    let instance = wgpu::Instance::default();
    let adapters = instance.enumerate_adapters(wgpu::Backends::all());
    let mut infos = Vec::new();
    reserve_probe_vec(&mut infos, adapters.len(), "adapter enumeration metadata")?;
    infos.extend(adapters.iter().map(wgpu::Adapter::get_info));
    Ok(infos)
}

/// Report whether the centralized adapter probe can see at least one real GPU.
#[must_use]
pub fn has_real_gpu_adapter() -> bool {
    let instance = wgpu::Instance::default();
    instance
        .enumerate_adapters(wgpu::Backends::all())
        .iter()
        .any(|adapter| crate::capabilities::is_real_gpu(&adapter.get_info()))
}

/// Re-open the live wgpu adapter matching a previously selected adapter info.
///
/// Tests and capability probes use this instead of directly constructing their
/// own `wgpu::Instance` so adapter identity and failure diagnostics stay in the
/// runtime device contract.
///
/// # Errors
///
/// Returns `Error::Gpu` when the adapter is no longer visible.
pub fn adapter_for_info(expected: &wgpu::AdapterInfo) -> Result<wgpu::Adapter> {
    let instance = wgpu::Instance::default();
    let adapters = instance.enumerate_adapters(wgpu::Backends::all());
    let mut probed = Vec::new();
    reserve_probe_vec(&mut probed, adapters.len(), "adapter recovery probe report")?;
    for adapter in adapters {
        let candidate = adapter.get_info();
        if adapter_info_matches(&candidate, expected) {
            return Ok(adapter);
        }
        probed.push(format!(
            "{} ({:?}, backend={:?}, vendor={:08x}, device={:08x})",
            candidate.name,
            candidate.device_type,
            candidate.backend,
            candidate.vendor,
            candidate.device
        ));
    }

    Err(Error::Gpu {
        message: format!(
            "selected adapter `{}` ({:?}, backend={:?}, vendor={:08x}, device={:08x}) is no longer enumerable. Probed adapters: [{}]. Fix: repair GPU visibility or reacquire the WGPU backend.",
            expected.name,
            expected.device_type,
            expected.backend,
            expected.vendor,
            expected.device,
            probed.join(", ")
        ),
    })
}

fn adapter_info_matches(candidate: &wgpu::AdapterInfo, expected: &wgpu::AdapterInfo) -> bool {
    candidate.name == expected.name
        && candidate.vendor == expected.vendor
        && candidate.device == expected.device
        && candidate.device_type == expected.device_type
        && candidate.driver == expected.driver
        && candidate.driver_info == expected.driver_info
        && candidate.backend == expected.backend
}

/// Build the centralized adapter diagnostic report used by acquisition errors.
#[must_use]
pub fn adapter_probe_report() -> AdapterProbeReport {
    let instance = wgpu::Instance::default();
    let adapters = instance.enumerate_adapters(wgpu::Backends::all());
    let mut report = AdapterProbeReport {
        probed: Vec::new(),
        missing: Vec::new(),
    };

    for adapter in adapters {
        let info = adapter.get_info();
        report.probed.push(format!(
            "{} ({:?}, backend={:?})",
            info.name, info.device_type, info.backend
        ));
        if matches!(
            info.device_type,
            wgpu::DeviceType::Cpu | wgpu::DeviceType::Other
        ) {
            continue;
        }
        if !adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            report.missing.push("TIMESTAMP_QUERY".to_string());
        }
        if !adapter
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS)
        {
            report
                .missing
                .push("TIMESTAMP_QUERY_INSIDE_ENCODERS".to_string());
        }
        let adapter_limits = adapter.limits();
        if let Err(error) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("vyre probe"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits {
                max_storage_buffers_per_shader_stage:
                    adapter_limits.max_storage_buffers_per_shader_stage,
                ..wgpu::Limits::default()
            },
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
        })) {
            report
                .missing
                .push(format!("device request failed on {}: {error}", info.name));
        }
    }

    report
}

/// Select the first adapter matching `criteria`. Returns its index
/// into [`enumerate_adapters`] plus its info.
///
/// # Errors
///
/// Returns `Error::Gpu` when no adapter matches.
pub fn select_adapter(criteria: &AdapterCriteria) -> Result<(usize, wgpu::AdapterInfo)> {
    let instance = wgpu::Instance::default();
    let adapters = instance.enumerate_adapters(wgpu::Backends::all());
    for (idx, adapter) in adapters.iter().enumerate() {
        let info = adapter.get_info();
        if adapter_is_selectable(&info, criteria) {
            return Ok((idx, info));
        }
    }
    Err(Error::Gpu {
        message: format!(
            "no real GPU adapter matches criteria {criteria:?}. Fix: loosen the criteria or install drivers exposing the requested GPU class."
        ),
    })
}

fn adapter_is_selectable(info: &wgpu::AdapterInfo, criteria: &AdapterCriteria) -> bool {
    crate::capabilities::is_real_gpu(info) && adapter_matches(info, criteria)
}

fn adapter_matches(info: &wgpu::AdapterInfo, criteria: &AdapterCriteria) -> bool {
    if let Some(ty) = criteria.device_type {
        if info.device_type != ty {
            return false;
        }
    }
    if let Some(vendor) = criteria.vendor {
        if info.vendor != vendor {
            return false;
        }
    }
    if let Some(needle) = &criteria.name_contains {
        if !adapter_name_contains(&info.name, needle) {
            return false;
        }
    }
    true
}

fn adapter_name_contains(name: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if name.is_ascii() && needle.is_ascii() {
        return name
            .as_bytes()
            .windows(needle.len())
            .any(|window| window.eq_ignore_ascii_case(needle.as_bytes()));
    }
    name.to_lowercase().contains(&needle.to_lowercase())
}

/// Initialize a device + queue bound to the adapter at `index`.
///
/// Pairs with [`enumerate_adapters`] / [`select_adapter`] to give
/// callers full control over which GPU the backend binds to.
///
/// # Errors
///
/// Returns `Error::Gpu` when `index` is out of range or device
/// creation fails.
pub fn init_device_for_adapter(
    index: usize,
) -> Result<(
    (wgpu::Device, wgpu::Queue),
    wgpu::AdapterInfo,
    crate::runtime::device::EnabledFeatures,
)> {
    super::device::wait_for_gpu(acquire_gpu_for_adapter(index))
}

/// Recreate a device on the same adapter identity used by an existing backend.
///
/// # Errors
///
/// Returns `Error::Gpu` when the adapter disappeared, no longer reports as a
/// real GPU, or rejects device creation.
pub(crate) fn init_device_for_adapter_identity(
    identity: &AdapterIdentity,
) -> Result<(
    (wgpu::Device, wgpu::Queue),
    wgpu::AdapterInfo,
    crate::runtime::device::EnabledFeatures,
)> {
    super::device::wait_for_gpu(acquire_gpu_for_adapter_identity(identity))
}

async fn acquire_gpu_for_adapter_identity(
    identity: &AdapterIdentity,
) -> Result<(
    (wgpu::Device, wgpu::Queue),
    wgpu::AdapterInfo,
    crate::runtime::device::EnabledFeatures,
)> {
    let instance = wgpu::Instance::default();
    let adapters = instance.enumerate_adapters(wgpu::Backends::all());
    for adapter in &adapters {
        let info = adapter.get_info();
        if identity.matches(&info) {
            if !crate::capabilities::is_real_gpu(&info) {
                return Err(Error::Gpu {
                    message: format!(
                        "recovery target `{}` now reports device type {:?}, which is not a real GPU execution target. Fix: restore the original GPU adapter or construct a new backend for the changed adapter.",
                        info.name, info.device_type
                    ),
                });
            }
            return super::device::request_device_for_adapter(adapter, "vyre device (recovered)")
                .await;
        }
    }

    let mut probed = Vec::new();
    reserve_probe_vec(
        &mut probed,
        adapters.len(),
        "adapter identity recovery probe report",
    )?;
    probed.extend(adapters.iter().map(|adapter| {
        let info = adapter.get_info();
        format!(
            "{} ({:?}, backend={:?}, vendor={:08x}, device={:08x})",
            info.name, info.device_type, info.backend, info.vendor, info.device
        )
    }));
    Err(Error::Gpu {
        message: format!(
            "original recovery adapter was not found. Target: {:?}. Probed adapters: [{}]. Fix: restore the original GPU or create a new WgpuBackend for the available adapter.",
            identity,
            probed.join(", ")
        ),
    })
}

/// Async variant of [`init_device_for_adapter`].
///
/// # Errors
///
/// Returns `Error::Gpu` when `index` is out of range or device
/// creation fails.
pub async fn acquire_gpu_for_adapter(
    index: usize,
) -> Result<(
    (wgpu::Device, wgpu::Queue),
    wgpu::AdapterInfo,
    crate::runtime::device::EnabledFeatures,
)> {
    let instance = wgpu::Instance::default();
    let adapters = instance.enumerate_adapters(wgpu::Backends::all());
    let adapter = adapters.get(index).ok_or_else(|| Error::Gpu {
        message: format!(
            "adapter index {index} out of range (saw {} adapters). Fix: call enumerate_adapters() first to see valid indices.",
            adapters.len()
        ),
    })?;
    let info = adapter.get_info();
    if !crate::capabilities::is_real_gpu(&info) {
        return Err(Error::Gpu {
            message: format!(
                "adapter index {index} resolved to `{}` with device type {:?}, which is not a real GPU execution target. Fix: choose a discrete, integrated, or virtual GPU adapter.",
                info.name, info.device_type
            ),
        });
    }
    super::device::request_device_for_adapter(adapter, "vyre device (selected)").await
}

/// Read the `VYRE_ADAPTER_INDEX` env override. `None` when unset.
///
/// # Errors
///
/// Returns an actionable GPU configuration error when the env var is
/// set but cannot be parsed. A typoed adapter override must not
/// silently fall back to automatic GPU selection.
#[must_use]
pub fn adapter_index_from_env() -> Result<Option<usize>> {
    adapter_index_from_raw(std::env::var("VYRE_ADAPTER_INDEX").ok().as_deref())
}

fn adapter_index_from_raw(raw: Option<&str>) -> Result<Option<usize>> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    raw.parse::<usize>().map(Some).map_err(|error| Error::Gpu {
        message: format!(
            "VYRE_ADAPTER_INDEX={raw:?} is not a valid adapter index: {error}. Fix: set VYRE_ADAPTER_INDEX to a non-negative integer from enumerate_adapters(), or unset it for automatic GPU selection."
        ),
    })
}

fn reserve_probe_vec<T>(vec: &mut Vec<T>, additional: usize, context: &'static str) -> Result<()> {
    reserve_backend_vec(vec, additional, context).map_err(|error| Error::Gpu {
        message: error.to_string(),
    })
}

#[cfg(test)]

mod tests {
    use super::*;

    #[test]
    fn enumerate_adapters_finds_required_gpu() {
        let adapters = enumerate_adapters();
        assert_ne!(adapters.len(), 0,
            "Fix: WGPU adapter enumeration returned no adapters on a GPU-required release host; repair driver/runtime configuration instead of accepting a CPU-only environment."
        );
    }

    #[test]
    fn criteria_high_perf_has_discrete_preset() {
        let c = AdapterCriteria::high_performance();
        assert_eq!(c.device_type, Some(wgpu::DeviceType::DiscreteGpu));
        assert_eq!(c.power, Some(wgpu::PowerPreference::HighPerformance));
    }

    #[test]
    fn criteria_low_power_has_integrated_preset() {
        let c = AdapterCriteria::low_power();
        assert_eq!(c.device_type, Some(wgpu::DeviceType::IntegratedGpu));
    }

    #[test]
    fn env_override_parses_valid_index() {
        assert_eq!(adapter_index_from_raw(Some("3")).unwrap(), Some(3));
    }

    #[test]
    fn env_override_rejects_garbage() {
        let error = adapter_index_from_raw(Some("not-a-number"))
            .expect_err("invalid VYRE_ADAPTER_INDEX must error");
        assert!(
            error.to_string().contains("VYRE_ADAPTER_INDEX"),
            "Fix: invalid adapter-index errors must name the misconfigured env var"
        );
    }

    #[test]
    fn selection_rejects_cpu_adapters_before_device_acquisition() {
        let cpu_info = wgpu::AdapterInfo {
            name: "llvmpipe".to_string(),
            vendor: 0,
            device: 0,
            device_type: wgpu::DeviceType::Cpu,
            driver: "software".to_string(),
            driver_info: "cpu".to_string(),
            backend: wgpu::Backend::Vulkan,
        };
        let gpu_info = wgpu::AdapterInfo {
            name: "RTX 5090".to_string(),
            vendor: 0x10de,
            device: 0x2c02,
            device_type: wgpu::DeviceType::DiscreteGpu,
            driver: "nvidia".to_string(),
            driver_info: "gpu".to_string(),
            backend: wgpu::Backend::Vulkan,
        };
        let criteria = AdapterCriteria::default();

        assert!(
            !adapter_is_selectable(&cpu_info, &criteria),
            "Fix: adapter selection must never return CPU/Other devices for later fallback handling."
        );
        assert!(adapter_is_selectable(&gpu_info, &criteria));
    }

    #[test]
    fn adapter_identity_matches_every_recovery_field() {
        let info = wgpu::AdapterInfo {
            name: "gpu-a".to_string(),
            vendor: 0x10de,
            device: 0x2684,
            device_type: wgpu::DeviceType::DiscreteGpu,
            driver: "nvidia".to_string(),
            driver_info: "driver-a".to_string(),
            backend: wgpu::Backend::Vulkan,
        };
        let identity = AdapterIdentity::from_info(&info);
        assert!(identity.matches(&info));

        let mut changed = info.clone();
        changed.device = 0x2685;
        assert!(
            !identity.matches(&changed),
            "Fix: recovery must not silently bind to a different physical adapter."
        );
    }

    #[test]
    fn adapter_name_contains_matches_ascii_without_lowercase_in_hot_path() {
        assert!(adapter_name_contains("NVIDIA GeForce RTX 5090", "rtx"));
        assert!(adapter_name_contains("NVIDIA GeForce RTX 5090", "RTX"));
        assert!(!adapter_name_contains("NVIDIA GeForce RTX 5090", "radeon"));
        assert!(adapter_name_contains("Mötley GPU", "mötley"));
    }

    #[test]
    fn production_selector_uses_fallible_probe_reservations() {
        let production = include_str!("selector.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: selector production section should precede tests");

        assert!(
            !production.contains("Vec::with_capacity"),
            "Fix: GPU probe paths must not use infallible capacity constructors."
        );
        assert!(
            production.contains("reserve_probe_vec"),
            "Fix: GPU probe metadata should reserve through the shared WGPU staging helper."
        );
        assert!(
            !production.contains("info.name.to_lowercase()"),
            "Fix: adapter matching must not allocate lowercase strings per adapter."
        );
        assert!(production.contains("adapter_name_contains"));
    }
}
