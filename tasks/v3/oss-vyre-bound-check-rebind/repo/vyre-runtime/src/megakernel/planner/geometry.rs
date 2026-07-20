//! Megakernel launch geometry helpers.

use std::time::Duration;

use vyre_driver::backend::{BackendError, DispatchConfig};

use super::grid::cached_geometry_from_slots;
use super::sizing::MegakernelSizingPolicy;

/// Host-side launch geometry for a finite megakernel dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MegakernelLaunchGeometry {
    /// Lanes per worker workgroup used to compile the program.
    pub workgroup_size_x: u32,
    /// Ring slots allocated for the dispatch, padded to a full workgroup.
    pub slot_count: u32,
    /// Grid submitted to the backend.
    pub dispatch_grid: [u32; 3],
}

impl MegakernelLaunchGeometry {
    /// Build geometry for `item_count` host work items.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the host queue cannot be represented by
    /// the u32 ring protocol.
    pub fn from_item_count(
        item_count: usize,
        worker_count: u32,
        max_workgroup_size_x: u32,
    ) -> Result<Self, BackendError> {
        let item_count = u32::try_from(item_count).map_err(|_| {
            BackendError::new(
                "megakernel work queue length exceeds u32::MAX. Fix: shard the queue before dispatch.",
            )
        })?;
        let geometry = Self::from_slots(item_count, worker_count, max_workgroup_size_x);
        if geometry.slot_count < item_count {
            return Err(BackendError::new(
                "megakernel work queue cannot be padded inside the u32 ring protocol. Fix: shard the queue before dispatch.",
            ));
        }
        Ok(geometry)
    }

    /// Build geometry for an already-sized ring.
    #[must_use]
    pub fn from_slots(slot_count: u32, worker_count: u32, max_workgroup_size_x: u32) -> Self {
        cached_geometry_from_slots(slot_count, worker_count, max_workgroup_size_x)
    }

    /// Number of worker workgroups needed to cover every ring slot exactly once.
    #[must_use]
    pub const fn covering_worker_groups(&self) -> u32 {
        self.slot_count / self.workgroup_size_x
    }

    /// Build the backend dispatch config that matches this launch geometry.
    #[must_use]
    pub fn dispatch_config(&self, timeout: Option<Duration>) -> DispatchConfig {
        let mut config = DispatchConfig::default();
        config.timeout = timeout;
        config.grid_override = Some(self.dispatch_grid);
        config.workgroup_override = Some([self.workgroup_size_x, 1, 1]);
        config
    }
}

/// Clamp the caller's worker setting into the legal x dimension used by the
/// current megakernel ABI.
#[must_use]
pub fn worker_workgroup_size(worker_count: u32, max_workgroup_size_x: u32) -> u32 {
    MegakernelSizingPolicy::standard().worker_workgroup_size(worker_count, max_workgroup_size_x)
}

/// Round a logical slot count up to a whole workgroup.
#[must_use]
pub fn padded_slot_count(slot_count: u32, workgroup_size_x: u32) -> u32 {
    MegakernelSizingPolicy::standard().padded_slot_count(slot_count, workgroup_size_x)
}

/// Compute the backend dispatch grid for a logical queue length.
#[must_use]
pub fn dispatch_grid_for(worker_count: u32, queue_len: u32, max_workgroup_size_x: u32) -> [u32; 3] {
    MegakernelSizingPolicy::standard().dispatch_grid_for(
        worker_count,
        queue_len,
        max_workgroup_size_x,
    )
}

/// Compute a persistent-worker ceiling from adapter limits.
///
/// This is the single host-side policy used by runtime batch dispatchers and
/// direct megakernel dispatch. Callers can still clamp further through
/// `MegakernelConfig::worker_count`, but occupancy heuristics live here.
#[must_use]
pub fn default_worker_groups_from_limits(
    max_compute_workgroups_per_dimension: u32,
    max_compute_invocations_per_workgroup: u32,
) -> u32 {
    MegakernelSizingPolicy::standard().default_worker_groups_from_limits(
        max_compute_workgroups_per_dimension,
        max_compute_invocations_per_workgroup,
    )
}
