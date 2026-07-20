//! Production-source architecture gate for hidden CPU fallback drift.
//!
//! This scans shipped `src/` trees only. Tests, docs, release evidence,
//! reference crates, and explicit CPU oracle modules may discuss CPU/reference
//! behavior; production dispatch/runtime/frontend source must not advertise or
//! implement a CPU/software/host fallback path.

use std::fs;
use std::path::{Path, PathBuf};

const PRODUCTION_SRC_DIRS: &[&str] = &[
    "vyre-core/src",
    "vyre-driver/src",
    "vyre-driver-cuda/src",
    "vyre-driver-wgpu/src",
    "vyre-emit-naga/src",
    "vyre-emit-ptx/src",
    "vyre-foundation/src",
    "vyre-frontend-c/src",
    "vyre-harness/src",
    "vyre-libs/src",
    "vyre-lower/src",
    "vyre-primitives/src",
    "vyre-runtime/src",
    "vyre-self-substrate/src",
];

const ALLOWED_FILES: &[&str] = &[
    "vyre-foundation/src/runtime/cpu_op.rs",
    "vyre-foundation/src/execution_plan/fusion/helpers.rs",
    "vyre-foundation/src/ir_inner/model/program/meta.rs",
    "vyre-driver/src/strategy/mod.rs",
    "vyre-self-substrate/src/release_validation_matrix.rs",
    "vyre-self-substrate/src/optimization_release_passes.rs",
    "vyre-self-substrate/src/hostile_input_coverage.rs",
    "vyre-self-substrate/src/cpu_fallback_reachability.rs",
    "vyre-self-substrate/src/gpu_probe_contract.rs",
    "vyre-self-substrate/src/hardware/gpu_probe_contract.rs",
    "vyre-self-substrate/src/optimizer/contracts/optimization_release_passes.rs",
    "vyre-self-substrate/src/integration/release/release_validation_matrix.rs",
    "vyre-self-substrate/src/integration/coverage/hostile_input_coverage.rs",
    "vyre-self-substrate/src/integration/quality/cpu_fallback_reachability.rs",
    "vyre-self-substrate/src/integration/quality/mod.rs",
    "vyre-self-substrate/src/c_dialect_matrix.rs",
    "vyre-self-substrate/src/deep_review_gate.rs",
    "vyre-self-substrate/src/lib.rs",
    // Capability contract: advertises the regex-accelerator class (incl. the
    // software class) and keeps unsupported devices fail-closed, making the
    // software path VISIBLE to benchmark evidence — the opposite of a hidden
    // fallback. Verified: no silent runtime degradation, only a surfaced
    // capability record.
    "vyre-driver/src/backend/regex_accelerator.rs",
    // Benchmark/parity evidence: records hardware-vs-software regex comparison
    // artifacts ("compare against hardware regex engines without implying CUDA
    // provides one"). A parity oracle, not a dispatch fallback.
    "vyre-driver-cuda/src/regex_hardware_comparison.rs",
];

const FORBIDDEN_PATTERNS: &[&str] = &[
    "cpu fallback",
    "cpu_fallback",
    "fallback to cpu",
    "fall back to cpu",
    "software fallback",
    "host fallback",
    "host-only fallback",
    "fallback dispatch",
    "fallback_to_cpu",
];

#[test]
fn production_sources_do_not_expose_hidden_cpu_fallbacks() {
    let workspace = workspace_root();
    let mut findings = Vec::new();

    for relative_dir in PRODUCTION_SRC_DIRS {
        let dir = workspace.join(relative_dir);
        assert!(
            dir.is_dir(),
            "Fix: production source scan root `{}` is missing; update the architecture gate instead of silently narrowing coverage.",
            dir.display()
        );
        scan_dir(&workspace, &dir, &mut findings);
    }
    for dir in adjacent_dataflow_source_dirs(&workspace) {
        scan_dir(&workspace, &dir, &mut findings);
    }

    assert!(
        findings.is_empty(),
        "Fix: production source must not contain hidden CPU/software/host fallback wording or paths outside explicit reference/parity oracles:\n{}",
        findings.join("\n")
    );
}

fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .expect("vyre-foundation must live directly under the vyre workspace")
        .to_path_buf()
}

fn adjacent_dataflow_source_dirs(workspace: &Path) -> Vec<PathBuf> {
    let dataflow_root = workspace.join("../../../dataflow");
    assert!(
        dataflow_root.is_dir(),
        "Fix: adjacent dataflow production source root `{}` is missing; update the architecture gate instead of silently narrowing coverage.",
        dataflow_root.display()
    );
    let entries = fs::read_dir(&dataflow_root).unwrap_or_else(|error| {
        panic!(
            "Fix: adjacent dataflow source scan could not read `{}`: {error}",
            dataflow_root.display()
        )
    });
    let mut dirs = Vec::new();
    for entry in entries {
        let entry = entry.expect("Fix: adjacent dataflow directory entry must be readable");
        let src = entry.path().join("src");
        if src.is_dir() {
            dirs.push(src);
        }
    }
    assert!(
        !dirs.is_empty(),
        "Fix: adjacent dataflow source root `{}` has no crate src/ directories; update the architecture gate instead of silently narrowing coverage.",
        dataflow_root.display()
    );
    dirs
}

fn scan_dir(workspace: &Path, dir: &Path, findings: &mut Vec<String>) {
    let entries = fs::read_dir(dir).unwrap_or_else(|error| {
        panic!(
            "Fix: production source scan could not read `{}`: {error}",
            dir.display()
        )
    });
    for entry in entries {
        let entry = entry.expect("Fix: production source directory entry must be readable");
        let path = entry.path();
        if path.is_dir() {
            scan_dir(workspace, &path, findings);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            scan_file(workspace, &path, findings);
        }
    }
}

fn scan_file(workspace: &Path, path: &Path, findings: &mut Vec<String>) {
    let relative = path.strip_prefix(workspace).unwrap_or(path);
    let relative_text = relative.to_string_lossy().replace('\\', "/");
    if ALLOWED_FILES.contains(&relative_text.as_str()) {
        return;
    }
    let text = fs::read_to_string(path).unwrap_or_else(|error| {
        panic!(
            "Fix: production source scan could not read `{}` as UTF-8 Rust source: {error}",
            path.display()
        )
    });
    let lower = text.to_ascii_lowercase();
    for pattern in FORBIDDEN_PATTERNS {
        if lower.contains(pattern) {
            findings.push(format!("{} contains `{pattern}`", relative_text));
        }
    }
}
