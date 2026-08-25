//! CUDA e-graph device-image upload planning tests.

#[path = "egraph_device_image_upload/upload_layout_contracts.rs"]
mod upload_layout_contracts;
#[path = "egraph_device_image_upload/source_snapshot_contracts.rs"]
mod source_snapshot_contracts;
#[path = "egraph_device_image_upload/structural_equivalence_contracts.rs"]
mod structural_equivalence_contracts;
#[path = "egraph_device_image_upload/union_compaction_contracts.rs"]
mod union_compaction_contracts;
#[path = "egraph_device_image_upload/canonical_rewrite_contracts.rs"]
mod canonical_rewrite_contracts;
#[path = "egraph_device_image_upload/fixed_point_contracts.rs"]
mod fixed_point_contracts;

use vyre_driver_cuda::{
    pack_cuda_egraph_canonical_rewrite_device_image, plan_cuda_egraph_device_upload,
    plan_cuda_egraph_device_upload_from_image, plan_cuda_egraph_device_upload_from_image_ref,
    plan_cuda_egraph_signature_buckets, plan_cuda_egraph_structural_equivalence_launch_artifact,
    plan_cuda_egraph_union_compaction, CudaBackend, CudaEGraphCanonicalRewrite,
    CudaEGraphDeviceByteLayout, CudaEGraphDeviceByteSpan, CudaEGraphDeviceUploadError,
    CudaEGraphFixedPointReadback, CudaEGraphKernelLaunchConfig,
    CudaEGraphSignatureBucketDeviceImage, CudaEGraphSignaturePairWave,
    CudaEGraphStructuralEquivalenceLaunchArtifact, CudaEGraphStructuralEquivalenceOutputPlan,
    CudaEGraphUnionCompactionPass, CudaEGraphUnionCompactionPlan,
    CUDA_EGRAPH_SIGNATURE_BUCKET_RECORD_WORDS, CUDA_EGRAPH_STRUCTURAL_EQUIVALENCE_KERNEL_ENTRY,
};
use vyre_foundation::optimizer::eqsat_gpu::{
    Equivalence, GpuEGraphDeviceImageError, GpuEGraphSnapshot,
};

fn expected_column_snapshot_bytes(layout: CudaEGraphDeviceByteLayout) -> usize {
    [
        layout.row_eclass_ids(),
        layout.row_language_op_ids(),
        layout.row_children_offsets(),
        layout.row_children_lens(),
        layout.row_signatures(),
        layout.children(),
    ]
    .iter()
    .map(CudaEGraphDeviceByteSpan::byte_len)
    .sum()
}


fn assert_span_matches_foundation(
    cuda: CudaEGraphDeviceByteSpan,
    foundation: vyre_foundation::optimizer::eqsat_gpu::GpuEGraphDeviceSpan,
) {
    assert_eq!(cuda.offset(), foundation.offset() * 4);
    assert_eq!(cuda.byte_len(), foundation.len() * 4);
}


fn next_u32(seed: &mut u64) -> u32 {
    *seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (*seed >> 32) as u32
}

fn assert_sorted_unique_pairs(plan: &CudaEGraphUnionCompactionPlan) {
    let mut previous = None;
    for pair in &plan.canonical_pairs {
        assert!(
            pair.left < pair.right,
            "Fix: CUDA e-graph union planner must drop self pairs and order reversed pairs."
        );
        if let Some(previous) = previous {
            assert!(
                previous < (pair.left, pair.right),
                "Fix: CUDA e-graph union planner must sort and deduplicate merge pairs."
            );
        }
        previous = Some((pair.left, pair.right));
    }
}

fn assert_rewrites_are_final_representatives(plan: &CudaEGraphUnionCompactionPlan) {
    for rewrite in &plan.canonical_rewrites {
        assert!(
            rewrite.representative < rewrite.eclass_id,
            "Fix: CUDA e-graph union compaction must choose the minimum e-class id as representative."
        );
        assert!(
            plan.affected_eclasses
                .binary_search(&rewrite.representative)
                .is_ok(),
            "Fix: CUDA e-graph union representative must be part of the affected e-class set."
        );
        assert_eq!(
            planned_representative(plan, rewrite.representative),
            rewrite.representative,
            "Fix: CUDA e-graph canonical rewrites must be final, not transitive chains."
        );
    }
    for pair in &plan.canonical_pairs {
        assert_eq!(
            planned_representative(plan, pair.left),
            planned_representative(plan, pair.right),
            "Fix: every planned union pair endpoint must collapse to one representative."
        );
    }
}

fn planned_representative(plan: &CudaEGraphUnionCompactionPlan, eclass_id: u32) -> u32 {
    plan.canonical_rewrites
        .iter()
        .find(|rewrite| rewrite.eclass_id == eclass_id)
        .map_or(eclass_id, |rewrite| rewrite.representative)
}

fn assert_wave_coverage(plan: &CudaEGraphUnionCompactionPlan, max_items_per_wave: u64) {
    let union_items = plan
        .waves
        .iter()
        .filter(|wave| wave.pass == CudaEGraphUnionCompactionPass::UnionPairs)
        .map(|wave| {
            assert!(wave.item_count <= max_items_per_wave);
            assert!(u64::from(wave.blocks * wave.threads_per_block) >= wave.item_count);
            wave.item_count
        })
        .sum::<u64>();
    let rewrite_items = plan
        .waves
        .iter()
        .filter(|wave| wave.pass == CudaEGraphUnionCompactionPass::CanonicalRewrites)
        .map(|wave| {
            assert!(wave.item_count <= max_items_per_wave);
            assert!(u64::from(wave.blocks * wave.threads_per_block) >= wave.item_count);
            wave.item_count
        })
        .sum::<u64>();

    assert_eq!(union_items, plan.canonical_pairs.len() as u64);
    assert_eq!(rewrite_items, plan.canonical_rewrites.len() as u64);
    assert_eq!(plan.total_items, union_items + rewrite_items);
}


fn read_u32_span(bytes: &[u8], span: CudaEGraphDeviceByteSpan, count: usize) -> Vec<u32> {
    (0..count)
        .map(|index| {
            let offset = span.offset() + (index * 4);
            let mut raw = [0u8; 4];
            raw.copy_from_slice(&bytes[offset..offset + 4]);
            u32::from_le_bytes(raw)
        })
        .collect()
}
