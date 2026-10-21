//! CUDA adapter for backend-neutral device-side work queue planning.

use vyre_driver::device_work_queue::{
    plan_device_work_queue, plan_device_work_queue_backpressure,
    plan_device_work_queue_with_expansion, DeviceWorkQueueBackpressurePlan,
    DeviceWorkQueueDrainStrategy, DeviceWorkQueueError, DeviceWorkQueueExpansionProfile,
    DeviceWorkQueuePlan, DeviceWorkQueueProfile, WorkQueueHostSync,
};

/// Host synchronization policy for a CUDA device-side work queue.
pub type CudaWorkQueueHostSync = WorkQueueHostSync;
/// Work queue workload profile.
pub type CudaDeviceWorkQueueProfile = DeviceWorkQueueProfile;
/// Work queue workload profile with resident device-side expansion headroom.
pub type CudaDeviceWorkQueueExpansionProfile = DeviceWorkQueueExpansionProfile;
/// Device-side work queue execution plan.
pub type CudaDeviceWorkQueuePlan = DeviceWorkQueuePlan;
/// Device-side work queue drain strategy.
pub type CudaDeviceWorkQueueDrainStrategy = DeviceWorkQueueDrainStrategy;
/// Device-side work queue plan with bounded resident drain windows.
pub type CudaDeviceWorkQueueBackpressurePlan = DeviceWorkQueueBackpressurePlan;
/// Device work queue planning errors.
pub type CudaDeviceWorkQueueError = DeviceWorkQueueError;

/// Plan a CUDA-resident work queue for dependent dataflow execution.
pub fn plan_cuda_device_work_queue(
    profile: CudaDeviceWorkQueueProfile,
) -> Result<CudaDeviceWorkQueuePlan, CudaDeviceWorkQueueError> {
    plan_device_work_queue(profile)
}

/// Plan a CUDA-resident work queue with budgeted device-side expansion room.
pub fn plan_cuda_device_work_queue_with_expansion(
    profile: CudaDeviceWorkQueueExpansionProfile,
) -> Result<CudaDeviceWorkQueuePlan, CudaDeviceWorkQueueError> {
    plan_device_work_queue_with_expansion(profile)
}

/// Plan a CUDA-resident work queue plus bounded device-side drain windows.
pub fn plan_cuda_device_work_queue_backpressure(
    profile: CudaDeviceWorkQueueProfile,
    max_items_per_drain_launch: u64,
) -> Result<CudaDeviceWorkQueueBackpressurePlan, CudaDeviceWorkQueueError> {
    plan_device_work_queue_backpressure(profile, max_items_per_drain_launch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuda_device_work_queue_is_adapter_not_policy_fork() {
        let source = include_str!("device_work_queue.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: CUDA device work-queue adapter source must contain production section");

        assert!(source.contains("use vyre_driver::device_work_queue::{"));
        assert!(source.contains("pub type CudaDeviceWorkQueueProfile = DeviceWorkQueueProfile;"));
        assert!(source.contains(
            "pub type CudaDeviceWorkQueueExpansionProfile = DeviceWorkQueueExpansionProfile;"
        ));
        assert!(source.contains("plan_device_work_queue(profile)"));
        assert!(source.contains("plan_device_work_queue_with_expansion(profile)"));
        assert!(source
            .contains("plan_device_work_queue_backpressure(profile, max_items_per_drain_launch)"));
        assert!(!production.contains("CudaArithmeticOverflow"));
        assert!(!production.contains("checked_add_u64_count"));
        assert!(!production.contains("checked_mul_u64_count"));
        assert!(!production.contains("CUDA_NUMERIC"));
        assert!(!production.contains("fn div_ceil_u64"));
    }

    #[test]
    fn cuda_device_work_queue_adapter_preserves_final_only_contract() {
        let plan = plan_cuda_device_work_queue(CudaDeviceWorkQueueProfile {
            initial_items: 256,
            queue_capacity: 1_024,
            entry_bytes: 16,
            control_bytes: 128,
            budget_bytes: 32_768,
            host_sync: CudaWorkQueueHostSync::FinalOnly,
        })
        .expect("Fix: valid CUDA device work queue should plan through shared owner");

        assert_eq!(plan.queue_bytes, 16_384);
        assert_eq!(plan.control_bytes, 128);
        assert_eq!(plan.resident_bytes, 16_512);
        assert_eq!(plan.initial_occupancy_bps, 2_500);
        assert!(plan.final_only_host_sync);
    }

    #[test]
    fn cuda_device_work_queue_adapter_preserves_expansion_headroom_contract() {
        let plan =
            plan_cuda_device_work_queue_with_expansion(CudaDeviceWorkQueueExpansionProfile {
                initial_items: 4,
                expansion_items: 12,
                entry_bytes: 8,
                control_bytes: 64,
                budget_bytes: 256,
                host_sync: CudaWorkQueueHostSync::FinalOnly,
            })
            .expect("Fix: valid CUDA expansion work queue should plan through shared owner");

        assert_eq!(plan.queue_bytes, 128);
        assert_eq!(plan.resident_bytes, 192);
        assert_eq!(plan.initial_occupancy_bps, 2_500);
        assert!(plan.final_only_host_sync);
    }

    #[test]
    fn cuda_device_work_queue_adapter_preserves_backpressure_contract() {
        let plan = plan_cuda_device_work_queue_backpressure(
            CudaDeviceWorkQueueProfile {
                initial_items: 4_096,
                queue_capacity: 65_536,
                entry_bytes: 16,
                control_bytes: 128,
                budget_bytes: 2 << 20,
                host_sync: CudaWorkQueueHostSync::FinalOnly,
            },
            8_192,
        )
        .expect("Fix: CUDA device work queue backpressure should plan through shared owner");

        assert_eq!(
            plan.strategy,
            CudaDeviceWorkQueueDrainStrategy::ChunkedResidentDrain
        );
        assert_eq!(plan.items_per_chunk, 8_192);
        assert_eq!(plan.chunks, 8);
        assert!(plan.final_only_host_sync);
    }
}
