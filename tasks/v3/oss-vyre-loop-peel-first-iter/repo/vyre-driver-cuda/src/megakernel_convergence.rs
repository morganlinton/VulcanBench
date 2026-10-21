//! CUDA megakernel convergence planning adapter.

use vyre_driver::device_convergence::{
    plan_device_convergence, ConvergenceReadbackPolicy, DeviceConvergencePlan,
    DeviceConvergencePlanError,
};

/// CUDA device-side convergence readback policy.
pub type CudaConvergenceReadbackPolicy = ConvergenceReadbackPolicy;

/// Execution plan for CUDA-side fixed-point convergence.
pub type CudaDeviceConvergencePlan = DeviceConvergencePlan;

/// Errors produced while planning CUDA convergence.
pub type CudaDeviceConvergencePlanError = DeviceConvergencePlanError;

/// Plan convergence detection for an iterative CUDA dataflow kernel.
pub fn plan_cuda_device_convergence(
    max_device_iterations: u32,
    changed_flag_bytes: u32,
    requested_host_iteration_polls: u32,
) -> Result<CudaDeviceConvergencePlan, CudaDeviceConvergencePlanError> {
    plan_device_convergence(
        max_device_iterations,
        changed_flag_bytes,
        requested_host_iteration_polls,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuda_convergence_planner_is_adapter_not_policy_fork() {
        let source = include_str!("megakernel_convergence.rs");
        assert!(source.contains("pub type CudaDeviceConvergencePlan = DeviceConvergencePlan;"));
        assert!(source.contains("plan_device_convergence("));
        assert!(!source.contains(concat!("host_sync_points", ": 1")));
        assert!(!source.contains(concat!("changed_flag_bytes", " != 4")));
        assert!(!source.contains(concat!("requested_host_iteration_polls", " != 0")));
    }

    #[test]
    fn convergence_plan_reads_final_flag_once() {
        let plan = plan_cuda_device_convergence(128, 4, 0).expect("Fix: valid plan should build");

        assert_eq!(plan.max_device_iterations, 128);
        assert_eq!(plan.host_sync_points, 1);
        assert_eq!(plan.changed_flag_readback_bytes, 4);
        assert_eq!(plan.host_iteration_polls, 0);
        assert_eq!(
            plan.readback_policy,
            CudaConvergenceReadbackPolicy::FinalFlagOnly
        );
    }

    #[test]
    fn convergence_plan_rejects_host_side_contract_violations() {
        assert_eq!(
            plan_cuda_device_convergence(0, 4, 0).expect_err("zero iterations cannot converge"),
            CudaDeviceConvergencePlanError::EmptyIterationBudget
        );
        assert_eq!(
            plan_cuda_device_convergence(8, 1, 0).expect_err("changed flag must be a u32"),
            CudaDeviceConvergencePlanError::InvalidChangedFlagWidth { bytes: 1 }
        );
        assert_eq!(
            plan_cuda_device_convergence(8, 4, 8)
                .expect_err("host polling every iteration is forbidden"),
            CudaDeviceConvergencePlanError::HostPolledConvergence { polls: 8 }
        );
    }

    #[test]
    fn cuda_convergence_generated_iteration_budgets_preserve_shared_contract() {
        for max_device_iterations in 1..=1_024 {
            let plan = plan_cuda_device_convergence(max_device_iterations, 4, 0)
                .expect("Fix: generated CUDA iteration budgets should plan");
            assert_eq!(plan.max_device_iterations, max_device_iterations);
            assert_eq!(plan.host_sync_points, 1);
            assert_eq!(plan.changed_flag_readback_bytes, 4);
            assert_eq!(plan.host_iteration_polls, 0);
            assert_eq!(
                plan.readback_policy,
                CudaConvergenceReadbackPolicy::FinalFlagOnly
            );
        }
    }
}
