//! Cross-crate performance contracts for Vyre consumers.
//!
//! Vyrec is a frontend/driver wrapper over the GPU-first Vyre stack. It must
//! not accidentally unlink runtime backends or bypass frontend megakernel
//! planning paths: that would leave the CLI compiling while silently disabling
//! the optimizations the release depends on.

use std::path::{Path, PathBuf};

#[test]
fn vyrec_links_gpu_backends_for_frontend_dispatch() {
    let root = santh_root();
    let cargo_toml = read(root.join("tools/vyrec/Cargo.toml"));
    assert!(
        cargo_toml.contains("vyre-driver-cuda"),
        "Fix: vyrec must link vyre-driver-cuda so CUDA backend registration is present."
    );
    assert!(
        cargo_toml.contains("vyre-driver-wgpu")
            && cargo_toml.contains("features = [\"c-parser\"]"),
        "Fix: vyrec must link vyre-driver-wgpu with the c-parser feature so frontend dispatch has a GPU backend."
    );
    assert!(
        !cargo_toml.contains("vyre-reference"),
        "Fix: vyrec must not depend on vyre-reference; CPU/reference execution belongs to parity tests, not the release CLI."
    );

    let lib_rs = read(root.join("tools/vyrec/src/lib.rs"));
    assert!(
        lib_rs.contains("use vyre_driver_cuda as _;")
            && lib_rs.contains("use vyre_driver_wgpu as _;"),
        "Fix: vyrec must keep side-effect imports for GPU backend registration."
    );
}

#[test]
fn frontend_c_backend_acquisition_fails_loudly_without_cpu_fallback() {
    let root = santh_root();
    let backend_acquire = read(
        root.join(
            "libs/performance/matching/vyre/vyre-frontend-c/src/pipeline/backend_select/backend_acquire.rs",
        ),
    );
    assert!(
        backend_acquire.contains("CUDA-first acquisition failed")
            && backend_acquire.contains("secondary WGPU GPU backend acquisition failed"),
        "Fix: frontend C backend selection must report concrete GPU backend acquisition failures."
    );
    assert!(
        !backend_acquire
            .to_ascii_lowercase()
            .contains("cpu fallback"),
        "Fix: frontend C backend acquisition must not advertise CPU fallback."
    );
}

#[test]
fn frontend_c_sparse_lexer_keeps_fused_megakernel_path() {
    let root = santh_root();
    let sparse_lexer = read(root.join(
        "libs/performance/matching/vyre/vyre-frontend-c/src/pipeline/sparse_lexer_megakernel.rs",
    ));
    assert!(
        sparse_lexer.contains("fuse_programs(&[sparse, scan, compact])"),
        "Fix: sparse lexer must keep sparse+scan+compact fused before dispatch; separate dispatches regress frontend throughput."
    );
}

fn read(path: impl AsRef<Path>) -> String {
    std::fs::read_to_string(path.as_ref()).unwrap_or_else(|error| {
        panic!(
            "Fix: cross-crate perf contract could not read {}: {error}",
            path.as_ref().display()
        )
    })
}

fn santh_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .expect("vyre-core must live under Santh/libs/performance/matching/vyre/vyre-core")
        .to_path_buf()
}
