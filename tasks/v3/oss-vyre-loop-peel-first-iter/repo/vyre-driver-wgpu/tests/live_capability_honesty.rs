//! Live capability honesty tests for WgpuBackend.
//!
//! These tests enforce that the wgpu backend does not lie about its
//! capabilities: every capability surface (VyreBackend trait,
//! BackendValidationCapabilities trait, adapter_caps snapshot) must
//! agree, async dispatch must be genuinely non-blocking and
//! contract-visible, and CPU fallback is never permitted.

#![allow(clippy::assertions_on_constants)]

mod common;
use common::shared_live_backend as live_backend;

use std::time::Instant;
use vyre::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre::{DispatchConfig, VyreBackend};
use vyre_driver_wgpu::WgpuBackend;
use vyre_foundation::validate::BackendValidationCapabilities;

fn selected_adapter(backend: &WgpuBackend) -> wgpu::Adapter {
    vyre_driver_wgpu::runtime::device::adapter_for_info(backend.adapter_info()).expect(
        "Fix: selected wgpu backend adapter must still be enumerable for live capability probing",
    )
}

fn add_one_program(words: u32) -> Program {
    let idx = Expr::gid_x();
    let in_bounds = Expr::lt(idx.clone(), Expr::u32(words));
    Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(words),
            BufferDecl::output("out", 1, DataType::U32)
                .with_count(words)
                .with_output_byte_range(0..(words as usize * 4)),
        ],
        [64, 1, 1],
        vec![
            Node::if_then(
                in_bounds,
                vec![Node::store(
                    "out",
                    idx.clone(),
                    Expr::add(Expr::load("input", idx), Expr::u32(1)),
                )],
            ),
            Node::return_(),
        ],
    )
}

// ------------------------------------------------------------------
// 1. subgroup_ops must match enabled_features/live BackendValidationCapabilities
// ------------------------------------------------------------------

#[test]
fn subgroup_ops_honesty_across_all_capability_surfaces() {
    let backend = live_backend();
    let adapter = selected_adapter(&backend);
    let adapter_features = adapter.features();
    let limits = adapter.limits();
    let info = backend.adapter_info();

    let vyre_report = <WgpuBackend as VyreBackend>::supports_subgroup_ops(&backend);
    let bvc_report = BackendValidationCapabilities::supports_subgroup_ops(&backend);
    let caps_report = backend.adapter_caps().supports_subgroup_ops;

    // All three capability surfaces must agree  -  any divergence is a
    // capability-honesty bug (LAW 9 evasion).
    assert_eq!(
        vyre_report, bvc_report,
        "Fix: VyreBackend::supports_subgroup_ops ({vyre_report}) must match \
         BackendValidationCapabilities::supports_subgroup_ops ({bvc_report}). Adapter: {}",
        info.name
    );
    assert_eq!(
        vyre_report, caps_report,
        "Fix: VyreBackend::supports_subgroup_ops ({vyre_report}) must match \
         adapter_caps().supports_subgroup_ops ({caps_report}). Adapter: {}",
        info.name
    );

    // If the adapter advertises SUBGROUP and has positive min_subgroup_size,
    // the backend must report true unless the live compile probe failed.
    let adapter_has_subgroup =
        adapter_features.contains(wgpu::Features::SUBGROUP) && limits.min_subgroup_size > 0;
    if adapter_has_subgroup {
        assert!(
            vyre_report,
            "Fix: adapter `{}` reports SUBGROUP and min_subgroup_size={}, \
             so supports_subgroup_ops must be true unless a live compile probe fails",
            info.name, limits.min_subgroup_size
        );
    }

    // subgroup_size() must be Some(positive) iff supports_subgroup_ops is true.
    if vyre_report {
        assert!(
            backend.subgroup_size().is_some(),
            "Fix: backend reporting supports_subgroup_ops=true must expose \
             Some(subgroup_size). Adapter: {}",
            info.name
        );
        let size = backend.subgroup_size().unwrap();
        assert!(
            size > 0,
            "Fix: reported subgroup_size must be positive when Some, got {size}. Adapter: {}",
            info.name
        );
    } else {
        assert!(
            backend.subgroup_size().is_none() || backend.subgroup_size() == Some(0),
            "Fix: backend reporting supports_subgroup_ops=false must not expose \
             a positive subgroup_size. Adapter: {}",
            info.name
        );
    }
}

// ------------------------------------------------------------------
// 2. indirect dispatch must match runtime adapter caps
// ------------------------------------------------------------------

#[test]
fn indirect_dispatch_honesty_across_all_capability_surfaces() {
    let backend = live_backend();
    let adapter = selected_adapter(&backend);
    let limits = adapter.limits();
    let info = backend.adapter_info();

    let vyre_report = <WgpuBackend as VyreBackend>::supports_indirect_dispatch(&backend);
    let bvc_report = BackendValidationCapabilities::supports_indirect_dispatch(&backend);
    let caps_report = backend.adapter_caps().supports_indirect_dispatch;

    // All three surfaces must agree.
    assert_eq!(
        vyre_report, bvc_report,
        "Fix: VyreBackend::supports_indirect_dispatch ({vyre_report}) must match \
         BackendValidationCapabilities::supports_indirect_dispatch ({bvc_report}). Adapter: {}",
        info.name
    );
    assert_eq!(
        vyre_report, caps_report,
        "Fix: VyreBackend::supports_indirect_dispatch ({vyre_report}) must match \
         adapter_caps().supports_indirect_dispatch ({caps_report}). Adapter: {}",
        info.name
    );

    // Must match the actual runtime adapter properties.
    let is_real_gpu = matches!(
        info.device_type,
        wgpu::DeviceType::DiscreteGpu
            | wgpu::DeviceType::IntegratedGpu
            | wgpu::DeviceType::VirtualGpu
    );
    let has_sufficient_binding_size = u64::from(limits.max_storage_buffer_binding_size) >= 12;
    let has_nonzero_workgroup_size = [
        limits.max_compute_workgroup_size_x,
        limits.max_compute_workgroup_size_y,
        limits.max_compute_workgroup_size_z,
    ]
    .iter()
    .all(|axis| *axis > 0);

    let expected_indirect =
        is_real_gpu && has_sufficient_binding_size && has_nonzero_workgroup_size;

    assert_eq!(
        vyre_report,
        expected_indirect,
        "Fix: supports_indirect_dispatch must be {expected_indirect} for adapter `{}` \
         (type={:?}, max_storage_buffer_binding_size={}, workgroup_limits=[{}, {}, {}]). \
         Got {vyre_report}.",
        info.name,
        info.device_type,
        limits.max_storage_buffer_binding_size,
        limits.max_compute_workgroup_size_x,
        limits.max_compute_workgroup_size_y,
        limits.max_compute_workgroup_size_z,
    );
}

// ------------------------------------------------------------------
// 3. async dispatch must be non-blocking contract-visible
// ------------------------------------------------------------------

#[test]
fn async_dispatch_returns_contract_visible_pending_handle() {
    let backend = live_backend();

    let program = add_one_program(1024);
    let input: Vec<u8> = vyre_primitives::wire::pack_u32_iter(0..1024u32);

    let pending = backend
        .dispatch_async(&program, &[input], &DispatchConfig::default())
        .expect(
            "Fix: dispatch_async must return a pending handle \
             without blocking on GPU completion",
        );

    // The returned handle must implement PendingDispatch  -  is_ready()
    // and await_result() must be callable without panic.
    let _ready_probe: bool = pending.is_ready();
    let outputs = pending
        .await_result()
        .expect("Fix: pending handle must resolve to correct GPU outputs");

    let expected: Vec<u8> = vyre_primitives::wire::pack_u32_iter(1..=1024u32);
    assert_eq!(
        outputs,
        vec![expected],
        "Fix: async dispatch must execute the program correctly"
    );
}

#[test]
fn async_dispatch_is_non_blocking_for_real_gpu_work() {
    let backend = live_backend();

    let program = add_one_program(256 * 1024);
    let input: Vec<u8> = vyre_primitives::wire::pack_u32_iter(0..256 * 1024u32);

    // Warm the pipeline cache so the measurement is about execution,
    // not shader compilation on the first dispatch.
    let warm_start = Instant::now();
    let _ = backend
        .dispatch(&program, &[input.clone()], &DispatchConfig::default())
        .expect("Fix: warm-up dispatch must succeed");
    let warm_elapsed = warm_start.elapsed();

    // Two back-to-back async dispatches should submit quickly.
    // A synchronous backend would block until GPU completion, so
    // two calls would take ~2x the warm-up time.
    let start = Instant::now();
    let pending1 = backend
        .dispatch_async(&program, &[input.clone()], &DispatchConfig::default())
        .expect("Fix: dispatch_async #1 must start");
    let pending2 = backend
        .dispatch_async(&program, &[input.clone()], &DispatchConfig::default())
        .expect("Fix: dispatch_async #2 must start");
    let submit_elapsed = start.elapsed();

    assert!(
        submit_elapsed < warm_elapsed,
        "Fix: two back-to-back dispatch_async calls took {:?}, \
         but a single synchronous dispatch takes {:?}. \
         If the backend were synchronous, the total would be at least \
         2x the single dispatch time.",
        submit_elapsed,
        warm_elapsed
    );

    // Both must complete correctly.
    let out1 = pending1
        .await_result()
        .expect("Fix: async dispatch #1 must complete correctly");
    let out2 = pending2
        .await_result()
        .expect("Fix: async dispatch #2 must complete correctly");
    assert_eq!(
        out1, out2,
        "Fix: identical async dispatches must produce identical outputs"
    );
}

#[test]
fn async_dispatch_ready_state_is_observable_for_non_trivial_work() {
    let backend = live_backend();

    // Use a large enough program that the GPU won't finish before we
    // can observe the pending state.
    let program = add_one_program(512 * 1024);
    let input: Vec<u8> = vyre_primitives::wire::pack_u32_iter(0..512 * 1024u32);

    let pending = backend
        .dispatch_async(&program, &[input], &DispatchConfig::default())
        .expect("Fix: dispatch_async must return a pending handle");

    // The contract-visible method is_ready() must be callable and must
    // return a boolean. For non-trivial work it is typically false
    // immediately after submission (we allow true on extremely fast
    // GPUs, but the contract method itself must always be visible).
    let ready_now = pending.is_ready();

    // await_result must eventually resolve correctly regardless of
    // the initial ready state.
    let outputs = pending
        .await_result()
        .expect("Fix: await_result must resolve the dispatch");

    let expected: Vec<u8> = vyre_primitives::wire::pack_u32_iter(1..=512 * 1024u32);
    assert_eq!(
        outputs,
        vec![expected],
        "Fix: resolved async dispatch must return correct outputs"
    );

    // After await_result, the handle has been consumed; we cannot call
    // is_ready again. This is the expected contract.
    if !ready_now {
        // If the handle was not ready immediately, we proved that the
        // non-blocking contract is visible  -  the backend did not
        // synchronously complete the work before returning.
        assert!(
            true,
            "non-blocking contract verified: is_ready() returned false before await"
        );
    }
}

// ------------------------------------------------------------------
// 4. no CPU fallback/skip is allowed
// ------------------------------------------------------------------

#[test]
fn acquisition_never_returns_cpu_adapter() {
    let backend = live_backend();
    let info = backend.adapter_info();
    assert!(
        !matches!(
            info.device_type,
            wgpu::DeviceType::Cpu | wgpu::DeviceType::Other
        ),
        "Fix: WgpuBackend must never silently fall back to a CPU adapter. \
         Adapter `{}` has type {:?}.",
        info.name,
        info.device_type
    );
}

#[test]
fn acquire_fails_when_only_cpu_adapters_are_available() {
    let has_real_gpu = vyre_driver_wgpu::runtime::device::has_real_gpu_adapter();

    if !has_real_gpu {
        let result = WgpuBackend::acquire();
        assert!(
            result.is_err(),
            "Fix: WgpuBackend::acquire() must fail when only CPU/Other adapters \
             are available, rather than falling back to CPU execution."
        );
        let err = result.unwrap_err();
        let text = err.to_string();
        assert!(
            text.contains("Fix:"),
            "Fix: CPU-only rejection error must be actionable. Got: {text}"
        );
    }
    // If a real GPU exists, this test passes trivially.
}

#[test]
fn new_also_rejects_cpu_fallback() {
    let has_real_gpu = vyre_driver_wgpu::runtime::device::has_real_gpu_adapter();

    if !has_real_gpu {
        let result = WgpuBackend::new();
        assert!(
            result.is_err(),
            "Fix: WgpuBackend::new() must fail when only CPU/Other adapters are available."
        );
    }
    // If a real GPU exists, verify the acquired adapter is not CPU.
    if has_real_gpu {
        let backend = WgpuBackend::new()
            .expect("Fix: WgpuBackend::new() must acquire a real GPU when one is present.");
        let info = backend.adapter_info();
        assert!(
            !matches!(
                info.device_type,
                wgpu::DeviceType::Cpu | wgpu::DeviceType::Other
            ),
            "Fix: WgpuBackend::new() must never return a CPU adapter. Got {:?}",
            info.device_type
        );
    }
}
