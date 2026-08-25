use std::path::Path;

pub(crate) fn workspace_relative(path: &Path) -> String {
    let s = path.to_string_lossy();
    for marker in [
        "docs/",
        "vyre-aot/",
        "vyre-bench/",
        "vyre-core/",
        "vyre-debug/",
        "vyre-driver/",
        "vyre-driver-cuda/",
        "vyre-driver-reference/",
        "vyre-driver-spirv/",
        "vyre-driver-wgpu/",
        "vyre-emit-naga/",
        "vyre-emit-ptx/",
        "vyre-emit-spirv/",
        "vyre-foundation/",
        "vyre-frontend-c/",
        "vyre-frontend-rust/",
        "vyre-harness/",
        "vyre-intrinsics/",
        "vyre-libs/",
        "vyre-lints/",
        "vyre-lower/",
        "vyre-macros/",
        "vyre-ops/",
        "vyre-primitives/",
        "vyre-reference/",
        "vyre-runtime/",
        "vyre-self-substrate/",
        "vyre-spec/",
        "vyre-std/",
        "xtask/",
    ] {
        if let Some(idx) = s.find(marker) {
            return s[idx..].to_string();
        }
    }
    s.to_string()
}
