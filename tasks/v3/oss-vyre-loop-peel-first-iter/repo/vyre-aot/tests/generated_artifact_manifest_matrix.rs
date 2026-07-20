//! Generated artifact and manifest schema matrix for AOT release bundles.
//!
//! AOT artifacts are release-boundary data: if their serde schema or byte-size
//! accounting drifts, launchers and bundle caches fail after compile time. This
//! matrix drives the public data model across generated targets, buffers,
//! dispatch geometries, and VSA fingerprints.

use vyre_aot::artifact::{
    BufferAccessKind, BufferEntry, BufferMemoryKind, CompiledArtifact, DispatchConfig, Target,
};
use vyre_aot::manifest::Manifest;

const MEMORY_KINDS: &[BufferMemoryKind] = &[
    BufferMemoryKind::Global,
    BufferMemoryKind::Shared,
    BufferMemoryKind::Constant,
];

const ACCESS_KINDS: &[BufferAccessKind] = &[
    BufferAccessKind::ReadOnly,
    BufferAccessKind::WriteOnly,
    BufferAccessKind::ReadWrite,
];

fn generated_dispatch(seed: u32) -> DispatchConfig {
    DispatchConfig {
        workgroup_size: [
            1 + (seed & 255),
            1 + ((seed >> 8) & 7),
            1 + ((seed >> 16) & 3),
        ],
        grid_size: [
            seed.rotate_left(3) & 0x3ff,
            seed.rotate_left(7) & 0xff,
            seed.rotate_left(11) & 0x3f,
        ],
        dynamic_shared_bytes: (seed & 0xff).saturating_mul(16),
    }
}

fn generated_buffers(seed: u32) -> Vec<BufferEntry> {
    let count = 1 + (seed as usize % 8);
    (0..count)
        .map(|idx| {
            let lane = seed.wrapping_add((idx as u32).wrapping_mul(0x9e37_79b9));
            BufferEntry {
                name: format!("buffer_{seed:08x}_{idx}"),
                binding: idx as u32,
                element_count: lane.rotate_left(idx as u32) & 0xffff,
                element_size_bytes: 1 << (lane & 3),
                memory_kind: MEMORY_KINDS[idx % MEMORY_KINDS.len()],
                access: ACCESS_KINDS[(idx + seed as usize) % ACCESS_KINDS.len()],
            }
        })
        .collect()
}

fn generated_fingerprint(seed: u32) -> Vec<u32> {
    (0..8)
        .map(|idx| {
            seed.wrapping_mul(0x45d9_f3b).rotate_left(idx * 3) ^ idx.wrapping_mul(0x9e37_79b9)
        })
        .collect()
}

fn generated_kernel(seed: u32) -> Vec<u8> {
    (0..(16 + (seed as usize % 96)))
        .map(|idx| seed.rotate_left((idx as u32) & 31) as u8 ^ idx as u8)
        .collect()
}

fn generated_artifact(seed: u32) -> CompiledArtifact {
    CompiledArtifact {
        target: if seed & 1 == 0 {
            Target::Ptx
        } else {
            Target::SpirV
        },
        kernel_bytes: generated_kernel(seed),
        entry_point: format!("entry_{seed:08x}"),
        buffers: generated_buffers(seed),
        dispatch: generated_dispatch(seed),
        aot_version: format!("0.4.{}", seed % 10),
        vsa_fingerprint: generated_fingerprint(seed),
    }
}

fn manifest_from_artifact(seed: u32, artifact: &CompiledArtifact) -> Manifest {
    Manifest {
        schema: Manifest::SCHEMA_VERSION.to_string(),
        aot_version: artifact.aot_version.clone(),
        artifact_name: format!("artifact_{seed:08x}"),
        target: artifact.target,
        entry_point: artifact.entry_point.clone(),
        dispatch: artifact.dispatch,
        kernel_file: format!("kernel_{seed:08x}.{}", artifact.target.extension()),
        weights_file: format!("weights_{seed:08x}.bin"),
        kernel_compression: if seed & 1 == 0 { "none" } else { "lzma" }.to_string(),
        weights_compression: if seed & 2 == 0 { "none" } else { "brotli-11" }.to_string(),
        buffers: artifact.buffers.clone(),
        kernel_sha256_hex: format!("{:064x}", seed as u64),
        weights_sha256_hex: format!("{:064x}", seed.wrapping_mul(17) as u64),
        notes: format!("generated seed {seed}"),
        vsa_fingerprint: artifact.vsa_fingerprint.clone(),
    }
}

#[test]
fn generated_compiled_artifacts_round_trip_and_preserve_size_accounting() {
    for seed in 0..512u32 {
        let artifact = generated_artifact(seed.wrapping_mul(0x9e37_79b9));
        let expected_total = artifact
            .buffers
            .iter()
            .map(|buffer| u64::from(buffer.element_count) * u64::from(buffer.element_size_bytes))
            .sum::<u64>();
        assert_eq!(artifact.total_buffer_bytes(), expected_total);

        let json =
            serde_json::to_vec(&artifact).expect("Fix: generated CompiledArtifact must serialize.");
        let restored: CompiledArtifact =
            serde_json::from_slice(&json).expect("Fix: generated CompiledArtifact must parse.");
        assert_eq!(restored.target, artifact.target);
        assert_eq!(restored.kernel_bytes, artifact.kernel_bytes);
        assert_eq!(restored.entry_point, artifact.entry_point);
        assert_eq!(restored.buffers.len(), artifact.buffers.len());
        assert_eq!(restored.total_buffer_bytes(), expected_total);
        assert_eq!(restored.vsa_fingerprint, artifact.vsa_fingerprint);
    }
}

#[test]
fn generated_manifests_round_trip_with_artifact_fields_intact() {
    for seed in 0..512u32 {
        let artifact = generated_artifact(seed ^ 0xa501_7b1d);
        let manifest = manifest_from_artifact(seed, &artifact);
        let json =
            serde_json::to_vec_pretty(&manifest).expect("Fix: generated Manifest must serialize.");
        let restored: Manifest =
            serde_json::from_slice(&json).expect("Fix: generated Manifest must parse.");

        assert_eq!(restored.schema, Manifest::SCHEMA_VERSION);
        assert_eq!(restored.aot_version, artifact.aot_version);
        assert_eq!(restored.target, artifact.target);
        assert_eq!(restored.entry_point, artifact.entry_point);
        assert_eq!(
            restored.dispatch.workgroup_size,
            artifact.dispatch.workgroup_size
        );
        assert_eq!(restored.dispatch.grid_size, artifact.dispatch.grid_size);
        assert_eq!(restored.buffers.len(), artifact.buffers.len());
        assert_eq!(restored.vsa_fingerprint, artifact.vsa_fingerprint);
        assert_eq!(restored.notes, format!("generated seed {seed}"));
    }
}
