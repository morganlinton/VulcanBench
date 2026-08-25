//! Optional CUDA profiler range integration for Nsight Systems.
//!
//! This module uses NVTX when explicitly enabled by environment variable, but
//! it never hard-links `libnvToolsExt`. That keeps the CUDA backend usable on
//! hosts where the profiler runtime is not installed while still giving release
//! investigations real named ranges when Nsight is available.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    OnceLock,
};

/// NVTX range label covering one host-dispatch path.
pub const CUDA_HOST_DISPATCH_RANGE: &[u8] = b"vyre.cuda.host_dispatch\0";
/// NVTX range label covering CUDA resident-buffer dispatch.
pub const CUDA_RESIDENT_DISPATCH_RANGE: &[u8] = b"vyre.cuda.resident_dispatch\0";
/// NVTX range label covering PTX code generation.
pub const CUDA_CODEGEN_RANGE: &[u8] = b"vyre.cuda.codegen\0";
/// NVTX range label covering compiled-pipeline construction.
pub const CUDA_PIPELINE_COMPILE_RANGE: &[u8] = b"vyre.cuda.pipeline.compile\0";
/// NVTX range label covering a single compiled-pipeline dispatch.
pub const CUDA_PIPELINE_DISPATCH_RANGE: &[u8] = b"vyre.cuda.pipeline.dispatch\0";
/// NVTX range label covering batched compiled-pipeline dispatch.
pub const CUDA_PIPELINE_BATCH_DISPATCH_RANGE: &[u8] = b"vyre.cuda.pipeline.batch_dispatch\0";

/// Static profiler range metadata exported for tests, tools, and trace UIs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CudaProfilerRangeSpec {
    /// Human-readable range name without the trailing NVTX NUL byte.
    pub name: &'static str,
    /// Static NUL-terminated label passed to NVTX.
    pub label: &'static [u8],
    /// Stable explanation of the execution phase covered by the range.
    pub description: &'static str,
}

/// CUDA profiler ranges emitted by this backend.
pub const CUDA_PROFILER_RANGE_CATALOG: &[CudaProfilerRangeSpec] = &[
    CudaProfilerRangeSpec {
        name: "vyre.cuda.codegen",
        label: CUDA_CODEGEN_RANGE,
        description: "PTX lowering from vyre IR to CUDA source.",
    },
    CudaProfilerRangeSpec {
        name: "vyre.cuda.pipeline.compile",
        label: CUDA_PIPELINE_COMPILE_RANGE,
        description: "Compiled-pipeline digesting, static parameter upload, and cache setup.",
    },
    CudaProfilerRangeSpec {
        name: "vyre.cuda.pipeline.dispatch",
        label: CUDA_PIPELINE_DISPATCH_RANGE,
        description: "Single compiled-pipeline dispatch including graph replay or fallback launch.",
    },
    CudaProfilerRangeSpec {
        name: "vyre.cuda.pipeline.batch_dispatch",
        label: CUDA_PIPELINE_BATCH_DISPATCH_RANGE,
        description:
            "Batched compiled-pipeline dispatch across CUDA graph lanes or resident batches.",
    },
    CudaProfilerRangeSpec {
        name: "vyre.cuda.host_dispatch",
        label: CUDA_HOST_DISPATCH_RANGE,
        description: "Borrowed host-buffer CUDA dispatch and readback.",
    },
    CudaProfilerRangeSpec {
        name: "vyre.cuda.resident_dispatch",
        label: CUDA_RESIDENT_DISPATCH_RANGE,
        description: "CUDA-resident buffer dispatch without host-buffer fallback.",
    },
];

type NvtxRangePushA = unsafe extern "C" fn(*const libc::c_char) -> libc::c_int;
type NvtxRangePop = unsafe extern "C" fn() -> libc::c_int;

#[derive(Clone, Copy)]
struct NvtxApi {
    range_push_a: NvtxRangePushA,
    range_pop: NvtxRangePop,
}

/// RAII guard for a CUDA profiler range.
///
/// The guard is active only when profiler ranges are enabled and the NVTX
/// runtime could be loaded dynamically. Dropping an active guard pops the NVTX
/// range on the current thread.
#[must_use]
pub struct CudaProfilerRange {
    active: bool,
}

impl CudaProfilerRange {
    const fn disabled() -> Self {
        Self { active: false }
    }

    /// True when this guard pushed an NVTX range and will pop it on drop.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.active
    }
}

impl Drop for CudaProfilerRange {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        if let Some(api) = nvtx_api() {
            // SAFETY: `range_pop` was loaded from NVTX as `nvtxRangePop`.
            // It takes no arguments and closes the current thread's most
            // recent range. Failure is non-fatal profiler metadata loss.
            unsafe {
                (api.range_pop)();
            }
        }
    }
}

/// Return true when CUDA NVTX profiler ranges are requested.
#[must_use]
pub fn cuda_profiler_ranges_enabled() -> bool {
    crate::instrumentation::cuda_profiler_ranges_enabled()
}

/// Start a CUDA profiler range for a static, NUL-terminated label.
///
/// The returned guard is a cheap inactive no-op unless profiling is enabled by
/// `VYRE_CUDA_NVTX_RANGES=1` or `VYRE_CUDA_PROFILE_RANGES=1`.
#[must_use]
pub fn cuda_profiler_range(label: &'static [u8]) -> CudaProfilerRange {
    if !cuda_profiler_ranges_enabled() {
        return CudaProfilerRange::disabled();
    }
    let Some(label) = valid_nvtx_label(label) else {
        tracing::warn!(
            "CUDA profiler range label must be static, non-empty, and NUL-terminated without interior NUL bytes."
        );
        return CudaProfilerRange::disabled();
    };
    let Some(api) = nvtx_api() else {
        warn_missing_nvtx_once();
        return CudaProfilerRange::disabled();
    };

    // SAFETY: `range_push_a` was loaded from NVTX as `nvtxRangePushA`.
    // `label` is a validated static NUL-terminated byte string, so the pointer
    // remains valid for the duration of NVTX's range registration call.
    let push_depth = unsafe { (api.range_push_a)(label.as_ptr().cast()) };
    if !nvtx_push_succeeded(push_depth) {
        tracing::warn!(
            "CUDA profiler range push failed with NVTX depth {push_depth}. Fix: inspect the active profiler session before trusting CUDA range traces."
        );
        return CudaProfilerRange::disabled();
    }
    CudaProfilerRange { active: true }
}

/// Return the static CUDA profiler range catalog.
#[must_use]
pub const fn cuda_profiler_range_catalog() -> &'static [CudaProfilerRangeSpec] {
    CUDA_PROFILER_RANGE_CATALOG
}

fn warn_missing_nvtx_once() {
    static WARNED: AtomicBool = AtomicBool::new(false);
    if WARNED
        .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
        .is_ok()
    {
        tracing::warn!(
            "CUDA profiler ranges requested but libnvToolsExt could not be loaded. Fix: install the NVIDIA NVTX runtime or unset VYRE_CUDA_NVTX_RANGES."
        );
    }
}

fn valid_nvtx_label(label: &'static [u8]) -> Option<&'static [u8]> {
    if !valid_nvtx_label_bytes(label) {
        return None;
    }
    Some(label)
}

fn valid_nvtx_label_bytes(label: &[u8]) -> bool {
    label.len() >= 2 && label.last() == Some(&0) && !label[..label.len() - 1].contains(&0)
}

fn nvtx_push_succeeded(depth: libc::c_int) -> bool {
    depth >= 0
}

fn nvtx_api() -> Option<NvtxApi> {
    static API: OnceLock<Option<NvtxApi>> = OnceLock::new();
    *API.get_or_init(load_nvtx_api)
}

#[cfg(unix)]
fn load_nvtx_api() -> Option<NvtxApi> {
    let handle = open_nvtx_library()?;
    let range_push_a = load_symbol::<NvtxRangePushA>(handle, b"nvtxRangePushA\0")?;
    let range_pop = load_symbol::<NvtxRangePop>(handle, b"nvtxRangePop\0")?;
    Some(NvtxApi {
        range_push_a,
        range_pop,
    })
}

#[cfg(not(unix))]
fn load_nvtx_api() -> Option<NvtxApi> {
    None
}

#[cfg(unix)]
fn open_nvtx_library() -> Option<*mut libc::c_void> {
    for name in [&b"libnvToolsExt.so.1\0"[..], &b"libnvToolsExt.so\0"[..]] {
        let handle = {
            // SAFETY: `name` is a static NUL-terminated library name and flags are
            // the standard POSIX dynamic-loader constants. A null result means the
            // optional profiler library is unavailable.
            unsafe { libc::dlopen(name.as_ptr().cast(), libc::RTLD_NOW | libc::RTLD_LOCAL) }
        };
        if !handle.is_null() {
            return Some(handle);
        }
    }
    None
}

#[cfg(unix)]
fn load_symbol<T>(handle: *mut libc::c_void, name: &'static [u8]) -> Option<T>
where
    T: Copy,
{
    debug_assert!(valid_nvtx_label(name).is_some());
    // SAFETY: `handle` comes from a successful `dlopen`, and `name` is a static
    // NUL-terminated symbol name. A null pointer means the optional symbol is
    // unavailable.
    let symbol = unsafe { libc::dlsym(handle, name.as_ptr().cast()) };
    if symbol.is_null() {
        return None;
    }
    // SAFETY: Callers instantiate `T` only with the exact NVTX function pointer
    // signatures used for the requested symbol.
    Some(unsafe { std::mem::transmute_copy::<*mut libc::c_void, T>(&symbol) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn profiler_label_validation_rejects_hostile_labels() {
        assert!(valid_nvtx_label(CUDA_HOST_DISPATCH_RANGE).is_some());
        assert!(valid_nvtx_label(b"").is_none());
        assert!(valid_nvtx_label(b"missing nul").is_none());
        assert!(valid_nvtx_label(b"interior\0nul\0").is_none());
    }

    #[test]
    fn profiler_range_catalog_is_valid_unique_and_namespaced() {
        let mut names = BTreeSet::new();
        let mut labels = BTreeSet::new();
        for spec in cuda_profiler_range_catalog() {
            assert!(
                spec.name.starts_with("vyre.cuda."),
                "Fix: CUDA profiler range `{}` must stay in the vyre.cuda namespace so Nsight traces group correctly.",
                spec.name
            );
            assert!(
                valid_nvtx_label(spec.label).is_some(),
                "Fix: CUDA profiler range `{}` must be a valid static NVTX label.",
                spec.name
            );
            let label_text =
                std::str::from_utf8(&spec.label[..spec.label.len() - 1]).expect(
                    "Fix: CUDA profiler catalog labels must be UTF-8 so trace tooling can display them.",
                );
            assert_eq!(
                label_text, spec.name,
                "Fix: CUDA profiler range catalog name and NVTX label diverged."
            );
            assert!(
                !spec.description.is_empty(),
                "Fix: CUDA profiler range `{}` needs an actionable description for trace tooling.",
                spec.name
            );
            assert!(
                names.insert(spec.name),
                "Fix: duplicate CUDA profiler range name `{}` hides a trace phase.",
                spec.name
            );
            assert!(
                labels.insert(spec.label),
                "Fix: duplicate CUDA profiler range label `{}` hides a trace phase.",
                spec.name
            );
        }
    }

    #[test]
    fn profiler_label_validation_generated_adversarial_cases() {
        let mut checked = 0usize;
        for len in 2..=1024 {
            let mut label = vec![b'x'; len];
            label[len - 1] = 0;
            assert!(
                valid_nvtx_label_bytes(&label),
                "Fix: CUDA profiler label validation rejected a valid {len}-byte NUL-terminated label."
            );
            checked += 1;

            let interior = len / 2;
            if interior < len - 1 {
                label[interior] = 0;
                assert!(
                    !valid_nvtx_label_bytes(&label),
                    "Fix: CUDA profiler label validation accepted an interior NUL at {interior} for len {len}."
                );
                checked += 1;
            }

            label[len - 1] = b'x';
            assert!(
                !valid_nvtx_label_bytes(&label),
                "Fix: CUDA profiler label validation accepted a non-terminated {len}-byte label."
            );
            checked += 1;
        }
        for byte in 1u8..=255 {
            let label = [byte, 0];
            assert!(
                valid_nvtx_label_bytes(&label),
                "Fix: CUDA profiler label validation rejected a nonzero byte label prefix {byte}."
            );
            checked += 1;
        }
        assert!(
            checked >= 3_000,
            "Fix: generated CUDA profiler validation should exercise thousands of boundary cases, got {checked}."
        );
    }

    #[test]
    fn disabled_guard_is_inactive_and_drop_safe() {
        let guard = CudaProfilerRange::disabled();
        assert!(!guard.is_active());
    }

    #[test]
    fn profiler_push_contract_rejects_failed_nvtx_depths() {
        assert!(nvtx_push_succeeded(0));
        assert!(nvtx_push_succeeded(1));
        assert!(!nvtx_push_succeeded(-1));
    }
}
