//! PTX AOT launcher emission contract tests.

use std::path::PathBuf;

use vyre_driver::aot::{emit_aot_launcher_target, AotLauncherRequest};
use vyre_driver_cuda as _;

fn request(include_collectives: bool) -> AotLauncherRequest<'static> {
    request_with_ttt(include_collectives, false)
}

fn request_with_ttt(
    include_collectives: bool,
    include_ttt_loop: bool,
) -> AotLauncherRequest<'static> {
    AotLauncherRequest {
        target: "secondary_text",
        crate_name: "test-launcher",
        include_collectives,
        include_ttt_loop,
    }
}

#[test]
fn ptx_launcher_tree_has_required_files() {
    let files = emit_aot_launcher_target("secondary_text", &request(true))
        .expect("Fix: vyre-driver-cuda must register the PTX launcher emitter.");

    for path in ["src/main.rs", "src/cuda_ffi.rs", "src/nccl_ffi.rs"] {
        assert!(
            files.files.contains_key(&PathBuf::from(path)),
            "Fix: PTX launcher tree must contain {path}."
        );
    }
    assert!(
        files
            .dependencies
            .iter()
            .any(|dep| dep.name == "libc" && dep.spec == "\"0.2\""),
        "Fix: PTX launcher must declare its driver FFI dependency."
    );
}

#[test]
fn ptx_launcher_main_calls_cuda_driver_api() {
    let files = emit_aot_launcher_target("secondary_text", &request(true))
        .expect("Fix: vyre-driver-cuda must emit PTX launcher files.");
    let main = &files.files[&PathBuf::from("src/main.rs")];

    for call in [
        "cuda::cu_init",
        "cuda::cu_device_get",
        "cuda::cu_ctx_create",
        "cuda::cu_module_load_data",
        "cuda::cu_module_get_function",
        "cuda::cu_mem_alloc",
        "cuda::cu_launch_kernel",
    ] {
        assert!(
            main.contains(call),
            "Fix: PTX launcher main.rs must call {call}."
        );
    }
}

#[test]
fn ptx_launcher_collectives_are_optional() {
    let with_collectives = emit_aot_launcher_target("secondary_text", &request(true))
        .expect("Fix: PTX launcher with collectives must emit.");
    assert!(
        with_collectives
            .files
            .contains_key(&PathBuf::from("src/nccl_ffi.rs")),
        "Fix: collectives-enabled PTX launcher must include NCCL FFI."
    );

    let without_collectives = emit_aot_launcher_target("secondary_text", &request(false))
        .expect("Fix: PTX launcher without collectives must emit.");
    assert!(
        !without_collectives
            .files
            .contains_key(&PathBuf::from("src/nccl_ffi.rs")),
        "Fix: collectives-disabled PTX launcher must omit NCCL FFI."
    );
}

#[test]
fn ptx_launcher_ttt_loop_is_cuda_owned_not_rejected() {
    let files = emit_aot_launcher_target("secondary_text", &request_with_ttt(false, true))
        .expect("Fix: CUDA PTX launcher must own eval-time TTT loops instead of rejecting them.");
    let main = &files.files[&PathBuf::from("src/main.rs")];

    for required in [
        "let mut kernel_args = cuda::KernelArgs::with_capacity(device_ptrs.len())?;",
        "run_eval_time_training_loop(kernel, &bundle, &device_ptrs, metrics_idx, &mut kernel_args, &launch_limits)?;",
        "const TTT_STEPS_ENV: &str = \"VYRE_TTT_STEPS\";",
        "const TTT_TARGET_LOSS_ENV: &str = \"VYRE_TTT_TARGET_LOSS\";",
        "launch_manifest_kernel(kernel, bundle, device_ptrs, kernel_args, launch_limits)?;",
        "read_final_metric_record(dptr, &bundle.manifest)?",
        "TTT_CONVERGED",
    ] {
        assert!(
            main.contains(required),
            "Fix: PTX launcher TTT mode must emit CUDA-owned loop code containing {required:?}."
        );
    }
    assert!(
        !main.contains("owns no TTT executor yet"),
        "Fix: PTX launcher TTT support must not preserve the old rejection path."
    );
}

#[test]
fn ptx_launcher_reuses_kernel_argument_storage_across_ttt_steps() {
    let files = emit_aot_launcher_target("secondary_text", &request_with_ttt(false, true))
        .expect("Fix: CUDA PTX launcher must emit reusable kernel argument storage.");
    let cuda_ffi = &files.files[&PathBuf::from("src/cuda_ffi.rs")];

    for required in [
        "pub struct KernelArgs",
        "pub fn cu_launch_kernel_prepared",
        "args.reset(device_ptrs)?;",
        "args.ptrs.as_mut_ptr()",
        "storage.try_reserve_exact(capacity)",
        "ptrs.try_reserve_exact(capacity)",
    ] {
        assert!(
            cuda_ffi.contains(required),
            "Fix: CUDA launcher FFI must expose reusable kernel argument storage containing {required:?}."
        );
    }
    assert!(
        !cuda_ffi.contains("device_ptrs.to_vec()"),
        "Fix: CUDA launcher hot loop must not clone device pointer args on every kernel launch."
    );
    assert!(
        !cuda_ffi.contains(".collect();"),
        "Fix: CUDA launcher hot loop must not allocate collected kernel arg pointer vectors per launch."
    );
}

#[test]
fn ptx_launcher_fails_loudly_on_manifest_size_overflow() {
    let files = emit_aot_launcher_target("secondary_text", &request(false))
        .expect("Fix: vyre-driver-cuda must emit PTX launcher files.");
    let main = &files.files[&PathBuf::from("src/main.rs")];

    assert!(
        main.contains("element_count.checked_mul(element_size_bytes)"),
        "Fix: PTX launcher must not wrap manifest buffer byte sizes in release builds."
    );
    assert!(
        main.contains("byte size overflows u64"),
        "Fix: PTX launcher manifest overflow errors must explain the corrupt buffer."
    );
    assert!(
        main.contains("if ring_size < METRIC_RECORD_WORDS"),
        "Fix: PTX launcher must reject undersized metrics rings instead of saturating to offset zero."
    );
    assert!(
        !main.contains("ring_size.saturating_sub"),
        "Fix: PTX launcher metrics offset must be checked, not saturating."
    );
    assert!(
        !main.contains("saturating_mul"),
        "Fix: PTX launcher completion backoff must use explicit checked/capped arithmetic, not saturating math."
    );
    assert!(
        !main.contains("wait_for_completion")
            && !main.contains("park_timeout")
            && !main.contains("COMPLETION_IDLE_POLLS"),
        "Fix: CUDA AOT launcher must not host-poll metrics with CPU parking; stream completion is the release-path fence."
    );
}
