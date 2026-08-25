//! PTX code generation from vyre `Program` IR.

mod descriptor_gate;
use vyre_driver::DispatchConfig;
use vyre_foundation::ir::Program;
use vyre_foundation::optimizer::AdapterCaps;

/// Generate PTX source from a vyre Program.
pub fn program_to_ptx(program: &Program, config: &DispatchConfig) -> Result<String, String> {
    program_to_ptx_for_sm(program, config, 80)
}

/// Generate PTX source for a concrete CUDA SM target.
pub fn program_to_ptx_for_sm(
    program: &Program,
    config: &DispatchConfig,
    target_sm: u32,
) -> Result<String, String> {
    program_to_ptx_for_sm_and_subgroup(program, config, target_sm, 32)
}

/// Generate PTX source for a concrete CUDA SM target and subgroup width.
pub fn program_to_ptx_for_sm_and_subgroup(
    program: &Program,
    config: &DispatchConfig,
    target_sm: u32,
    subgroup_size: u32,
) -> Result<String, String> {
    let _profiler_range = crate::profiler::cuda_profiler_range(crate::profiler::CUDA_CODEGEN_RANGE);
    if target_sm == 0 {
        return Err(
            "CUDA PTX lowering received target sm_0. Fix: probe CUDA compute capability before lowering."
                .to_string(),
        );
    }
    if subgroup_size == 0 || subgroup_size > 32 || !subgroup_size.is_power_of_two() {
        return Err(format!(
            "CUDA PTX lowering received invalid subgroup size {subgroup_size}. Fix: pass the probed CUDA warp size from CudaDeviceCaps."
        ));
    }
    let trace = crate::instrumentation::cuda_stage_trace_enabled();
    let start = std::time::Instant::now();
    if trace {
        tracing::debug!("[cuda-codegen] start target_sm={target_sm} subgroup={subgroup_size}");
    }
    let subgroup_lowered = vyre_foundation::lower::lower_subgroup_reductions(
        program.clone(),
        &AdapterCaps {
            backend: "cuda",
            supports_subgroup_ops: true,
            subgroup_size,
            ..AdapterCaps::conservative()
        },
    );
    let descriptor = descriptor_gate::validate_and_analyze(&subgroup_lowered, target_sm)?;
    if trace {
        tracing::debug!(
            "[cuda-codegen] +{}ms descriptor ops={} bindings={}",
            start.elapsed().as_millis(),
            descriptor.body.ops.len(),
            descriptor.bindings.slots.len()
        );
    }
    let ptx = vyre_emit_ptx::emit_with_options(
        &descriptor,
        vyre_emit_ptx::PtxEmitOptions {
            target: descriptor_gate::compute_capability(target_sm),
            subgroup_size,
            ulp_budget: config.ulp_budget.map(u32::from),
            // CUDA lowers GridSync to a native cooperative grid barrier. This is
            // safe because every grid-sync program CUDA emits PTX for is launched
            // cooperatively: the dispatch path forces `cuLaunchCooperativeKernel`
            // and zeroes the barrier counter per launch, while the AOT
            // compile_native path rejects grid-sync before reaching codegen.
            cooperative_grid_sync: true,
        },
    )
    .map_err(|error| {
        format!(
            "CUDA descriptor PTX emission failed: {error}. Fix: add the missing PTX lowering in vyre-emit-ptx rather than reintroducing driver-local Program emission."
        )
    })?;
    if trace {
        tracing::debug!(
            "[cuda-codegen] +{}ms emit bytes={}",
            start.elapsed().as_millis(),
            ptx.len()
        );
    }
    Ok(ptx)
}
