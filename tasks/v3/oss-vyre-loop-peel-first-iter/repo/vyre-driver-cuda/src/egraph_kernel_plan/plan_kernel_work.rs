use crate::backend::staging_reserve::reserved_typed_vec;
use crate::egraph_device_image::CudaEGraphDeviceKernelView;

use super::{
    helpers::{append_pass_waves, usize_to_u64, wave_count_for},
    CudaEGraphKernelLaunchConfig, CudaEGraphKernelPass, CudaEGraphKernelPlanError,
    CudaEGraphKernelWorkPlan,
};

/// Plan bounded CUDA launch waves for a resident e-graph image.
pub fn plan_cuda_egraph_kernel_work(
    view: CudaEGraphDeviceKernelView,
    config: CudaEGraphKernelLaunchConfig,
) -> Result<CudaEGraphKernelWorkPlan, CudaEGraphKernelPlanError> {
    if config.threads_per_block == 0 {
        return Err(CudaEGraphKernelPlanError::ZeroThreadsPerBlock);
    }
    if config.max_blocks_per_launch == 0 {
        return Err(CudaEGraphKernelPlanError::ZeroMaxBlocksPerLaunch);
    }

    let row_count = usize_to_u64(view.row_count(), "row count")?;
    let child_count = usize_to_u64(view.child_count(), "child count")?;
    let group_count = usize_to_u64(view.eclass_group_count(), "eclass group count")?;
    let row_waves = wave_count_for(row_count, config)?;
    let child_waves = wave_count_for(child_count, config)?;
    let group_waves = wave_count_for(group_count, config)?;
    let wave_count = row_waves
        .checked_add(child_waves)
        .and_then(|count| count.checked_add(group_waves))
        .ok_or(CudaEGraphKernelPlanError::CountOverflow {
            field: "wave count",
        })?;
    let mut waves = reserved_typed_vec(
        usize::try_from(wave_count).map_err(|_| CudaEGraphKernelPlanError::CountOverflow {
            field: "wave count usize conversion",
        })?,
        "egraph kernel waves",
    )?;

    let mut total_items = 0_u64;
    let mut total_blocks = 0_u64;
    append_pass_waves(
        &mut waves,
        &mut total_items,
        &mut total_blocks,
        CudaEGraphKernelPass::RowScan,
        row_count,
        config,
    )?;
    append_pass_waves(
        &mut waves,
        &mut total_items,
        &mut total_blocks,
        CudaEGraphKernelPass::ChildEdgeScan,
        child_count,
        config,
    )?;
    append_pass_waves(
        &mut waves,
        &mut total_items,
        &mut total_blocks,
        CudaEGraphKernelPass::EclassGroupScan,
        group_count,
        config,
    )?;

    Ok(CudaEGraphKernelWorkPlan {
        view,
        waves,
        total_items,
        total_blocks,
    })
}
