use crate::backend::ordering::sort_unstable_if_needed;
use crate::backend::staging_reserve::reserved_typed_vec;
use crate::egraph_device_image::CudaEGraphDeviceKernelView;
use vyre_foundation::optimizer::eqsat_gpu::{Equivalence, GpuEGraphDeviceImage};

use super::{
    constants::SIGNATURE_BUCKET_RECORD_WORDS,
    helpers::{
        cuda_egraph_signature_pair_rows, packed_rows_structurally_equal,
        validate_image_view_matches,
    },
    plan_cuda_egraph_signature_buckets, CudaEGraphKernelLaunchConfig, CudaEGraphKernelPlanError,
    CudaEGraphSignatureBucketDeviceImage, CudaEGraphSignatureBucketPlan,
    CudaEGraphStructuralEquivalenceLaunchArtifact, CudaEGraphStructuralEquivalenceOutputPlan,
    CudaEGraphStructuralEquivalencePlan,
};

/// Build signature buckets and emit exact structural e-class equivalences from
/// the packed columns.
///
/// This is the host-side mirror of the CUDA duplicate-discovery kernel:
/// signatures bound the search space, then exact op/arity/child comparison
/// prevents hash-collision false positives before emitting merge candidates.
pub fn plan_cuda_egraph_structural_equivalences(
    image: &GpuEGraphDeviceImage,
    view: CudaEGraphDeviceKernelView,
    config: CudaEGraphKernelLaunchConfig,
) -> Result<CudaEGraphStructuralEquivalencePlan, CudaEGraphKernelPlanError> {
    let signature_plan = plan_cuda_egraph_signature_buckets(image, view, config)?;
    collect_cuda_egraph_structural_equivalences(image, signature_plan)
}

/// Emit exact structural equivalences from an existing signature-bucket plan.
///
/// # Errors
///
/// Returns [`CudaEGraphKernelPlanError`] if the packed image does not match
/// the bucket plan's checked view or if a planned bucket/pair range is invalid.
pub fn collect_cuda_egraph_structural_equivalences(
    image: &GpuEGraphDeviceImage,
    signature_plan: CudaEGraphSignatureBucketPlan,
) -> Result<CudaEGraphStructuralEquivalencePlan, CudaEGraphKernelPlanError> {
    validate_image_view_matches(image, signature_plan.view)?;

    let mut equivalence_keys = reserved_typed_vec(
        signature_plan.buckets.len(),
        "egraph structural equivalences",
    )?;
    let mut exact_pair_count = 0_u64;
    let mut redundant_pair_count = 0_u64;
    let mut rejected_candidate_pair_count = 0_u64;

    for bucket_index in 0..signature_plan.buckets.len() {
        let bucket = &signature_plan.buckets[bucket_index];
        for pair_ordinal in 0..bucket.candidate_pair_count {
            let (left_row, right_row) = cuda_egraph_signature_pair_rows(
                &signature_plan,
                bucket_index as u32,
                pair_ordinal,
            )?;
            if !packed_rows_structurally_equal(image, left_row, right_row)? {
                rejected_candidate_pair_count = rejected_candidate_pair_count
                    .checked_add(1)
                    .ok_or(CudaEGraphKernelPlanError::CountOverflow {
                        field: "rejected structural candidate count",
                    })?;
                continue;
            }
            exact_pair_count = exact_pair_count.checked_add(1).ok_or(
                CudaEGraphKernelPlanError::CountOverflow {
                    field: "exact structural pair count",
                },
            )?;

            let left_eclass = image.row_eclass_ids()[left_row as usize];
            let right_eclass = image.row_eclass_ids()[right_row as usize];
            if left_eclass == right_eclass {
                redundant_pair_count = redundant_pair_count.checked_add(1).ok_or(
                    CudaEGraphKernelPlanError::CountOverflow {
                        field: "redundant structural pair count",
                    },
                )?;
                continue;
            }
            equivalence_keys.push(if left_eclass < right_eclass {
                (left_eclass, right_eclass)
            } else {
                (right_eclass, left_eclass)
            });
        }
    }

    sort_unstable_if_needed(&mut equivalence_keys);
    equivalence_keys.dedup();
    let equivalence_output_words =
        equivalence_keys
            .len()
            .checked_mul(2)
            .ok_or(CudaEGraphKernelPlanError::CountOverflow {
                field: "structural equivalence output words",
            })?;
    let mut equivalences = reserved_typed_vec(
        equivalence_keys.len(),
        "egraph structural equivalence output",
    )?;
    equivalences.extend(
        equivalence_keys
            .into_iter()
            .map(|(left, right)| Equivalence { left, right }),
    );

    Ok(CudaEGraphStructuralEquivalencePlan {
        signature_plan,
        equivalences,
        exact_pair_count,
        redundant_pair_count,
        rejected_candidate_pair_count,
        equivalence_output_words,
    })
}

/// Pack signature-bucket metadata into a fixed-width u32 table for resident
/// CUDA kernels.
///
/// The table is intentionally separate from the foundation e-graph image:
/// foundation owns canonical e-graph columns, while CUDA owns launch-local
/// work partitioning.
pub fn pack_cuda_egraph_signature_bucket_device_image(
    signature_plan: &CudaEGraphSignatureBucketPlan,
) -> Result<CudaEGraphSignatureBucketDeviceImage, CudaEGraphKernelPlanError> {
    let bucket_words = pack_cuda_egraph_signature_bucket_words(signature_plan)?;
    Ok(CudaEGraphSignatureBucketDeviceImage {
        bucket_words,
        bucket_rows: signature_plan.bucket_rows.clone(),
        bucket_count: signature_plan.buckets.len(),
        bucket_record_words: SIGNATURE_BUCKET_RECORD_WORDS,
        candidate_pair_count: signature_plan.candidate_pair_count,
    })
}

/// Pack signature-bucket metadata while consuming the host plan.
///
/// The borrowed packing API is retained for callers that need to inspect the
/// plan after packing. Release execution usually creates the plan only to
/// launch the CUDA kernel, so this consuming variant moves the large
/// `bucket_rows` and `pair_waves` vectors into the launch artifact instead of
/// cloning them.
pub fn plan_cuda_egraph_structural_equivalence_launch_artifact_from_plan(
    signature_plan: CudaEGraphSignatureBucketPlan,
) -> Result<CudaEGraphStructuralEquivalenceLaunchArtifact, CudaEGraphKernelPlanError> {
    let output = plan_cuda_egraph_structural_equivalence_output(&signature_plan)?;
    let bucket_words = pack_cuda_egraph_signature_bucket_words(&signature_plan)?;
    let CudaEGraphSignatureBucketPlan {
        buckets,
        bucket_rows,
        pair_waves,
        candidate_pair_count,
        ..
    } = signature_plan;
    Ok(CudaEGraphStructuralEquivalenceLaunchArtifact {
        bucket_image: CudaEGraphSignatureBucketDeviceImage {
            bucket_words,
            bucket_rows,
            bucket_count: buckets.len(),
            bucket_record_words: SIGNATURE_BUCKET_RECORD_WORDS,
            candidate_pair_count,
        },
        output,
        pair_waves,
    })
}

fn pack_cuda_egraph_signature_bucket_words(
    signature_plan: &CudaEGraphSignatureBucketPlan,
) -> Result<Vec<u32>, CudaEGraphKernelPlanError> {
    let bucket_word_count = signature_plan
        .buckets
        .len()
        .checked_mul(SIGNATURE_BUCKET_RECORD_WORDS)
        .ok_or(CudaEGraphKernelPlanError::CountOverflow {
            field: "signature bucket device word count",
        })?;
    let mut bucket_words =
        reserved_typed_vec(bucket_word_count, "egraph signature bucket device words")?;
    for (bucket_index, bucket) in signature_plan.buckets.iter().enumerate() {
        let start = bucket.first_bucket_row as usize;
        let end = start.checked_add(bucket.row_count as usize).ok_or(
            CudaEGraphKernelPlanError::CountOverflow {
                field: "signature bucket device row range end",
            },
        )?;
        if end > signature_plan.bucket_rows.len() {
            return Err(CudaEGraphKernelPlanError::SignatureBucketRowsOutOfBounds {
                bucket_index: u32::try_from(bucket_index).map_err(|_| {
                    CudaEGraphKernelPlanError::CountOverflow {
                        field: "signature bucket device index",
                    }
                })?,
                first_bucket_row: start,
                row_count: bucket.row_count as usize,
                bucket_rows_len: signature_plan.bucket_rows.len(),
            });
        }
        let pair_bytes = bucket.candidate_pair_count.to_le_bytes();
        bucket_words.extend_from_slice(&[
            bucket.signature,
            bucket.first_bucket_row,
            bucket.row_count,
            u32::from_le_bytes([pair_bytes[0], pair_bytes[1], pair_bytes[2], pair_bytes[3]]),
            u32::from_le_bytes([pair_bytes[4], pair_bytes[5], pair_bytes[6], pair_bytes[7]]),
        ]);
    }
    Ok(bucket_words)
}

/// Plan the worst-case structural-equivalence output buffers for a signature
/// bucket plan.
pub fn plan_cuda_egraph_structural_equivalence_output(
    signature_plan: &CudaEGraphSignatureBucketPlan,
) -> Result<CudaEGraphStructuralEquivalenceOutputPlan, CudaEGraphKernelPlanError> {
    let output_pair_words = usize::try_from(signature_plan.candidate_pair_count)
        .map_err(|_| CudaEGraphKernelPlanError::CountOverflow {
            field: "structural equivalence output pair count usize conversion",
        })?
        .checked_mul(2)
        .ok_or(CudaEGraphKernelPlanError::CountOverflow {
            field: "structural equivalence output pair words",
        })?;
    let output_pair_bytes = output_pair_words
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or(CudaEGraphKernelPlanError::CountOverflow {
            field: "structural equivalence output pair bytes",
        })?;
    let output_counter_words = 2_usize;
    let output_counter_bytes = output_counter_words
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or(CudaEGraphKernelPlanError::CountOverflow {
            field: "structural equivalence output counter bytes",
        })?;
    Ok(CudaEGraphStructuralEquivalenceOutputPlan {
        max_equivalences: signature_plan.candidate_pair_count,
        output_pair_words,
        output_pair_bytes,
        output_counter_words,
        output_counter_bytes,
    })
}

/// Build the resident launch artifact consumed by a structural-equivalence
/// CUDA kernel.
pub fn plan_cuda_egraph_structural_equivalence_launch_artifact(
    signature_plan: &CudaEGraphSignatureBucketPlan,
) -> Result<CudaEGraphStructuralEquivalenceLaunchArtifact, CudaEGraphKernelPlanError> {
    Ok(CudaEGraphStructuralEquivalenceLaunchArtifact {
        bucket_image: pack_cuda_egraph_signature_bucket_device_image(signature_plan)?,
        output: plan_cuda_egraph_structural_equivalence_output(signature_plan)?,
        pair_waves: signature_plan.pair_waves.clone(),
    })
}
