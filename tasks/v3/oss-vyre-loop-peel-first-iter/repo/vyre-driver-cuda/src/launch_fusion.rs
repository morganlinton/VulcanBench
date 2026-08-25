//! CUDA-facing adjacent-stage launch fusion adapter.
//!
//! The adjacent-stage fusion algorithm is backend-neutral. This module
//! preserves the CUDA public API names while delegating planning to
//! `vyre-driver`.

use vyre_driver::launch_fusion::{
    plan_launch_fusion, plan_launch_fusion_with_scratch, LaunchFusionError, LaunchFusionGroup,
    LaunchFusionPlan, LaunchFusionScratch, LaunchFusionStage,
};

/// One adjacent CUDA stage considered for launch fusion.
pub type CudaFusionStage = LaunchFusionStage;

/// One fused adjacent-stage CUDA launch group.
pub type CudaLaunchFusionGroup = LaunchFusionGroup;

/// Complete CUDA launch fusion plan.
pub type CudaLaunchFusionPlan = LaunchFusionPlan;

/// Caller-owned scratch for repeated CUDA launch-fusion planning.
pub type CudaLaunchFusionScratch = LaunchFusionScratch;

/// CUDA launch fusion planning errors.
pub type CudaLaunchFusionError = LaunchFusionError;

/// Plan adjacent CUDA launch fusion under layout and memory constraints.
pub fn plan_cuda_launch_fusion(
    stages: &[CudaFusionStage],
    max_group_bytes: u64,
) -> Result<CudaLaunchFusionPlan, CudaLaunchFusionError> {
    plan_launch_fusion(stages, max_group_bytes)
}

/// Plan adjacent CUDA launch fusion using caller-owned temporary storage.
pub fn plan_cuda_launch_fusion_with_scratch(
    stages: &[CudaFusionStage],
    max_group_bytes: u64,
    scratch: &mut CudaLaunchFusionScratch,
) -> Result<CudaLaunchFusionPlan, CudaLaunchFusionError> {
    plan_launch_fusion_with_scratch(stages, max_group_bytes, scratch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuda_launch_fusion_is_adapter_not_algorithm_fork() {
        let production = include_str!("launch_fusion.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: CUDA launch fusion production source must precede tests.");

        assert!(
            production.contains("vyre_driver::launch_fusion"),
            "Fix: CUDA launch fusion must delegate to the backend-neutral driver owner."
        );
        for forbidden in [
            "FxHashSet",
            "CudaStorageReserveFailure",
            "CudaArithmeticOverflow",
            "fn singleton_group_with_capacity",
            "fn can_append_to_group",
            "fn fused_required_bytes",
            "fn stage_required_bytes",
            "reserved_typed_vec",
            "reserve_typed_hash_set",
        ] {
            assert!(
                !production.contains(forbidden),
                "Fix: CUDA launch fusion must not carry local adjacent-stage fusion logic: {forbidden}."
            );
        }
    }

    #[test]
    fn launch_fusion_groups_adjacent_compatible_stages() {
        let plan = plan_cuda_launch_fusion(
            &[
                stage(1, 7, 64, 32, 8, false),
                stage(2, 7, 32, 48, 8, false),
                stage(3, 7, 48, 16, 8, false),
            ],
            256,
        )
        .expect("Fix: compatible stages should fuse");

        assert_eq!(plan.launch_count, 1);
        assert_eq!(plan.avoided_launches, 2);
        assert_eq!(plan.groups[0].stage_ids, vec![1, 2, 3]);
        assert_eq!(plan.avoided_intermediate_bytes, 80);
    }

    #[test]
    fn launch_fusion_splits_on_layout_host_boundary_and_budget() {
        let plan = plan_cuda_launch_fusion(
            &[
                stage(1, 7, 64, 32, 8, false),
                stage(2, 8, 32, 48, 8, false),
                stage(3, 8, 48, 16, 8, true),
                stage(4, 9, 16, 16, 8, false),
            ],
            128,
        )
        .expect("Fix: incompatible stages should split deterministically");

        assert_eq!(plan.launch_count, 4);
        assert_eq!(plan.avoided_launches, 0);
        assert_eq!(plan.groups[0].stage_ids, vec![1]);
        assert_eq!(plan.groups[1].stage_ids, vec![2]);
        assert_eq!(plan.groups[2].stage_ids, vec![3]);
        assert_eq!(plan.groups[3].stage_ids, vec![4]);
    }

    #[test]
    fn launch_fusion_rejects_invalid_inputs() {
        assert_eq!(
            plan_cuda_launch_fusion(&[stage(1, 7, 1, 1, 1, false)], 0)
                .expect_err("zero budget should fail"),
            CudaLaunchFusionError::ZeroBudget
        );
        assert_eq!(
            plan_cuda_launch_fusion(
                &[stage(1, 7, 1, 1, 1, false), stage(1, 7, 1, 1, 1, false),],
                128,
            )
            .expect_err("duplicate stages should fail"),
            CudaLaunchFusionError::DuplicateStage { id: 1 }
        );
        assert_eq!(
            plan_cuda_launch_fusion(&[stage(9, 7, 64, 32, 64, false)], 128)
                .expect_err("single over-budget stage should fail"),
            CudaLaunchFusionError::StageOverBudget {
                id: 9,
                required_bytes: 160,
                budget_bytes: 128,
            }
        );
    }

    #[test]
    fn launch_fusion_reuses_caller_owned_duplicate_detection_scratch() {
        let mut scratch = CudaLaunchFusionScratch::try_with_capacity(64)
            .expect("Fix: fusion scratch should reserve");
        let wide = (0..64)
            .map(|id| stage(id, 7, 16, 16, 4, false))
            .collect::<Vec<_>>();
        let first = plan_cuda_launch_fusion_with_scratch(&wide, 8_192, &mut scratch)
            .expect("Fix: wide compatible CUDA stages should fuse");
        let id_capacity = scratch.id_capacity();

        assert_eq!(first.launch_count, 1);
        assert_eq!(first.avoided_launches, 63);

        let second = plan_cuda_launch_fusion_with_scratch(
            &[
                stage(10, 7, 64, 32, 8, false),
                stage(11, 8, 32, 48, 8, false),
            ],
            512,
            &mut scratch,
        )
        .expect("Fix: smaller incompatible CUDA stages should reuse duplicate-detection scratch");

        assert_eq!(second.launch_count, 2);
        assert!(scratch.id_capacity() >= id_capacity);
    }

    fn stage(
        id: u32,
        layout_hash: u64,
        input_bytes: u64,
        output_bytes: u64,
        scratch_bytes: u64,
        requires_host_materialization: bool,
    ) -> CudaFusionStage {
        CudaFusionStage {
            id,
            layout_hash,
            input_bytes,
            output_bytes,
            scratch_bytes,
            requires_host_materialization,
        }
    }
}
