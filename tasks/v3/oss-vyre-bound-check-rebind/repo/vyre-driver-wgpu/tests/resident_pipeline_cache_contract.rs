//! WGPU resident dispatch pipeline-cache contracts.

use std::fs;
use std::path::PathBuf;

fn crate_src(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join(name)
}

#[test]
fn resident_dispatch_uses_backend_pipeline_cache_not_per_call_compile_native() {
    let backend_impl = fs::read_to_string(crate_src("backend_impl.rs"))
        .expect("WGPU backend implementation source must be readable");
    let resident_dispatch = fs::read_to_string(crate_src("resident_dispatch.rs"))
        .expect("WGPU resident dispatch source must be readable");
    let lib = fs::read_to_string(crate_src("lib.rs")).expect("WGPU lib source must be readable");
    let dispatch_start = resident_dispatch
        .find("fn dispatch_resident_timed")
        .expect("WGPU resident dispatch module must implement resident timed dispatch");
    let tail = &resident_dispatch[dispatch_start..];
    let dispatch_end = tail
        .find("fn elapsed_nanos_u64")
        .expect("resident dispatch must be followed by telemetry conversion");
    let dispatch_body = &tail[..dispatch_end];

    assert!(
        lib.contains("resident_pipeline_cache"),
        "WGPU backend must own a resident pipeline cache"
    );
    assert!(
        backend_impl.contains("pub(crate) fn compile_resident_pipeline_cached"),
        "WGPU backend must have a resident-specific cached compile helper"
    );
    assert!(
        backend_impl.contains("crate::resident_dispatch::dispatch_resident_timed"),
        "WGPU backend trait implementation must delegate resident timed dispatch to the resident dispatch module"
    );
    assert!(
        dispatch_body.contains("compile_resident_pipeline_cached"),
        "resident dispatch must use the cached resident pipeline helper"
    );
    assert!(
        !dispatch_body.contains("compile_native(program, config)"),
        "resident dispatch must not route through per-call compile_native"
    );
}
