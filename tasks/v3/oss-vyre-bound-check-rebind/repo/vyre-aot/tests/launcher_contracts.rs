//! Launcher source emission contract tests.

use vyre_aot::artifact::{
    BufferAccessKind, BufferEntry, BufferMemoryKind, CompiledArtifact, DispatchConfig, Target,
};
use vyre_aot::{emit_launcher_rust, LauncherError, LauncherOpts};

const MINIMAL_PTX_KERNEL: &[u8] =
    b".version 8.0\n.target sm_80\n.address_size 64\n.visible .entry main() {\n\tret;\n}\n";

fn minimal_ptx_artifact() -> CompiledArtifact {
    CompiledArtifact {
        target: Target::Ptx,
        kernel_bytes: MINIMAL_PTX_KERNEL.to_vec(),
        entry_point: "main".to_string(),
        buffers: vec![BufferEntry {
            name: "out".to_string(),
            binding: 0,
            element_count: 16,
            element_size_bytes: 4,
            memory_kind: BufferMemoryKind::Global,
            access: BufferAccessKind::ReadWrite,
        }],
        dispatch: DispatchConfig {
            workgroup_size: [1, 1, 1],
            grid_size: [0, 0, 0],
            dynamic_shared_bytes: 0,
        },
        aot_version: vyre_aot::VERSION.to_string(),
        vsa_fingerprint: Vec::new(),
    }
}

#[test]
fn launcher_requires_linked_target_emitter() {
    let artifact = minimal_ptx_artifact();
    let opts = LauncherOpts::default();
    let err = emit_launcher_rust(&artifact, &opts).expect_err(
        "Fix: vyre-aot must not synthesize target-owned launcher files without a linked driver.",
    );
    assert!(
        matches!(err, LauncherError::TargetNotEnabled("secondary_text")),
        "Fix: missing launcher emitter must report target-not-enabled, got {err:?}."
    );
}

#[test]
fn launcher_options_are_target_neutral() {
    let opts = LauncherOpts {
        crate_name: "custom-launcher".to_string(),
        include_collectives: false,
        include_ttt_loop: true,
    };
    assert_eq!(opts.crate_name, "custom-launcher");
    assert!(!opts.include_collectives);
    assert!(opts.include_ttt_loop);
}
