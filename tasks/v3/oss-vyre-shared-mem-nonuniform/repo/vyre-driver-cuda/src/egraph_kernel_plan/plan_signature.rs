use crate::backend::ordering::sort_unstable_by_key_if_needed;
use crate::backend::staging_reserve::reserved_typed_vec;
use crate::egraph_device_image::CudaEGraphDeviceKernelView;
use vyre_foundation::optimizer::eqsat_gpu::GpuEGraphDeviceImage;

use super::{
    helpers::{append_signature_pair_waves, unordered_pair_count, wave_count_for},
    CudaEGraphKernelLaunchConfig, CudaEGraphKernelPlanError, CudaEGraphResidentColumnSnapshot,
    CudaEGraphResidentSignatureSnapshot, CudaEGraphSignatureBucket, CudaEGraphSignatureBucketPlan,
};

/// Plan structural-signature candidate buckets for GPU-side e-graph
/// equivalence discovery.
///
/// Row signatures are a prefilter only: kernels must still compare
/// language-op ids, child lengths, and child columns before emitting an
/// equivalence. The value of this plan is that the expensive exact comparison
/// runs only on compact candidate buckets instead of every row pair.
pub fn plan_cuda_egraph_signature_buckets(
    image: &GpuEGraphDeviceImage,
    view: CudaEGraphDeviceKernelView,
    config: CudaEGraphKernelLaunchConfig,
) -> Result<CudaEGraphSignatureBucketPlan, CudaEGraphKernelPlanError> {
    plan_cuda_egraph_signature_buckets_from_column(
        image.row_signatures(),
        image.layout().row_count(),
        image.layout().child_count(),
        image.layout().eclass_group_count(),
        view,
        config,
    )
}

/// Plan structural-signature candidate buckets from a current CUDA-resident
/// column snapshot.
///
/// This is the planning path used after a CUDA kernel mutates resident row or
/// child e-class ids and refreshes the resident signature column. Exact
/// comparison still happens against device memory; this host snapshot only
/// bounds candidate-pair launch work.
pub fn plan_cuda_egraph_signature_buckets_from_resident_snapshot(
    snapshot: &CudaEGraphResidentColumnSnapshot,
    view: CudaEGraphDeviceKernelView,
    config: CudaEGraphKernelLaunchConfig,
) -> Result<CudaEGraphSignatureBucketPlan, CudaEGraphKernelPlanError> {
    plan_cuda_egraph_signature_buckets_from_column(
        &snapshot.row_signatures,
        snapshot.row_count(),
        snapshot.child_count(),
        snapshot.eclass_group_count,
        view,
        config,
    )
}

/// Plan structural-signature candidate buckets from a lightweight
/// CUDA-resident signature snapshot.
pub fn plan_cuda_egraph_signature_buckets_from_signature_snapshot(
    snapshot: &CudaEGraphResidentSignatureSnapshot,
    view: CudaEGraphDeviceKernelView,
    config: CudaEGraphKernelLaunchConfig,
) -> Result<CudaEGraphSignatureBucketPlan, CudaEGraphKernelPlanError> {
    plan_cuda_egraph_signature_buckets_from_column(
        &snapshot.row_signatures,
        snapshot.row_count(),
        snapshot.child_count(),
        snapshot.eclass_group_count,
        view,
        config,
    )
}

fn plan_cuda_egraph_signature_buckets_from_column(
    signatures: &[u32],
    row_count: usize,
    child_count: usize,
    eclass_group_count: usize,
    view: CudaEGraphDeviceKernelView,
    config: CudaEGraphKernelLaunchConfig,
) -> Result<CudaEGraphSignatureBucketPlan, CudaEGraphKernelPlanError> {
    if config.threads_per_block == 0 {
        return Err(CudaEGraphKernelPlanError::ZeroThreadsPerBlock);
    }
    if config.max_blocks_per_launch == 0 {
        return Err(CudaEGraphKernelPlanError::ZeroMaxBlocksPerLaunch);
    }
    if row_count != view.row_count() {
        return Err(CudaEGraphKernelPlanError::ImageViewMismatch {
            field: "row count",
            image: row_count,
            view: view.row_count(),
        });
    }
    if child_count != view.child_count() {
        return Err(CudaEGraphKernelPlanError::ImageViewMismatch {
            field: "child count",
            image: child_count,
            view: view.child_count(),
        });
    }
    if eclass_group_count != view.eclass_group_count() {
        return Err(CudaEGraphKernelPlanError::ImageViewMismatch {
            field: "eclass group count",
            image: eclass_group_count,
            view: view.eclass_group_count(),
        });
    }

    let mut sorted_rows = reserved_typed_vec(signatures.len(), "egraph signature sorted rows")?;
    for row in 0..signatures.len() {
        sorted_rows.push(u32::try_from(row).map_err(|_| {
            CudaEGraphKernelPlanError::CountOverflow {
                field: "signature row index",
            }
        })?);
    }
    sort_unstable_by_key_if_needed(&mut sorted_rows, |&row| (signatures[row as usize], row));

    let mut buckets = reserved_typed_vec(signatures.len(), "egraph signature buckets")?;
    let mut bucket_rows = reserved_typed_vec(signatures.len(), "egraph signature bucket rows")?;
    let mut candidate_pair_count = 0_u64;

    let mut cursor = 0_usize;
    while cursor < sorted_rows.len() {
        let signature = signatures[sorted_rows[cursor] as usize];
        let start = cursor;
        cursor += 1;
        while cursor < sorted_rows.len() && signatures[sorted_rows[cursor] as usize] == signature {
            cursor += 1;
        }

        let row_count = cursor - start;
        if row_count < 2 {
            continue;
        }
        let first_bucket_row = u32::try_from(bucket_rows.len()).map_err(|_| {
            CudaEGraphKernelPlanError::CountOverflow {
                field: "signature bucket row offset",
            }
        })?;
        bucket_rows.extend_from_slice(&sorted_rows[start..cursor]);
        let pair_count = unordered_pair_count(row_count as u64)?;
        candidate_pair_count = candidate_pair_count.checked_add(pair_count).ok_or(
            CudaEGraphKernelPlanError::CountOverflow {
                field: "signature candidate pair count",
            },
        )?;
        buckets.push(CudaEGraphSignatureBucket {
            signature,
            first_bucket_row,
            row_count: u32::try_from(row_count).map_err(|_| {
                CudaEGraphKernelPlanError::CountOverflow {
                    field: "signature bucket row count",
                }
            })?,
            candidate_pair_count: pair_count,
        });
    }

    let pair_wave_count = buckets.iter().try_fold(0_u64, |acc, bucket| {
        wave_count_for(bucket.candidate_pair_count, config).and_then(|count| {
            acc.checked_add(count)
                .ok_or(CudaEGraphKernelPlanError::CountOverflow {
                    field: "signature pair wave count",
                })
        })
    })?;
    let mut pair_waves = reserved_typed_vec(
        usize::try_from(pair_wave_count).map_err(|_| CudaEGraphKernelPlanError::CountOverflow {
            field: "signature pair wave count usize conversion",
        })?,
        "egraph signature pair waves",
    )?;
    let mut total_blocks = 0_u64;
    for (bucket_index, bucket) in buckets.iter().enumerate() {
        append_signature_pair_waves(
            &mut pair_waves,
            &mut total_blocks,
            u32::try_from(bucket_index).map_err(|_| CudaEGraphKernelPlanError::CountOverflow {
                field: "signature bucket index",
            })?,
            bucket.candidate_pair_count,
            config,
        )?;
    }

    Ok(CudaEGraphSignatureBucketPlan {
        view,
        buckets,
        bucket_rows,
        pair_waves,
        candidate_pair_count,
        total_blocks,
    })
}
