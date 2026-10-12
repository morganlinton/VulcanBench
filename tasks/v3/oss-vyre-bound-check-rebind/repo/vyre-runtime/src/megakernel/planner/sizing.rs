use vyre_driver::backend::BackendError;
use vyre_foundation::execution_plan::SchedulingPolicy;

use super::{
    MegakernelGridLimits, MegakernelGridPlan, MegakernelGridRequest, MegakernelLaunchGeometry,
};

/// Shared worker-grid sizing policy for megakernel dispatch.
///
/// This is the host-side policy surface for persistent worker counts,
/// workgroup width, slot padding, and backend grid geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MegakernelSizingPolicy {
    scheduling: SchedulingPolicy,
}

impl Default for MegakernelSizingPolicy {
    fn default() -> Self {
        Self::standard()
    }
}

impl MegakernelSizingPolicy {
    /// Standard megakernel sizing policy used by built-in dispatch paths.
    #[must_use]
    pub const fn standard() -> Self {
        Self {
            scheduling: SchedulingPolicy::standard(),
        }
    }

    /// Build from a shared backend-neutral scheduling policy.
    #[must_use]
    pub const fn from_scheduling(scheduling: SchedulingPolicy) -> Self {
        Self { scheduling }
    }

    /// Default persistent worker workgroup count.
    #[must_use]
    pub const fn default_worker_count(&self) -> u32 {
        self.scheduling.default_worker_count()
    }

    /// Clamp a requested worker count into the legal workgroup x dimension.
    #[must_use]
    pub const fn worker_workgroup_size(&self, worker_count: u32, max_workgroup_size_x: u32) -> u32 {
        self.scheduling
            .worker_workgroup_size(worker_count, max_workgroup_size_x)
    }

    /// Round a logical slot count up to a whole worker workgroup.
    #[must_use]
    pub const fn padded_slot_count(&self, slot_count: u32, workgroup_size_x: u32) -> u32 {
        self.scheduling
            .padded_slot_count(slot_count, workgroup_size_x)
    }

    /// Compute the backend dispatch grid for a logical queue length.
    #[must_use]
    pub const fn dispatch_grid_for(
        &self,
        worker_count: u32,
        queue_len: u32,
        max_workgroup_size_x: u32,
    ) -> [u32; 3] {
        self.scheduling
            .dispatch_grid_for(worker_count, queue_len, max_workgroup_size_x)
    }

    /// Compute a persistent-worker ceiling from adapter limits.
    #[must_use]
    pub const fn default_worker_groups_from_limits(
        &self,
        max_compute_workgroups_per_dimension: u32,
        max_compute_invocations_per_workgroup: u32,
    ) -> u32 {
        self.scheduling.default_worker_groups_from_limits(
            max_compute_workgroups_per_dimension,
            max_compute_invocations_per_workgroup,
        )
    }

    /// Resolve worker groups, workgroup width, slot padding, and dispatch grid.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when adapter limits are malformed.
    pub fn calculate_optimal_grid(
        &self,
        request: MegakernelGridRequest,
        limits: MegakernelGridLimits,
    ) -> Result<MegakernelGridPlan, BackendError> {
        limits.validate()?;

        let occupancy_worker_groups = self
            .default_worker_groups_from_limits(
                limits.max_compute_workgroups_per_dimension,
                limits.max_compute_invocations_per_workgroup,
            )
            .min(limits.max_compute_workgroups_per_dimension);

        let worker_groups = if request.requested_worker_groups == 0 {
            occupancy_worker_groups
        } else {
            request
                .requested_worker_groups
                .min(limits.max_compute_workgroups_per_dimension)
        }
        .max(1);

        let geometry = self.geometry_from_slots(
            request.queue_len.max(1),
            worker_groups,
            limits.max_workgroup_size_x,
        );

        Ok(MegakernelGridPlan {
            geometry,
            worker_groups,
        })
    }

    /// Build geometry for an already-sized ring.
    #[must_use]
    pub fn geometry_from_slots(
        &self,
        slot_count: u32,
        worker_count: u32,
        max_workgroup_size_x: u32,
    ) -> MegakernelLaunchGeometry {
        let workgroup_size_x = self.worker_workgroup_size(worker_count, max_workgroup_size_x);
        let slot_count = self.padded_slot_count(slot_count, workgroup_size_x);
        let dispatch_grid = self.dispatch_grid_for(worker_count, slot_count, workgroup_size_x);
        MegakernelLaunchGeometry {
            workgroup_size_x,
            slot_count,
            dispatch_grid,
        }
    }
}
