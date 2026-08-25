//! Bundle packaging contract tests.

use vyre_aot::artifact::{
    BufferAccessKind, BufferEntry, BufferMemoryKind, CompiledArtifact, DispatchConfig, Target,
};
use vyre_aot::{bundle, BundleError, LauncherError, LauncherOpts};

const MINIMAL_PTX_KERNEL: &[u8] =
    b".version 8.0\n.target sm_80\n.address_size 64\n.visible .entry main() {\n\tret;\n}\n";

fn minimal_ptx_artifact() -> CompiledArtifact {
    CompiledArtifact {
        target: Target::Ptx,
        kernel_bytes: MINIMAL_PTX_KERNEL.to_vec(),
        entry_point: "main".to_string(),
        buffers: vec![
            BufferEntry {
                name: "params".to_string(),
                binding: 0,
                element_count: 256,
                element_size_bytes: 4,
                memory_kind: BufferMemoryKind::Global,
                access: BufferAccessKind::ReadOnly,
            },
            BufferEntry {
                name: "out".to_string(),
                binding: 1,
                element_count: 64,
                element_size_bytes: 4,
                memory_kind: BufferMemoryKind::Global,
                access: BufferAccessKind::WriteOnly,
            },
        ],
        dispatch: DispatchConfig {
            workgroup_size: [64, 1, 1],
            grid_size: [1, 1, 1],
            dynamic_shared_bytes: 0,
        },
        aot_version: vyre_aot::VERSION.to_string(),
        vsa_fingerprint: vec![1, 2, 3, 4, 5, 6, 7, 8],
    }
}

#[test]
fn bundle_requires_linked_launcher_emitter() {
    let dir = tempfile::tempdir().expect("Fix: tempdir must succeed.");
    let artifact = minimal_ptx_artifact();
    let weights = vec![42u8; 128];
    let opts = LauncherOpts::default();

    let err = bundle(
        dir.path(),
        &artifact,
        &weights,
        "test-bundle",
        &opts,
        "test notes",
    )
    .expect_err("Fix: vyre-aot must not bundle target launchers without a linked driver.");

    assert!(
        matches!(
            err,
            BundleError::Launcher(LauncherError::TargetNotEnabled("secondary_text"))
        ),
        "Fix: missing launcher emitter must surface as target-not-enabled, got {err:?}."
    );
}

#[test]
fn bundle_does_not_write_partial_artifacts_without_launcher() {
    let dir = tempfile::tempdir().expect("Fix: tempdir must succeed.");
    let artifact = minimal_ptx_artifact();
    let weights = vec![0u8; 16];
    let opts = LauncherOpts {
        crate_name: "test-launcher".to_string(),
        include_collectives: false,
        include_ttt_loop: false,
    };

    let _ = bundle(dir.path(), &artifact, &weights, "launcher-test", &opts, "")
        .expect_err("Fix: bundle must fail before writing a partial target bundle.");

    assert!(
        !dir.path().join("manifest.json").exists(),
        "Fix: failed bundle must not leave manifest.json behind."
    );
    assert!(
        !dir.path().join("kernel.secondary_text.lzma").exists(),
        "Fix: failed bundle must not leave compressed kernel behind."
    );
    assert!(
        !dir.path().join("weights.brotli").exists(),
        "Fix: failed bundle must not leave compressed weights behind."
    );
    assert!(
        !dir.path().join("test-launcher").exists(),
        "Fix: failed bundle must not leave a launcher tree behind."
    );
}

#[test]
fn bundle_does_not_create_output_directory_before_launcher_generation_succeeds() {
    let parent = tempfile::tempdir().expect("Fix: tempdir must succeed.");
    let out_dir = parent.path().join("missing-launcher-output");
    let artifact = minimal_ptx_artifact();
    let weights = vec![0u8; 16];
    let opts = LauncherOpts::default();

    let _ = bundle(&out_dir, &artifact, &weights, "launcher-test", &opts, "").expect_err(
        "Fix: bundle must fail before creating an output directory without a launcher emitter.",
    );

    assert!(
        !out_dir.exists(),
        "Fix: failed launcher generation must not create even an empty output directory."
    );
}

#[test]
fn bundle_does_not_create_output_directory_for_invalid_artifacts() {
    let parent = tempfile::tempdir().expect("Fix: tempdir must succeed.");
    let out_dir = parent.path().join("invalid-artifact-output");
    let mut artifact = minimal_ptx_artifact();
    artifact.dispatch.grid_size = [0, 1, 1];
    let weights = vec![0u8; 16];
    let opts = LauncherOpts::default();

    let _ = bundle(&out_dir, &artifact, &weights, "bad-grid", &opts, "")
        .expect_err("Fix: invalid artifacts must be rejected before touching the output path.");

    assert!(
        !out_dir.exists(),
        "Fix: invalid artifact rejection must not create even an empty output directory."
    );
}

#[test]
fn bundle_rejects_runtime_grid_placeholder_before_launcher_generation() {
    let dir = tempfile::tempdir().expect("Fix: tempdir must succeed.");
    let mut artifact = minimal_ptx_artifact();
    artifact.dispatch.grid_size = [0, 1, 1];
    let weights = vec![0u8; 16];
    let opts = LauncherOpts::default();

    let err = bundle(dir.path(), &artifact, &weights, "bad-grid", &opts, "")
        .expect_err("Fix: bundle must reject runtime-grid placeholders before writing artifacts.");

    assert!(
        matches!(err, BundleError::InvalidArtifact(ref message) if message.contains("runtime-grid placeholders are not bundleable")),
        "Fix: invalid grid placeholder must fail as InvalidArtifact, got {err:?}."
    );
    assert!(
        !dir.path().join("manifest.json").exists(),
        "Fix: invalid artifact rejection must happen before manifest writes."
    );
}

#[test]
fn bundle_rejects_duplicate_bindings_before_launcher_generation() {
    let dir = tempfile::tempdir().expect("Fix: tempdir must succeed.");
    let mut artifact = minimal_ptx_artifact();
    artifact.buffers[1].binding = artifact.buffers[0].binding;
    let weights = vec![0u8; 16];
    let opts = LauncherOpts::default();

    let err = bundle(dir.path(), &artifact, &weights, "bad-bindings", &opts, "").expect_err(
        "Fix: bundle must reject ambiguous CUDA argument tables before launcher generation.",
    );

    assert!(
        matches!(err, BundleError::InvalidArtifact(ref message) if message.contains("both use binding")),
        "Fix: duplicate bindings must fail as InvalidArtifact, got {err:?}."
    );
}

#[test]
fn bundle_rejects_invalid_metrics_abi_before_launcher_generation() {
    let dir = tempfile::tempdir().expect("Fix: tempdir must succeed.");
    let mut artifact = minimal_ptx_artifact();
    artifact.buffers.push(BufferEntry {
        name: "metrics".to_string(),
        binding: 2,
        element_count: 4,
        element_size_bytes: 8,
        memory_kind: BufferMemoryKind::Global,
        access: BufferAccessKind::ReadWrite,
    });
    let weights = vec![0u8; 16];
    let opts = LauncherOpts::default();

    let err = bundle(dir.path(), &artifact, &weights, "bad-metrics", &opts, "")
        .expect_err("Fix: bundle must reject metrics ABI mismatches before launcher generation.");

    assert!(
        matches!(err, BundleError::InvalidArtifact(ref message) if message.contains("metrics records are u32 words")),
        "Fix: invalid metrics ABI must fail as InvalidArtifact, got {err:?}."
    );
}

#[test]
fn bundle_rejects_weight_payload_larger_than_finite_parameter_buffer() {
    let dir = tempfile::tempdir().expect("Fix: tempdir must succeed.");
    let artifact = minimal_ptx_artifact();
    let weights = vec![0u8; 2048];
    let opts = LauncherOpts::default();

    let err = bundle(
        dir.path(),
        &artifact,
        &weights,
        "oversized-weights",
        &opts,
        "",
    )
    .expect_err(
        "Fix: bundle must reject weights that cannot fit the first finite parameter buffer.",
    );

    assert!(
        matches!(err, BundleError::InvalidArtifact(ref message) if message.contains("weights payload has")),
        "Fix: oversized weights must fail as InvalidArtifact, got {err:?}."
    );
}
