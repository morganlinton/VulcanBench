//! Shared flash-attention work partition and benchmark-evidence planner.

/// Absolute output tolerance used by flash-attention parity checks.
pub const FLASH_ATTENTION_OUTPUT_TOLERANCE_ABS: f32 = 1.0e-3;

/// Target number of KV tiles handled by one sequence-parallel split.
pub const FLASH_ATTENTION_SEQUENCE_PARALLEL_TARGET_TILES_PER_SPLIT: u32 = 4;

const SCALAR_ONLINE_WORKGROUP_LANES: u32 = 128;
const COOPERATIVE_TILED_WORKGROUP_LANES: u32 = 64;
const WARP_LANES: u32 = 32;
const F32_BYTES: u64 = core::mem::size_of::<f32>() as u64;

/// Flash-attention kernel family selected by the planner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlashAttentionKernelKind {
    /// One query row per invocation, key rows processed scalar online.
    ScalarOnline,
    /// One query row per invocation, key rows processed in cooperative tiles.
    CooperativeTiled,
}

/// Estimated memory traffic for a flash-attention work plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlashAttentionMemoryTraffic {
    /// Estimated global-memory read bytes.
    pub global_read_bytes: u64,
    /// Estimated global-memory write bytes.
    pub global_write_bytes: u64,
    /// Workgroup/shared-memory footprint bytes.
    pub shared_memory_bytes: u64,
}

/// Benchmark evidence fields required for flash-attention planner rows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlashAttentionBenchMetrics {
    /// Output tolerance used when comparing against the scalar baseline.
    pub output_tolerance_abs: f32,
    /// Estimated memory traffic.
    pub memory_traffic: FlashAttentionMemoryTraffic,
    /// Occupancy proxy in basis points, based on active lanes per workgroup.
    pub occupancy_proxy_bps: u32,
    /// Estimated non-matmul floating-point operations.
    pub non_matmul_flops: u64,
}

/// Shared work partition for flash-attention variants.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlashAttentionWorkPlan {
    /// Kernel family.
    pub kernel: FlashAttentionKernelKind,
    /// Sequence length.
    pub seq_len: u32,
    /// Head dimension.
    pub head_dim: u32,
    /// KV tile size. Scalar online uses `1`.
    pub tile_size: u32,
    /// Number of KV tiles.
    pub tile_count: u32,
    /// Number of sequence-parallel partial rows used for one query row.
    pub sequence_splits: u32,
    /// Maximum KV tiles assigned to one sequence-parallel split.
    pub tiles_per_sequence_split: u32,
    /// Maximum KV rows assigned to one sequence-parallel split.
    pub keys_per_sequence_split: u32,
    /// Number of query rows assigned to one workgroup block.
    pub rows_per_block: u32,
    /// Workgroups launched per query row after sequence splitting.
    pub parallel_workgroups_per_row: u32,
    /// Threads/lanes per workgroup block.
    pub workgroup_lanes: u32,
    /// Logical warp width.
    pub warp_lanes: u32,
    /// Warps assigned to one workgroup block.
    pub warps_per_block: u32,
    /// Logical tensor elements in each q/k/v/out matrix.
    pub logical_elements: u32,
    /// Query scratch elements.
    pub q_scratch_elements: u32,
    /// Score-tile scratch elements.
    pub score_scratch_elements: u32,
    /// Output-accumulator scratch elements.
    pub o_acc_scratch_elements: u32,
    /// Reduction scratch elements for sequence-parallel partial softmax state.
    pub split_reduce_scratch_elements: u32,
    /// Benchmark/evidence metrics for this plan.
    pub bench_metrics: FlashAttentionBenchMetrics,
}

/// Plan the scalar online-softmax flash-attention baseline.
///
/// # Errors
///
/// Returns a fix-directed error when dimensions are empty or overflow.
pub fn plan_flash_attention_scalar(
    seq_len: u32,
    head_dim: u32,
) -> Result<FlashAttentionWorkPlan, String> {
    let logical_elements = validate_attention_dims("flash_attention", seq_len, head_dim)?;
    let q_scratch_elements = checked_mul(
        SCALAR_ONLINE_WORKGROUP_LANES,
        head_dim,
        "flash_attention q/o scratch",
    )?;
    let memory_traffic = scalar_memory_traffic(seq_len, head_dim, q_scratch_elements)?;
    Ok(FlashAttentionWorkPlan {
        kernel: FlashAttentionKernelKind::ScalarOnline,
        seq_len,
        head_dim,
        tile_size: 1,
        tile_count: seq_len,
        sequence_splits: 1,
        tiles_per_sequence_split: seq_len,
        keys_per_sequence_split: seq_len,
        rows_per_block: 1,
        parallel_workgroups_per_row: 1,
        workgroup_lanes: SCALAR_ONLINE_WORKGROUP_LANES,
        warp_lanes: WARP_LANES,
        warps_per_block: SCALAR_ONLINE_WORKGROUP_LANES / WARP_LANES,
        logical_elements,
        q_scratch_elements,
        score_scratch_elements: 0,
        o_acc_scratch_elements: q_scratch_elements,
        split_reduce_scratch_elements: 0,
        bench_metrics: FlashAttentionBenchMetrics {
            output_tolerance_abs: FLASH_ATTENTION_OUTPUT_TOLERANCE_ABS,
            memory_traffic,
            occupancy_proxy_bps: occupancy_proxy_bps(head_dim.max(1), SCALAR_ONLINE_WORKGROUP_LANES),
            non_matmul_flops: scalar_non_matmul_flops(seq_len, head_dim),
        },
    })
}

/// Plan cooperative tiled FlashAttention-2-style execution.
///
/// # Errors
///
/// Returns a fix-directed error when dimensions are empty or overflow.
pub fn plan_flash_attention_tiled(
    seq_len: u32,
    head_dim: u32,
    tile_size: u32,
) -> Result<FlashAttentionWorkPlan, String> {
    let logical_elements = validate_attention_dims("flash_attention_2", seq_len, head_dim)?;
    if tile_size == 0 {
        return Err("Fix: flash_attention_2 tile_size must be > 0".to_string());
    }
    let tile_count = seq_len.div_ceil(tile_size);
    let sequence_splits = sequence_parallel_splits(tile_count);
    let tiles_per_sequence_split = tile_count.div_ceil(sequence_splits);
    let keys_per_sequence_split = tile_size
        .checked_mul(tiles_per_sequence_split)
        .ok_or_else(|| "Fix: flash_attention_2 split key span overflows u32".to_string())?
        .min(seq_len);
    let q_scratch_elements = checked_mul(
        COOPERATIVE_TILED_WORKGROUP_LANES,
        head_dim,
        "flash_attention_2 q_scratch",
    )?;
    let score_scratch_elements = checked_mul(
        COOPERATIVE_TILED_WORKGROUP_LANES,
        tile_size,
        "flash_attention_2 score_scratch",
    )?;
    let o_acc_scratch_elements = checked_mul(
        COOPERATIVE_TILED_WORKGROUP_LANES,
        head_dim,
        "flash_attention_2 o_acc",
    )?;
    let split_reduce_scratch_elements = split_reduce_scratch_elements(sequence_splits, head_dim)?;
    let shared_elements = q_scratch_elements
        .checked_add(score_scratch_elements)
        .and_then(|value| value.checked_add(o_acc_scratch_elements))
        .and_then(|value| value.checked_add(split_reduce_scratch_elements))
        .ok_or_else(|| "Fix: flash_attention_2 shared scratch overflows u32".to_string())?;
    let memory_traffic = tiled_memory_traffic(seq_len, head_dim, shared_elements)?;
    Ok(FlashAttentionWorkPlan {
        kernel: FlashAttentionKernelKind::CooperativeTiled,
        seq_len,
        head_dim,
        tile_size,
        tile_count,
        sequence_splits,
        tiles_per_sequence_split,
        keys_per_sequence_split,
        rows_per_block: 1,
        parallel_workgroups_per_row: sequence_splits,
        workgroup_lanes: COOPERATIVE_TILED_WORKGROUP_LANES,
        warp_lanes: WARP_LANES,
        warps_per_block: COOPERATIVE_TILED_WORKGROUP_LANES / WARP_LANES,
        logical_elements,
        q_scratch_elements,
        score_scratch_elements,
        o_acc_scratch_elements,
        split_reduce_scratch_elements,
        bench_metrics: FlashAttentionBenchMetrics {
            output_tolerance_abs: FLASH_ATTENTION_OUTPUT_TOLERANCE_ABS,
            memory_traffic,
            occupancy_proxy_bps: occupancy_proxy_bps(
                head_dim.max(tile_size),
                COOPERATIVE_TILED_WORKGROUP_LANES,
            ),
            non_matmul_flops: tiled_non_matmul_flops(
                seq_len,
                head_dim,
                tile_count,
                sequence_splits,
            ),
        },
    })
}

fn sequence_parallel_splits(tile_count: u32) -> u32 {
    tile_count
        .div_ceil(FLASH_ATTENTION_SEQUENCE_PARALLEL_TARGET_TILES_PER_SPLIT)
        .max(1)
}

fn split_reduce_scratch_elements(sequence_splits: u32, head_dim: u32) -> Result<u32, String> {
    let softmax_scalars = head_dim
        .checked_add(2)
        .ok_or_else(|| "Fix: flash_attention_2 split reduction state overflows u32".to_string())?;
    checked_mul(
        sequence_splits,
        softmax_scalars,
        "flash_attention_2 split reduction scratch",
    )
}

fn validate_attention_dims(context: &str, seq_len: u32, head_dim: u32) -> Result<u32, String> {
    if seq_len == 0 {
        return Err(format!("{context} seq_len=0 is invalid: empty sequence"));
    }
    if head_dim == 0 {
        return Err(format!("{context} head_dim=0 is invalid: empty head dimension"));
    }
    checked_mul(seq_len, head_dim, context)
}

fn checked_mul(lhs: u32, rhs: u32, context: &str) -> Result<u32, String> {
    lhs.checked_mul(rhs)
        .ok_or_else(|| format!("Fix: {context} dimensions overflow u32: {lhs}*{rhs}."))
}

fn scalar_memory_traffic(
    seq_len: u32,
    head_dim: u32,
    scratch_elements: u32,
) -> Result<FlashAttentionMemoryTraffic, String> {
    let pair_elements = square_times_dim(seq_len, head_dim, "flash_attention scalar traffic")?;
    let output_elements = u64::from(seq_len) * u64::from(head_dim);
    Ok(FlashAttentionMemoryTraffic {
        global_read_bytes: pair_elements.saturating_mul(3).saturating_mul(F32_BYTES),
        global_write_bytes: output_elements.saturating_mul(F32_BYTES),
        shared_memory_bytes: u64::from(scratch_elements).saturating_mul(F32_BYTES),
    })
}

fn tiled_memory_traffic(
    seq_len: u32,
    head_dim: u32,
    shared_elements: u32,
) -> Result<FlashAttentionMemoryTraffic, String> {
    let pair_elements = square_times_dim(seq_len, head_dim, "flash_attention tiled traffic")?;
    let row_elements = u64::from(seq_len) * u64::from(head_dim);
    Ok(FlashAttentionMemoryTraffic {
        global_read_bytes: pair_elements
            .saturating_mul(2)
            .saturating_add(row_elements)
            .saturating_mul(F32_BYTES),
        global_write_bytes: row_elements.saturating_mul(F32_BYTES),
        shared_memory_bytes: u64::from(shared_elements).saturating_mul(F32_BYTES),
    })
}

fn square_times_dim(seq_len: u32, head_dim: u32, context: &str) -> Result<u64, String> {
    let square = u64::from(seq_len)
        .checked_mul(u64::from(seq_len))
        .ok_or_else(|| format!("Fix: {context} seq_len^2 overflows u64."))?;
    square
        .checked_mul(u64::from(head_dim))
        .ok_or_else(|| format!("Fix: {context} seq_len^2*head_dim overflows u64."))
}

fn occupancy_proxy_bps(active_lanes: u32, workgroup_lanes: u32) -> u32 {
    if workgroup_lanes == 0 {
        return 0;
    }
    let active = active_lanes.min(workgroup_lanes).max(1);
    ((u64::from(active) * 10_000) / u64::from(workgroup_lanes)) as u32
}

fn scalar_non_matmul_flops(seq_len: u32, head_dim: u32) -> u64 {
    let pairs = u64::from(seq_len) * u64::from(seq_len);
    let row_values = u64::from(seq_len) * u64::from(head_dim);
    pairs
        .saturating_mul(9)
        .saturating_add(row_values.saturating_mul(4))
}

fn tiled_non_matmul_flops(
    seq_len: u32,
    head_dim: u32,
    tile_count: u32,
    sequence_splits: u32,
) -> u64 {
    let pairs = u64::from(seq_len) * u64::from(seq_len);
    let row_values = u64::from(seq_len) * u64::from(head_dim);
    let tile_updates = u64::from(seq_len) * u64::from(tile_count);
    let split_reduce_values =
        u64::from(seq_len) * u64::from(sequence_splits.saturating_sub(1)) * u64::from(head_dim);
    pairs
        .saturating_mul(4)
        .saturating_add(tile_updates.saturating_mul(8))
        .saturating_add(row_values.saturating_mul(3))
        .saturating_add(split_reduce_values.saturating_mul(3))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiled_plan_reports_lower_global_reads_than_scalar_baseline() {
        let scalar = plan_flash_attention_scalar(128, 64).expect("scalar plan");
        let tiled = plan_flash_attention_tiled(128, 64, 64).expect("tiled plan");

        assert_eq!(scalar.kernel, FlashAttentionKernelKind::ScalarOnline);
        assert_eq!(tiled.kernel, FlashAttentionKernelKind::CooperativeTiled);
        assert_eq!(tiled.workgroup_lanes, 64);
        assert_eq!(tiled.warp_lanes, 32);
        assert_eq!(tiled.warps_per_block, 2);
        assert_eq!(tiled.tile_count, 2);
        assert_eq!(tiled.sequence_splits, 1);
        assert_eq!(tiled.parallel_workgroups_per_row, 1);
        assert!(
            tiled.bench_metrics.memory_traffic.global_read_bytes
                < scalar.bench_metrics.memory_traffic.global_read_bytes
        );
        assert!(tiled.bench_metrics.memory_traffic.shared_memory_bytes > 0);
        assert!(tiled.bench_metrics.occupancy_proxy_bps > 0);
        assert!(tiled.bench_metrics.non_matmul_flops > 0);
        assert_eq!(
            tiled.bench_metrics.output_tolerance_abs,
            FLASH_ATTENTION_OUTPUT_TOLERANCE_ABS
        );
    }

    #[test]
    fn sequence_parallel_tiled_plan_splits_long_rows_and_reports_work() {
        let scalar = plan_flash_attention_scalar(4096, 128).expect("scalar plan");
        let tiled = plan_flash_attention_tiled(4096, 128, 128).expect("tiled plan");

        assert_eq!(tiled.tile_count, 32);
        assert_eq!(tiled.sequence_splits, 8);
        assert_eq!(
            tiled.tiles_per_sequence_split,
            FLASH_ATTENTION_SEQUENCE_PARALLEL_TARGET_TILES_PER_SPLIT
        );
        assert_eq!(tiled.keys_per_sequence_split, 512);
        assert_eq!(tiled.parallel_workgroups_per_row, tiled.sequence_splits);
        assert_eq!(
            tiled.split_reduce_scratch_elements,
            tiled.sequence_splits * (tiled.head_dim + 2)
        );
        assert!(tiled.bench_metrics.memory_traffic.shared_memory_bytes > 0);
        assert!(tiled.bench_metrics.occupancy_proxy_bps > 0);
        assert!(tiled.bench_metrics.non_matmul_flops > 0);
        assert!(
            tiled.bench_metrics.non_matmul_flops < scalar.bench_metrics.non_matmul_flops
        );
    }

    #[test]
    fn shared_planner_feeds_flash_attention_builders() {
        let scalar = plan_flash_attention_scalar(9, 7).expect("scalar plan");
        let scalar_program =
            super::super::flash_attention::flash_attention("q", "k", "v", "out", 9, 7)
                .expect("flash_attention build");
        assert_eq!(scalar_program.workgroup_size()[0], scalar.workgroup_lanes);
        assert!(
            scalar_program
                .buffers()
                .iter()
                .any(|buffer| buffer.name() == "flash_o"
                    && buffer.count() == scalar.o_acc_scratch_elements)
        );

        let tiled = plan_flash_attention_tiled(8, 16, 4).expect("tiled plan");
        let tiled_program =
            super::super::flash_attention_2::flash_attention_2("q", "k", "v", "out", 8, 16, 4);
        assert_eq!(tiled_program.workgroup_size()[0], tiled.workgroup_lanes);
        assert!(
            tiled_program
                .buffers()
                .iter()
                .any(|buffer| buffer.name() == "score_tile"
                    && buffer.count() == tiled.score_scratch_elements)
        );
    }
}
