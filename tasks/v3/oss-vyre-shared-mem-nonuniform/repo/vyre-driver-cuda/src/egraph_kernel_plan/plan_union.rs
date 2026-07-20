use crate::backend::ordering::{sort_unstable_by_key_if_needed, sort_unstable_if_needed};
use crate::backend::staging_reserve::reserved_typed_vec;
use rustc_hash::FxHashMap;
use vyre_foundation::optimizer::eqsat_gpu::Equivalence;

use super::helpers::usize_to_u64;
use super::{
    helpers::{ceil_div_u64, wave_count_for},
    CudaEGraphCanonicalRewrite, CudaEGraphCanonicalRewriteDeviceImage,
    CudaEGraphKernelLaunchConfig, CudaEGraphKernelPlanError, CudaEGraphUnionCompactionPass,
    CudaEGraphUnionCompactionPlan, CudaEGraphUnionCompactionWave,
    CUDA_EGRAPH_CANONICAL_REWRITE_RECORD_WORDS,
};

/// Generate the concrete PTX kernel that compares packed e-graph rows inside
/// one signature bucket wave and emits exact e-class equivalences.
///
/// The kernel expects the packed columns produced by
/// [`GpuEGraphDeviceImage`], the bucket table produced by
/// [`pack_cuda_egraph_signature_bucket_device_image`], and the output buffers
/// sized by [`plan_cuda_egraph_structural_equivalence_output`].
pub fn plan_cuda_egraph_union_compaction(
    equivalences: &[Equivalence],
    config: CudaEGraphKernelLaunchConfig,
) -> Result<CudaEGraphUnionCompactionPlan, CudaEGraphKernelPlanError> {
    if config.threads_per_block == 0 {
        return Err(CudaEGraphKernelPlanError::ZeroThreadsPerBlock);
    }
    if config.max_blocks_per_launch == 0 {
        return Err(CudaEGraphKernelPlanError::ZeroMaxBlocksPerLaunch);
    }
    if equivalences.is_empty() {
        return Ok(CudaEGraphUnionCompactionPlan {
            canonical_pairs: Vec::new(),
            affected_eclasses: Vec::new(),
            canonical_rewrites: Vec::new(),
            waves: Vec::new(),
            ignored_self_pair_count: 0,
            duplicate_pair_count: 0,
            total_items: 0,
            total_blocks: 0,
        });
    }

    let mut ignored_self_pair_count = 0_u64;
    let mut canonical_pairs =
        reserved_typed_vec(equivalences.len(), "egraph union canonical pairs")?;
    for pair in equivalences {
        if pair.left == pair.right {
            ignored_self_pair_count = ignored_self_pair_count.checked_add(1).ok_or(
                CudaEGraphKernelPlanError::CountOverflow {
                    field: "ignored self pair count",
                },
            )?;
            continue;
        }
        let (left, right) = if pair.left < pair.right {
            (pair.left, pair.right)
        } else {
            (pair.right, pair.left)
        };
        canonical_pairs.push(Equivalence { left, right });
    }
    let pair_count_before_dedup = canonical_pairs.len();
    sort_unstable_by_key_if_needed(&mut canonical_pairs, |pair| (pair.left, pair.right));
    canonical_pairs.dedup();
    let duplicate_pair_count = pair_count_before_dedup
        .checked_sub(canonical_pairs.len())
        .ok_or(CudaEGraphKernelPlanError::CountOverflow {
            field: "duplicate pair count",
        })? as u64;

    let affected_capacity =
        canonical_pairs
            .len()
            .checked_mul(2)
            .ok_or(CudaEGraphKernelPlanError::CountOverflow {
                field: "affected eclass capacity",
            })?;
    let mut affected_eclasses =
        reserved_typed_vec(affected_capacity, "egraph union affected eclasses")?;
    for pair in &canonical_pairs {
        affected_eclasses.push(pair.left);
        affected_eclasses.push(pair.right);
    }
    sort_unstable_if_needed(&mut affected_eclasses);
    affected_eclasses.dedup();

    let mut parents = reserved_typed_vec(affected_eclasses.len(), "egraph union parents")?;
    for index in 0..affected_eclasses.len() {
        parents.push(index);
    }
    let mut eclass_indices = FxHashMap::<u32, usize>::default();
    eclass_indices
        .try_reserve(affected_eclasses.len())
        .map_err(|error| CudaEGraphKernelPlanError::StorageReserveFailed {
            field: "egraph union eclass index",
            requested: affected_eclasses.len(),
            message: error.to_string(),
        })?;
    for (index, &eclass_id) in affected_eclasses.iter().enumerate() {
        eclass_indices.insert(eclass_id, index);
    }
    for pair in &canonical_pairs {
        let left =
            *eclass_indices
                .get(&pair.left)
                .ok_or(CudaEGraphKernelPlanError::CountOverflow {
                    field: "left eclass lookup",
                })?;
        let right =
            *eclass_indices
                .get(&pair.right)
                .ok_or(CudaEGraphKernelPlanError::CountOverflow {
                    field: "right eclass lookup",
                })?;
        union_min_parent(&mut parents, left, right);
    }

    let mut canonical_rewrites =
        reserved_typed_vec(affected_eclasses.len(), "egraph canonical rewrites")?;
    for index in 0..affected_eclasses.len() {
        let root = find_union_parent(&mut parents, index);
        let representative = affected_eclasses[root];
        let eclass_id = affected_eclasses[index];
        if representative != eclass_id {
            canonical_rewrites.push(CudaEGraphCanonicalRewrite {
                eclass_id,
                representative,
            });
        }
    }

    let union_items = usize_to_u64(canonical_pairs.len(), "canonical union pair count")?;
    let rewrite_items = usize_to_u64(canonical_rewrites.len(), "canonical rewrite count")?;
    let union_wave_count = wave_count_for(union_items, config)?;
    let rewrite_wave_count = wave_count_for(rewrite_items, config)?;
    let wave_count = union_wave_count.checked_add(rewrite_wave_count).ok_or(
        CudaEGraphKernelPlanError::CountOverflow {
            field: "union compaction wave count",
        },
    )?;
    let mut waves = reserved_typed_vec(
        usize::try_from(wave_count).map_err(|_| CudaEGraphKernelPlanError::CountOverflow {
            field: "union compaction wave count usize conversion",
        })?,
        "egraph union compaction waves",
    )?;
    let mut total_items = 0_u64;
    let mut total_blocks = 0_u64;
    append_union_compaction_waves(
        &mut waves,
        &mut total_items,
        &mut total_blocks,
        CudaEGraphUnionCompactionPass::UnionPairs,
        union_items,
        config,
    )?;
    append_union_compaction_waves(
        &mut waves,
        &mut total_items,
        &mut total_blocks,
        CudaEGraphUnionCompactionPass::CanonicalRewrites,
        rewrite_items,
        config,
    )?;

    Ok(CudaEGraphUnionCompactionPlan {
        canonical_pairs,
        affected_eclasses,
        canonical_rewrites,
        waves,
        ignored_self_pair_count,
        duplicate_pair_count,
        total_items,
        total_blocks,
    })
}

/// Pack canonical e-class rewrites into fixed-width device records sorted by
/// source e-class id.
///
/// # Errors
///
/// Returns [`CudaEGraphKernelPlanError`] if packed word-count arithmetic
/// overflows host addressing.
pub fn pack_cuda_egraph_canonical_rewrite_device_image(
    plan: &CudaEGraphUnionCompactionPlan,
) -> Result<CudaEGraphCanonicalRewriteDeviceImage, CudaEGraphKernelPlanError> {
    let word_count = plan
        .canonical_rewrites
        .len()
        .checked_mul(CUDA_EGRAPH_CANONICAL_REWRITE_RECORD_WORDS)
        .ok_or(CudaEGraphKernelPlanError::CountOverflow {
            field: "canonical rewrite word count",
        })?;
    let mut rewrite_words = reserved_typed_vec(word_count, "canonical rewrite words")?;
    for rewrite in &plan.canonical_rewrites {
        rewrite_words.push(rewrite.eclass_id);
        rewrite_words.push(rewrite.representative);
    }
    Ok(CudaEGraphCanonicalRewriteDeviceImage {
        rewrite_words,
        rewrite_count: plan.canonical_rewrites.len(),
        rewrite_record_words: CUDA_EGRAPH_CANONICAL_REWRITE_RECORD_WORDS,
    })
}

/// Generate PTX for applying canonical e-class rewrites directly to a
/// CUDA-resident packed e-graph image.
///
/// The kernel scans `row_eclass_ids || children`, binary-searches the sorted
/// rewrite table, and overwrites matching ids with their canonical
/// representative. This keeps equality-saturation compaction on the GPU after
/// structural-equivalence discovery.
///
/// # Errors
///
/// Returns [`CudaEGraphKernelPlanError::InvalidPtxTarget`] when `target_sm` is
/// zero.
fn append_union_compaction_waves(
    waves: &mut Vec<CudaEGraphUnionCompactionWave>,
    total_items: &mut u64,
    total_blocks: &mut u64,
    pass: CudaEGraphUnionCompactionPass,
    item_count: u64,
    config: CudaEGraphKernelLaunchConfig,
) -> Result<(), CudaEGraphKernelPlanError> {
    let mut remaining = item_count;
    let mut first_item = 0_u64;
    let max_items_per_wave = u64::from(config.threads_per_block)
        .checked_mul(u64::from(config.max_blocks_per_launch))
        .ok_or(CudaEGraphKernelPlanError::CountOverflow {
            field: "union compaction max items per wave",
        })?;
    while remaining > 0 {
        let wave_items = remaining.min(max_items_per_wave);
        let blocks = ceil_div_u64(wave_items, u64::from(config.threads_per_block))?;
        let blocks =
            u32::try_from(blocks).map_err(|_| CudaEGraphKernelPlanError::CountOverflow {
                field: "blocks per union compaction launch wave",
            })?;
        waves.push(CudaEGraphUnionCompactionWave {
            pass,
            first_item,
            item_count: wave_items,
            blocks,
            threads_per_block: config.threads_per_block,
        });
        *total_items = total_items.checked_add(wave_items).ok_or(
            CudaEGraphKernelPlanError::CountOverflow {
                field: "union compaction total items",
            },
        )?;
        *total_blocks = total_blocks.checked_add(u64::from(blocks)).ok_or(
            CudaEGraphKernelPlanError::CountOverflow {
                field: "union compaction total blocks",
            },
        )?;
        first_item =
            first_item
                .checked_add(wave_items)
                .ok_or(CudaEGraphKernelPlanError::CountOverflow {
                    field: "union compaction wave first item",
                })?;
        remaining -= wave_items;
    }
    Ok(())
}

fn find_union_parent(parents: &mut [usize], index: usize) -> usize {
    let parent = parents[index];
    if parent == index {
        return index;
    }
    let root = find_union_parent(parents, parent);
    parents[index] = root;
    root
}

fn union_min_parent(parents: &mut [usize], left: usize, right: usize) {
    let left_root = find_union_parent(parents, left);
    let right_root = find_union_parent(parents, right);
    if left_root == right_root {
        return;
    }
    if left_root < right_root {
        parents[right_root] = left_root;
    } else {
        parents[left_root] = right_root;
    }
}
