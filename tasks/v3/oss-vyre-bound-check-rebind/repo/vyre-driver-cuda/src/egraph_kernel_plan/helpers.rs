use crate::egraph_device_image::CudaEGraphDeviceKernelView;
use crate::numeric::CUDA_NUMERIC;
use vyre_foundation::optimizer::eqsat_gpu::GpuEGraphDeviceImage;

use super::{
    CudaEGraphKernelLaunchConfig, CudaEGraphKernelPass, CudaEGraphKernelPlanError,
    CudaEGraphKernelWave, CudaEGraphSignatureBucketPlan, CudaEGraphSignaturePairWave,
};

/// Decode a signature-bucket pair ordinal to the concrete row ids kernels must
/// compare.
///
/// Pair ordinals enumerate the upper triangle of each bucket in row-major
/// order: `(0, 1), (0, 2), ..., (1, 2), ...`. CUDA kernels can use this same
/// arithmetic to map a thread's pair ordinal to two row ids without materializing
/// all candidate pairs.
///
/// # Errors
///
/// Returns [`CudaEGraphKernelPlanError::SignaturePairOrdinalOutOfBounds`] when
/// `bucket_index` or `pair_ordinal` does not identify a planned candidate pair.
pub fn cuda_egraph_signature_pair_rows(
    plan: &CudaEGraphSignatureBucketPlan,
    bucket_index: u32,
    pair_ordinal: u64,
) -> Result<(u32, u32), CudaEGraphKernelPlanError> {
    let Some(bucket) = plan.buckets.get(bucket_index as usize) else {
        return Err(CudaEGraphKernelPlanError::SignaturePairOrdinalOutOfBounds {
            bucket_index,
            pair_ordinal,
            candidate_pair_count: 0,
        });
    };
    if pair_ordinal >= bucket.candidate_pair_count {
        return Err(CudaEGraphKernelPlanError::SignaturePairOrdinalOutOfBounds {
            bucket_index,
            pair_ordinal,
            candidate_pair_count: bucket.candidate_pair_count,
        });
    }

    let row_count = u64::from(bucket.row_count);
    let mut lo = 0_u64;
    let mut hi = row_count - 1;
    while lo < hi {
        let mid = lo + ((hi - lo) / 2);
        let next_start = signature_pairs_before_row(mid + 1, row_count)?;
        if next_start <= pair_ordinal {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    let local_left = lo;
    let row_pair_base = signature_pairs_before_row(local_left, row_count)?;
    let local_right = local_left
        .checked_add(1)
        .and_then(|value| value.checked_add(pair_ordinal - row_pair_base))
        .ok_or(CudaEGraphKernelPlanError::CountOverflow {
            field: "signature pair local right row",
        })?;
    let base = bucket.first_bucket_row as usize;
    let bucket_end = base.checked_add(bucket.row_count as usize).ok_or(
        CudaEGraphKernelPlanError::CountOverflow {
            field: "signature bucket row range end",
        },
    )?;
    if bucket_end > plan.bucket_rows.len() {
        return Err(CudaEGraphKernelPlanError::SignatureBucketRowsOutOfBounds {
            bucket_index,
            first_bucket_row: base,
            row_count: bucket.row_count as usize,
            bucket_rows_len: plan.bucket_rows.len(),
        });
    }
    let left = plan.bucket_rows[base + local_left as usize];
    let right = plan.bucket_rows[base + local_right as usize];
    Ok((left, right))
}

pub(super) fn validate_image_view_matches(
    image: &GpuEGraphDeviceImage,
    view: CudaEGraphDeviceKernelView,
) -> Result<(), CudaEGraphKernelPlanError> {
    if image.layout().row_count() != view.row_count() {
        return Err(CudaEGraphKernelPlanError::ImageViewMismatch {
            field: "row count",
            image: image.layout().row_count(),
            view: view.row_count(),
        });
    }
    if image.layout().child_count() != view.child_count() {
        return Err(CudaEGraphKernelPlanError::ImageViewMismatch {
            field: "child count",
            image: image.layout().child_count(),
            view: view.child_count(),
        });
    }
    if image.layout().eclass_group_count() != view.eclass_group_count() {
        return Err(CudaEGraphKernelPlanError::ImageViewMismatch {
            field: "eclass group count",
            image: image.layout().eclass_group_count(),
            view: view.eclass_group_count(),
        });
    }
    Ok(())
}

pub(super) fn packed_rows_structurally_equal(
    image: &GpuEGraphDeviceImage,
    left_row: u32,
    right_row: u32,
) -> Result<bool, CudaEGraphKernelPlanError> {
    let left = left_row as usize;
    let right = right_row as usize;
    let row_count = image.layout().row_count();
    if left >= row_count {
        return Err(CudaEGraphKernelPlanError::ImageColumnOutOfBounds {
            column: "rows",
            row: left_row,
            start: left,
            end: left.saturating_add(1),
            len: row_count,
        });
    }
    if right >= row_count {
        return Err(CudaEGraphKernelPlanError::ImageColumnOutOfBounds {
            column: "rows",
            row: right_row,
            start: right,
            end: right.saturating_add(1),
            len: row_count,
        });
    }
    if image.row_signatures()[left] != image.row_signatures()[right] {
        return Ok(false);
    }
    if image.row_language_op_ids()[left] != image.row_language_op_ids()[right] {
        return Ok(false);
    }
    if image.row_children_lens()[left] != image.row_children_lens()[right] {
        return Ok(false);
    }

    let left_children = packed_row_children(image, left_row)?;
    let right_children = packed_row_children(image, right_row)?;
    Ok(left_children == right_children)
}

pub(super) fn packed_row_children(
    image: &GpuEGraphDeviceImage,
    row: u32,
) -> Result<&[u32], CudaEGraphKernelPlanError> {
    let row_index = row as usize;
    let start = image.row_children_offsets()[row_index] as usize;
    let len = image.row_children_lens()[row_index] as usize;
    let end = start
        .checked_add(len)
        .ok_or(CudaEGraphKernelPlanError::CountOverflow {
            field: "packed row child span end",
        })?;
    let children = image.children();
    if end > children.len() {
        return Err(CudaEGraphKernelPlanError::ImageColumnOutOfBounds {
            column: "children",
            row,
            start,
            end,
            len: children.len(),
        });
    }
    Ok(&children[start..end])
}

pub(super) fn append_pass_waves(
    waves: &mut Vec<CudaEGraphKernelWave>,
    total_items: &mut u64,
    total_blocks: &mut u64,
    pass: CudaEGraphKernelPass,
    item_count: u64,
    config: CudaEGraphKernelLaunchConfig,
) -> Result<(), CudaEGraphKernelPlanError> {
    if item_count == 0 {
        return Ok(());
    }
    let items_per_wave = u64::from(config.threads_per_block)
        .checked_mul(u64::from(config.max_blocks_per_launch))
        .ok_or(CudaEGraphKernelPlanError::CountOverflow {
            field: "items per launch wave",
        })?;
    let mut first_item = 0_u64;
    while first_item < item_count {
        let remaining = item_count - first_item;
        let wave_items = remaining.min(items_per_wave);
        let blocks = ceil_div_u64(wave_items, u64::from(config.threads_per_block))?;
        let blocks =
            u32::try_from(blocks).map_err(|_| CudaEGraphKernelPlanError::CountOverflow {
                field: "blocks per launch wave",
            })?;
        waves.push(CudaEGraphKernelWave {
            pass,
            first_item,
            item_count: wave_items,
            blocks,
            threads_per_block: config.threads_per_block,
        });
        *total_items = total_items.checked_add(wave_items).ok_or(
            CudaEGraphKernelPlanError::CountOverflow {
                field: "total logical items",
            },
        )?;
        *total_blocks = total_blocks.checked_add(u64::from(blocks)).ok_or(
            CudaEGraphKernelPlanError::CountOverflow {
                field: "total blocks",
            },
        )?;
        first_item =
            first_item
                .checked_add(wave_items)
                .ok_or(CudaEGraphKernelPlanError::CountOverflow {
                    field: "next wave first item",
                })?;
    }
    Ok(())
}

pub(super) fn append_signature_pair_waves(
    pair_waves: &mut Vec<CudaEGraphSignaturePairWave>,
    total_blocks: &mut u64,
    bucket_index: u32,
    pair_count: u64,
    config: CudaEGraphKernelLaunchConfig,
) -> Result<(), CudaEGraphKernelPlanError> {
    let items_per_wave = u64::from(config.threads_per_block)
        .checked_mul(u64::from(config.max_blocks_per_launch))
        .ok_or(CudaEGraphKernelPlanError::CountOverflow {
            field: "items per signature pair launch wave",
        })?;
    let mut first_pair = 0_u64;
    while first_pair < pair_count {
        let remaining = pair_count - first_pair;
        let wave_pairs = remaining.min(items_per_wave);
        let blocks = ceil_div_u64(wave_pairs, u64::from(config.threads_per_block))?;
        let blocks =
            u32::try_from(blocks).map_err(|_| CudaEGraphKernelPlanError::CountOverflow {
                field: "blocks per signature pair launch wave",
            })?;
        pair_waves.push(CudaEGraphSignaturePairWave {
            bucket_index,
            first_pair,
            pair_count: wave_pairs,
            blocks,
            threads_per_block: config.threads_per_block,
        });
        *total_blocks = total_blocks.checked_add(u64::from(blocks)).ok_or(
            CudaEGraphKernelPlanError::CountOverflow {
                field: "signature pair total blocks",
            },
        )?;
        first_pair =
            first_pair
                .checked_add(wave_pairs)
                .ok_or(CudaEGraphKernelPlanError::CountOverflow {
                    field: "next signature pair first item",
                })?;
    }
    Ok(())
}

pub(super) fn wave_count_for(
    item_count: u64,
    config: CudaEGraphKernelLaunchConfig,
) -> Result<u64, CudaEGraphKernelPlanError> {
    if item_count == 0 {
        return Ok(0);
    }
    let items_per_wave = u64::from(config.threads_per_block)
        .checked_mul(u64::from(config.max_blocks_per_launch))
        .ok_or(CudaEGraphKernelPlanError::CountOverflow {
            field: "items per launch wave",
        })?;
    ceil_div_u64(item_count, items_per_wave)
}

pub(super) fn ceil_div_u64(
    numerator: u64,
    denominator: u64,
) -> Result<u64, CudaEGraphKernelPlanError> {
    if denominator == 0 {
        return Err(CudaEGraphKernelPlanError::CountOverflow {
            field: "ceil division denominator",
        });
    }
    if numerator == 0 {
        return Ok(0);
    }
    numerator
        .checked_add(denominator - 1)
        .ok_or(CudaEGraphKernelPlanError::CountOverflow {
            field: "ceil division numerator",
        })
        .map(|value| value / denominator)
}

pub(super) fn unordered_pair_count(item_count: u64) -> Result<u64, CudaEGraphKernelPlanError> {
    item_count
        .checked_mul(item_count.saturating_sub(1))
        .and_then(|count| count.checked_div(2))
        .ok_or(CudaEGraphKernelPlanError::CountOverflow {
            field: "unordered pair count",
        })
}

pub(super) fn signature_pairs_before_row(
    local_row: u64,
    row_count: u64,
) -> Result<u64, CudaEGraphKernelPlanError> {
    local_row
        .checked_mul(
            row_count
                .checked_mul(2)
                .and_then(|value| value.checked_sub(local_row))
                .and_then(|value| value.checked_sub(1))
                .ok_or(CudaEGraphKernelPlanError::CountOverflow {
                    field: "signature pair row width",
                })?,
        )
        .and_then(|value| value.checked_div(2))
        .ok_or(CudaEGraphKernelPlanError::CountOverflow {
            field: "signature pairs before row",
        })
}

pub(crate) fn usize_to_u64(
    value: usize,
    field: &'static str,
) -> Result<u64, CudaEGraphKernelPlanError> {
    CUDA_NUMERIC
        .usize_to_u64(value, field)
        .map_err(|_| CudaEGraphKernelPlanError::CountOverflow { field })
}
