//! Artifact data model contract tests.

use vyre_aot::artifact::{
    BufferAccessKind, BufferEntry, BufferMemoryKind, CompiledArtifact, DispatchConfig, Target,
};

const MINIMAL_PTX_KERNEL: &[u8] =
    b".version 8.0\n.target sm_80\n.address_size 64\n.visible .entry main() {\n\tret;\n}\n";

// ── Target ───────────────────────────────────────────────────────────

#[test]
fn target_ptx_extension_is_ptx() {
    assert_eq!(Target::Ptx.extension(), "secondary_text");
}

#[test]
fn target_spirv_extension_is_spv() {
    assert_eq!(Target::SpirV.extension(), "spv");
}

#[test]
fn target_round_trips_through_serde() {
    for target in [Target::Ptx, Target::SpirV] {
        let json = serde_json::to_string(&target).expect("Fix: Target must serialize.");
        let restored: Target = serde_json::from_str(&json).expect("Fix: Target must deserialize.");
        assert_eq!(restored, target);
    }
}

// ── BufferEntry ──────────────────────────────────────────────────────

#[test]
fn buffer_entry_total_bytes_is_count_times_size() {
    let entry = BufferEntry {
        name: "params".to_string(),
        binding: 0,
        element_count: 1024,
        element_size_bytes: 4,
        memory_kind: BufferMemoryKind::Global,
        access: BufferAccessKind::ReadOnly,
    };
    assert_eq!(entry.total_bytes(), 4096);
}

#[test]
fn buffer_entry_total_bytes_zero_for_streaming() {
    let entry = BufferEntry {
        name: "stream".to_string(),
        binding: 1,
        element_count: 0,
        element_size_bytes: 4,
        memory_kind: BufferMemoryKind::Global,
        access: BufferAccessKind::ReadWrite,
    };
    assert_eq!(
        entry.total_bytes(),
        0,
        "Fix: streaming (count=0) buffers must report 0 total bytes."
    );
}

#[test]
fn buffer_entry_total_bytes_large_count_no_overflow() {
    let entry = BufferEntry {
        name: "big".to_string(),
        binding: 0,
        element_count: u32::MAX,
        element_size_bytes: 8,
        memory_kind: BufferMemoryKind::Global,
        access: BufferAccessKind::ReadOnly,
    };
    let expected = u64::from(u32::MAX) * 8;
    assert_eq!(
        entry.total_bytes(),
        expected,
        "Fix: total_bytes must not overflow for u32::MAX elements."
    );
}

// ── BufferMemoryKind + BufferAccessKind serde ────────────────────────

#[test]
fn buffer_memory_kinds_round_trip() {
    for kind in [
        BufferMemoryKind::Global,
        BufferMemoryKind::Shared,
        BufferMemoryKind::Constant,
    ] {
        let json = serde_json::to_string(&kind).expect("Fix: BufferMemoryKind must serialize.");
        let restored: BufferMemoryKind =
            serde_json::from_str(&json).expect("Fix: BufferMemoryKind must deserialize.");
        assert_eq!(restored, kind);
    }
}

#[test]
fn buffer_access_kinds_round_trip() {
    for kind in [
        BufferAccessKind::ReadOnly,
        BufferAccessKind::WriteOnly,
        BufferAccessKind::ReadWrite,
    ] {
        let json = serde_json::to_string(&kind).expect("Fix: BufferAccessKind must serialize.");
        let restored: BufferAccessKind =
            serde_json::from_str(&json).expect("Fix: BufferAccessKind must deserialize.");
        assert_eq!(restored, kind);
    }
}

// ── DispatchConfig ───────────────────────────────────────────────────

#[test]
fn dispatch_config_round_trips_through_serde() {
    let config = DispatchConfig {
        workgroup_size: [256, 1, 1],
        grid_size: [128, 1, 1],
        dynamic_shared_bytes: 4096,
    };
    let json = serde_json::to_string(&config).expect("Fix: DispatchConfig must serialize.");
    let restored: DispatchConfig =
        serde_json::from_str(&json).expect("Fix: DispatchConfig must deserialize.");
    assert_eq!(restored.workgroup_size, config.workgroup_size);
    assert_eq!(restored.grid_size, config.grid_size);
    assert_eq!(restored.dynamic_shared_bytes, config.dynamic_shared_bytes);
}

// ── CompiledArtifact ─────────────────────────────────────────────────

#[test]
fn compiled_artifact_total_buffer_bytes_sums_all_entries() {
    let artifact = CompiledArtifact {
        target: Target::Ptx,
        kernel_bytes: vec![0; 100],
        entry_point: "main".to_string(),
        buffers: vec![
            BufferEntry {
                name: "a".to_string(),
                binding: 0,
                element_count: 10,
                element_size_bytes: 4,
                memory_kind: BufferMemoryKind::Global,
                access: BufferAccessKind::ReadOnly,
            },
            BufferEntry {
                name: "b".to_string(),
                binding: 1,
                element_count: 20,
                element_size_bytes: 4,
                memory_kind: BufferMemoryKind::Global,
                access: BufferAccessKind::WriteOnly,
            },
        ],
        dispatch: DispatchConfig {
            workgroup_size: [1, 1, 1],
            grid_size: [0, 0, 0],
            dynamic_shared_bytes: 0,
        },
        aot_version: "0.6.0".to_string(),
        vsa_fingerprint: Vec::new(),
    };
    assert_eq!(
        artifact.total_buffer_bytes(),
        10 * 4 + 20 * 4,
        "Fix: total_buffer_bytes must sum over all buffer entries."
    );
}

#[test]
fn compiled_artifact_round_trips_through_serde() {
    let artifact = CompiledArtifact {
        target: Target::Ptx,
        kernel_bytes: MINIMAL_PTX_KERNEL.to_vec(),
        entry_point: "main".to_string(),
        buffers: vec![BufferEntry {
            name: "out".to_string(),
            binding: 0,
            element_count: 64,
            element_size_bytes: 4,
            memory_kind: BufferMemoryKind::Global,
            access: BufferAccessKind::ReadWrite,
        }],
        dispatch: DispatchConfig {
            workgroup_size: [64, 1, 1],
            grid_size: [0, 0, 0],
            dynamic_shared_bytes: 0,
        },
        aot_version: "0.6.0".to_string(),
        vsa_fingerprint: vec![1, 2, 3, 4, 5, 6, 7, 8],
    };
    let json = serde_json::to_vec_pretty(&artifact).expect("Fix: CompiledArtifact must serialize.");
    let restored: CompiledArtifact =
        serde_json::from_slice(&json).expect("Fix: CompiledArtifact must deserialize.");
    assert_eq!(restored.target, artifact.target);
    assert_eq!(restored.kernel_bytes, artifact.kernel_bytes);
    assert_eq!(restored.entry_point, artifact.entry_point);
    assert_eq!(restored.buffers.len(), artifact.buffers.len());
    assert_eq!(restored.vsa_fingerprint, artifact.vsa_fingerprint);
}
