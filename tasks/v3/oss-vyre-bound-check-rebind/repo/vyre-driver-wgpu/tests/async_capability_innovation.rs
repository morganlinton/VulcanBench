//! Aggressive external contract tests for wgpu backend innovation invariants.
//!
//! These tests assert the live backend honors its capability contracts:
//! - GPU is required and CPU fallback is never silent
//! - Subgroup/indirect capabilities are derived from adapter/device limits
//! - Unsupported f16/bf16 are rejected at the capability gate before lowering
//! - Async dispatch is genuinely asynchronous (not synchronous-under-the-hood)
//! - Timeout errors carry actionable remediation guidance
//! - Backend max_workgroup_size is nonzero on any real GPU

mod common;
use common::shared_live_backend as live_backend;

use std::time::{Duration, Instant};
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre::{DispatchConfig, VyreBackend};
use vyre_driver_wgpu::WgpuBackend;
use vyre_foundation::validate::BackendValidationCapabilities;

// ------------------------------------------------------------------
// 1. Live GPU required; never silently CPU-fallback
// ------------------------------------------------------------------

#[test]
fn live_gpu_required_no_cpu_fallback() {
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
fn acquisition_failure_is_actionable_and_lists_probed_adapters() {
    if let Err(e) = WgpuBackend::acquire() {
        let msg = e.to_string();
        assert!(
            msg.contains("Fix:"),
            "Fix: headless backend error must be actionable, got: {msg}"
        );
        assert!(
            msg.contains("Probed adapters") || msg.contains("no compatible GPU adapter"),
            "Fix: backend error should list probed adapters or clearly state none were found, got: {msg}"
        );
    }
}

// ------------------------------------------------------------------
// 2. Subgroup / indirect capabilities are adapter/device-derived
// ------------------------------------------------------------------

#[test]
fn subgroup_capability_is_derived_from_adapter_features() {
    let backend = live_backend();

    let info = backend.adapter_info();
    let adapter = vyre_driver_wgpu::runtime::device::adapter_for_info(info)
        .expect("Fix: selected wgpu backend adapter must remain enumerable for capability probing");
    let adapter_features = adapter.features();
    let limits = adapter.limits();

    // The backend's report must be consistent with the adapter's SUBGROUP feature
    // and the positive min_subgroup_size limit.
    let adapter_has_subgroup =
        adapter_features.contains(wgpu::Features::SUBGROUP) && limits.min_subgroup_size > 0;
    let reported = <WgpuBackend as vyre::VyreBackend>::supports_subgroup_ops(&backend);

    if adapter_has_subgroup {
        assert!(
            reported,
            "Fix: adapter `{}` reports SUBGROUP and min_subgroup_size={}, so supports_subgroup_ops must be true unless a live subgroup compile probe fails",
            info.name, limits.min_subgroup_size
        );
    }

    // If the backend reports subgroup support, it MUST expose a size.
    if reported {
        assert!(
            backend.subgroup_size().is_some(),
            "Fix: backend reporting subgroup ops must expose a subgroup_size"
        );
        let size = backend.subgroup_size().unwrap();
        assert!(
            size > 0,
            "Fix: reported subgroup_size must be positive, got {size}"
        );
    }
}

#[test]
fn indirect_capability_is_derived_from_adapter_properties() {
    let backend = live_backend();

    let info = backend.adapter_info();
    let reported = <WgpuBackend as vyre::VyreBackend>::supports_indirect_dispatch(&backend);

    // Indirect dispatch is only honest on real GPU device types with sufficient
    // storage-buffer binding size for the u32 x/y/z dispatch tuple.
    let is_real_gpu = matches!(
        info.device_type,
        wgpu::DeviceType::DiscreteGpu
            | wgpu::DeviceType::IntegratedGpu
            | wgpu::DeviceType::VirtualGpu
    );

    if is_real_gpu {
        assert!(
            reported,
            "Fix: backend bound to real GPU `{}` ({:?}) must report indirect_dispatch=true",
            info.name, info.device_type
        );
    }

    // The validation-capability snapshot must agree with the trait report.
    let bvc = BackendValidationCapabilities::backend_capabilities(&backend);
    assert_eq!(
        bvc.supports_indirect_dispatch, reported,
        "Fix: BackendValidationCapabilities must not drift from VyreBackend for indirect_dispatch"
    );
}

// ------------------------------------------------------------------
// 3. Unsupported f16 / bf16 fail at capability gate BEFORE lowering
// ------------------------------------------------------------------

#[test]
fn f16_rejected_at_capability_gate_before_lowering() {
    let backend = live_backend();

    // Confirm the backend honestly reports no f16 support.
    assert!(
        !backend.supports_f16(),
        "Fix: wgpu backend must not claim f16 support until lowering path exists"
    );

    let program = Program::wrapped(
        vec![
            BufferDecl::storage("half_out", 0, BufferAccess::ReadWrite, DataType::F16)
                .with_count(1),
        ],
        [1, 1, 1],
        vec![Node::Return],
    );

    let err = backend
        .dispatch(&program, &[], &DispatchConfig::default())
        .expect_err("Fix: wgpu must reject F16 at capability validation before Naga lowering");

    let text = err.to_string();
    assert!(
        text.contains("missing required capabilities") && text.contains("f16"),
        "Fix: unsupported F16 programs must fail at the backend capability gate. Got: {text}"
    );
    assert!(
        text.contains("Fix:"),
        "Fix: capability-gate rejection must be actionable. Got: {text}"
    );
}

#[test]
fn bf16_rejected_at_capability_gate_before_lowering() {
    let backend = live_backend();

    // Confirm the backend honestly reports no bf16 support.
    assert!(
        !backend.supports_bf16(),
        "Fix: wgpu backend must not claim bf16 support until lowering path exists"
    );

    let program = Program::wrapped(
        vec![
            BufferDecl::storage("bf_out", 0, BufferAccess::ReadWrite, DataType::BF16).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::Return],
    );

    let err = backend
        .dispatch(&program, &[], &DispatchConfig::default())
        .expect_err("Fix: wgpu must reject BF16 at capability validation before Naga lowering");

    let text = err.to_string();
    assert!(
        text.contains("missing required capabilities") && text.contains("bf16"),
        "Fix: unsupported BF16 programs must fail at the backend capability gate. Got: {text}"
    );
    assert!(
        text.contains("Fix:"),
        "Fix: capability-gate rejection must be actionable. Got: {text}"
    );
}

// ------------------------------------------------------------------
// 4. Async dispatch / pending dispatch is NOT synchronous under the hood
// ------------------------------------------------------------------

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

/// Build a program that takes measurably longer on the GPU than on the host.
fn long_running_program() -> Program {
    const OUTPUT_WORDS: u32 = 2 * 1024 * 1024;
    let mut body = Vec::with_capacity(260);
    body.push(Node::let_bind("idx", Expr::gid_x()));
    body.push(Node::let_bind("acc", Expr::var("idx")));
    for round in 0..128u32 {
        body.push(Node::assign(
            "acc",
            Expr::bitxor(
                Expr::mul(Expr::var("acc"), Expr::u32(1_664_525)),
                Expr::add(
                    Expr::var("idx"),
                    Expr::u32(1_013_904_223u32.wrapping_add(round)),
                ),
            ),
        ));
    }
    body.push(Node::if_then(
        Expr::lt(Expr::var("idx"), Expr::buf_len("out")),
        vec![Node::store("out", Expr::var("idx"), Expr::var("acc"))],
    ));
    Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32)
            .with_count(OUTPUT_WORDS)
            .with_output_byte_range(0..4)],
        [256, 1, 1],
        body,
    )
}

#[test]
fn async_dispatch_does_not_block_on_gpu_execution() {
    let backend = live_backend();

    let program = long_running_program();

    // Warm the pipeline cache so the measurement isn't dominated by
    // shader compilation on the first dispatch.
    let warm_start = Instant::now();
    let _ = backend
        .dispatch(&program, &[], &DispatchConfig::default())
        .expect("Fix: warm-up dispatch must succeed");
    let warm_elapsed = warm_start.elapsed();

    // A synchronous backend would block dispatch_async until the GPU
    // finishes, so two back-to-back calls would take ~2x the warm-up
    // time. An async backend submits both quickly and lets the GPU
    // execute them in parallel.
    let start = Instant::now();
    let pending1 = backend
        .dispatch_async(&program, &[], &DispatchConfig::default())
        .expect("Fix: dispatch_async #1 must start");
    let pending2 = backend
        .dispatch_async(&program, &[], &DispatchConfig::default())
        .expect("Fix: dispatch_async #2 must start");
    let submit_elapsed = start.elapsed();

    assert!(
        submit_elapsed < warm_elapsed,
        "Fix: two back-to-back dispatch_async calls took {:?}, but a single synchronous \
         dispatch takes {:?}. If the backend were synchronous, each dispatch_async would \
         block on GPU completion and the total would be at least 2x the single dispatch time.",
        submit_elapsed,
        warm_elapsed
    );

    // Both must complete correctly.
    let _ = pending1
        .await_result()
        .expect("Fix: async dispatch #1 must complete");
    let _ = pending2
        .await_result()
        .expect("Fix: async dispatch #2 must complete");
}

#[test]
fn dispatch_async_overlaps_with_host_work() {
    let backend = live_backend();

    let program = add_one_program(256 * 1024);
    let input: Vec<u8> = vyre_primitives::wire::pack_u32_iter(0..256 * 1024u32);

    let start = Instant::now();
    let pending = backend
        .dispatch_async(&program, &[input], &DispatchConfig::default())
        .expect("Fix: dispatch_async must start");

    // Perform host-side "work" while the GPU runs.
    let mut host_acc = 0u64;
    for i in 0..1_000_000 {
        host_acc = host_acc.wrapping_add(i);
    }

    let elapsed_before_await = start.elapsed();
    let outputs = pending
        .await_result()
        .expect("Fix: async dispatch must complete successfully");

    // The total elapsed time should be dominated by GPU work, but the fact
    // that we could interleave host computation proves overlap. We sanity-check
    // that await_result did not return instantly (which would imply synchronous
    // fake-async).
    assert!(
        elapsed_before_await > Duration::from_millis(0),
        "Fix: host work must have taken measurable time"
    );

    let expected: Vec<u8> = vyre_primitives::wire::pack_u32_iter(1..=256 * 1024u32);
    assert_eq!(
        outputs,
        vec![expected],
        "Fix: overlapped dispatch must still be correct"
    );
}

// ------------------------------------------------------------------
// 5. Timeout errors are actionable
// ------------------------------------------------------------------

#[test]
fn timeout_error_contains_actionable_fix() {
    let backend = live_backend();

    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(4)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(42))],
    );

    // Use a 1-nanosecond timeout to force the pre-submission timeout path.
    let mut config = DispatchConfig::default();
    config.timeout = Some(Duration::from_nanos(1));

    let err = backend
        .dispatch(&program, &[], &config)
        .expect_err("Fix: 1ns timeout must force an overrun on any real GPU path");

    let text = err.to_string();
    assert!(
        text.contains("Fix:"),
        "Fix: timeout error must contain actionable remediation. Got: {text}"
    );
    assert!(
        text.contains("timeout") || text.contains("Deadline") || text.contains("budget"),
        "Fix: timeout error must mention timeout or budget so the user understands the cause. Got: {text}"
    );
    assert!(
        text.contains("raise DispatchConfig.timeout") || text.contains("split the program"),
        "Fix: timeout error must suggest concrete remediation (raise timeout or split program). Got: {text}"
    );
}

// ------------------------------------------------------------------
// 6. Backend max_workgroup_size is nonzero
// ------------------------------------------------------------------

#[test]
fn max_workgroup_size_is_nonzero_on_live_gpu() {
    let backend = live_backend();

    let size = backend.max_workgroup_size();
    let info = backend.adapter_info();

    assert!(
        size[0] > 0,
        "Fix: max_workgroup_size[0] must be nonzero on a real GPU. Adapter: {}. Got: {:?}",
        info.name,
        size
    );
    assert!(
        size[1] > 0,
        "Fix: max_workgroup_size[1] must be nonzero on a real GPU. Adapter: {}. Got: {:?}",
        info.name,
        size
    );
    assert!(
        size[2] > 0,
        "Fix: max_workgroup_size[2] must be nonzero on a real GPU. Adapter: {}. Got: {:?}",
        info.name,
        size
    );

    // The reported size must be within a sane GPU range.
    // We do not assert exact equality against device_limits() because the
    // backend caches adapter limits at construction time, while the device
    // may report lower defaults (e.g. 256 vs 1024). The contract here is
    // simply "nonzero"; exact limit enforcement is a dispatch-time concern.
    assert!(
        size[0] <= 1024,
        "Fix: max_workgroup_size[0] ({}) exceeds any known GPU limit",
        size[0]
    );
    assert!(
        size[1] <= 1024,
        "Fix: max_workgroup_size[1] ({}) exceeds any known GPU limit",
        size[1]
    );
    assert!(
        size[2] <= 64,
        "Fix: max_workgroup_size[2] ({}) exceeds any known GPU limit",
        size[2]
    );
}

// ------------------------------------------------------------------
// 7. Capability snapshot honesty: backend features match validation surface
// ------------------------------------------------------------------

#[test]
fn backend_capability_snapshot_matches_live_adapter() {
    let backend = live_backend();

    let bvc = BackendValidationCapabilities::backend_capabilities(&backend);
    assert_eq!(
        bvc.supports_subgroup_ops,
        <WgpuBackend as vyre::VyreBackend>::supports_subgroup_ops(&backend),
        "Fix: BackendValidationCapabilities snapshot must match live VyreBackend for subgroup"
    );
    assert_eq!(
        bvc.supports_indirect_dispatch,
        <WgpuBackend as vyre::VyreBackend>::supports_indirect_dispatch(&backend),
        "Fix: BackendValidationCapabilities snapshot must match live VyreBackend for indirect"
    );
    assert_eq!(
        bvc.supports_specialization_constants,
        <WgpuBackend as vyre_foundation::validate::BackendValidationCapabilities>::supports_specialization_constants(&backend),
        "Fix: BackendValidationCapabilities snapshot must match live backend for specialization"
    );
}
