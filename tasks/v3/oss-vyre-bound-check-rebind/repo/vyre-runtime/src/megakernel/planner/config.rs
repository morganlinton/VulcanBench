//! Megakernel launch configuration and policy request construction.

use std::time::Duration;

use vyre_driver::backend::BackendError;

use super::super::policy::{
    MegakernelLaunchPolicy, MegakernelLaunchRecommendation, MegakernelLaunchRequest,
};
use super::super::task::{TaskQueueSnapshot, TaskWorkItem};
use super::geometry::dispatch_grid_for;
use super::sizing::MegakernelSizingPolicy;

/// Optional scale signals that let the megakernel launch policy choose sparse,
/// dense, hybrid, fused, or memory-constrained execution from real workload
/// shape instead of queue length alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MegakernelWorkloadHints {
    /// Count of opcodes observed hot enough for promotion consideration.
    pub hot_opcode_count: u32,
    /// Count of ticketed route windows observed hot enough for promotion.
    pub hot_window_count: u32,
    /// Resident dependency-graph node count.
    pub graph_node_count: u32,
    /// Resident dependency-graph edge count.
    pub graph_edge_count: u32,
    /// Active frontier density in basis points. Zero means infer when possible.
    pub frontier_density_bps: u16,
    /// Device memory pressure in basis points. Zero means infer when possible.
    pub memory_pressure_bps: u16,
    /// Device-resident bytes already required by this dispatch family.
    pub resident_device_bytes: u64,
    /// Hard device-memory budget. Zero means unbounded.
    pub device_memory_budget_bytes: u64,
}

/// Configuration for one megakernel dispatch invocation.
#[derive(Debug, Clone)]
pub struct MegakernelConfig {
    /// Number of persistent worker workgroups.
    pub worker_count: u32,
    /// Maximum wall-clock time the megakernel runs before draining
    /// queued work and exiting.
    pub max_wall_time: Duration,
    /// Hint to the scheduler about expected items per worker.
    pub expected_items_per_worker: u32,
    /// Optional workload-shape hints consumed by the launch policy.
    pub workload: MegakernelWorkloadHints,
}

impl Default for MegakernelConfig {
    fn default() -> Self {
        Self {
            worker_count: MegakernelSizingPolicy::standard().default_worker_count(),
            max_wall_time: Duration::from_secs(60),
            expected_items_per_worker: 0,
            workload: MegakernelWorkloadHints::default(),
        }
    }
}

impl MegakernelConfig {
    /// Validate the config and surface actionable errors.
    ///
    /// # Errors
    ///
    /// Returns an error when the worker count is zero or the wall-clock budget
    /// is empty, because either condition would make persistent dispatch
    /// unschedulable.
    pub fn validate(&self) -> Result<(), BackendError> {
        if self.worker_count == 0 {
            return Err(BackendError::new(
                "megakernel worker_count must be non-zero. Fix: provide at least one worker workgroup.",
            ));
        }
        if self.max_wall_time.is_zero() {
            return Err(BackendError::new(
                "megakernel max_wall_time must be non-zero. Fix: supply a positive Duration budget.",
            ));
        }
        Ok(())
    }

    /// Compute the direct-dispatch grid for `queue_len` logical work slots.
    ///
    /// `worker_count` is the caller's persistent worker-workgroup ceiling; the
    /// returned grid never launches more workgroups than that ceiling or the
    /// backend occupancy cap.
    #[must_use]
    pub fn dispatch_grid(&self, queue_len: u32, max_workgroup_size_x: u32) -> [u32; 3] {
        dispatch_grid_for(self.worker_count, queue_len, max_workgroup_size_x)
    }

    /// Build a policy request from this config and adapter limits.
    #[must_use]
    pub const fn launch_request(
        &self,
        queue_len: u32,
        max_workgroup_size_x: u32,
        max_compute_workgroups_per_dimension: u32,
        max_compute_invocations_per_workgroup: u32,
    ) -> MegakernelLaunchRequest {
        MegakernelLaunchRequest {
            queue_len,
            requested_worker_groups: self.worker_count,
            max_workgroup_size_x,
            max_compute_workgroups_per_dimension,
            max_compute_invocations_per_workgroup,
            requested_hit_capacity: 0,
            expected_hits_per_item: if self.expected_items_per_worker > 1 {
                self.expected_items_per_worker
            } else {
                1
            },
            hot_opcode_count: self.workload.hot_opcode_count,
            hot_window_count: self.workload.hot_window_count,
            requeue_count: 0,
            max_priority_age: 0,
            graph_node_count: self.workload.graph_node_count,
            graph_edge_count: self.workload.graph_edge_count,
            frontier_density_bps: self.workload.frontier_density_bps,
            memory_pressure_bps: self.workload.memory_pressure_bps,
            resident_device_bytes: self.workload.resident_device_bytes,
            device_memory_budget_bytes: self.workload.device_memory_budget_bytes,
        }
    }

    /// Build a policy request from device-visible continuation task slots.
    ///
    /// Paused, completed, empty, running, and faulted tasks do not add launch
    /// lanes. Yielded and requeued tasks stay schedulable so the GPU can resume
    /// them without a CPU-side republish loop.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when a task slot contains an invalid state word.
    pub fn launch_request_for_tasks(
        &self,
        tasks: &[TaskWorkItem],
        max_workgroup_size_x: u32,
        max_compute_workgroups_per_dimension: u32,
        max_compute_invocations_per_workgroup: u32,
    ) -> Result<MegakernelLaunchRequest, BackendError> {
        let snapshot = TaskQueueSnapshot::from_tasks(tasks)?;
        let schedulable_count = snapshot.try_schedulable_count()?;
        let request = self.launch_request(
            schedulable_count,
            max_workgroup_size_x,
            max_compute_workgroups_per_dimension,
            max_compute_invocations_per_workgroup,
        );
        snapshot.try_apply_to_launch_request(request)
    }

    /// Recommend one launch shape through the shared megakernel policy.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when adapter limits are malformed.
    pub fn launch_recommendation(
        &self,
        queue_len: u32,
        max_workgroup_size_x: u32,
        max_compute_workgroups_per_dimension: u32,
        max_compute_invocations_per_workgroup: u32,
    ) -> Result<MegakernelLaunchRecommendation, BackendError> {
        MegakernelLaunchPolicy::standard().recommend(self.launch_request(
            queue_len,
            max_workgroup_size_x,
            max_compute_workgroups_per_dimension,
            max_compute_invocations_per_workgroup,
        ))
    }

    /// Recommend one launch shape for a continuation task queue.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when adapter limits are malformed or any task
    /// slot contains an invalid state word.
    pub fn launch_recommendation_for_tasks(
        &self,
        tasks: &[TaskWorkItem],
        max_workgroup_size_x: u32,
        max_compute_workgroups_per_dimension: u32,
        max_compute_invocations_per_workgroup: u32,
    ) -> Result<MegakernelLaunchRecommendation, BackendError> {
        MegakernelLaunchPolicy::standard().recommend(self.launch_request_for_tasks(
            tasks,
            max_workgroup_size_x,
            max_compute_workgroups_per_dimension,
            max_compute_invocations_per_workgroup,
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_request_preserves_workload_hints() {
        let config = MegakernelConfig {
            workload: MegakernelWorkloadHints {
                hot_opcode_count: 7,
                hot_window_count: 11,
                graph_node_count: 1_000,
                graph_edge_count: 4_000,
                frontier_density_bps: 7_500,
                memory_pressure_bps: 8_000,
                resident_device_bytes: 1 << 20,
                device_memory_budget_bytes: 1 << 24,
            },
            ..MegakernelConfig::default()
        };

        let request = config.launch_request(128, 256, 65_535, 1_024);

        assert_eq!(request.hot_opcode_count, 7);
        assert_eq!(request.hot_window_count, 11);
        assert_eq!(request.graph_node_count, 1_000);
        assert_eq!(request.graph_edge_count, 4_000);
        assert_eq!(request.frontier_density_bps, 7_500);
        assert_eq!(request.memory_pressure_bps, 8_000);
        assert_eq!(request.resident_device_bytes, 1 << 20);
        assert_eq!(request.device_memory_budget_bytes, 1 << 24);
    }
}
