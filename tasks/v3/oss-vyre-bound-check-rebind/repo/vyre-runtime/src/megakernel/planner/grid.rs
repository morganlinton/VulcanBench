//! Megakernel grid request, limits, plan cache, and recommendation surface.

use std::cell::RefCell;

use rustc_hash::FxHashMap;
use vyre_driver::backend::BackendError;

use super::geometry::MegakernelLaunchGeometry;
use super::sizing::MegakernelSizingPolicy;

/// Adapter limits that bound a megakernel worker-grid recommendation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MegakernelGridLimits {
    /// Adapter maximum workgroup size in the x dimension.
    pub max_workgroup_size_x: u32,
    /// Adapter maximum compute workgroups per dimension.
    pub max_compute_workgroups_per_dimension: u32,
    /// Adapter maximum invocations per compute workgroup.
    pub max_compute_invocations_per_workgroup: u32,
}

const GRID_PLAN_CACHE_CAP: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct GeometryCacheKey {
    slot_count: u32,
    worker_count: u32,
    max_workgroup_size_x: u32,
}

struct MegakernelPlannerCache {
    grid_plans:
        FxHashMap<(MegakernelGridRequest, MegakernelGridLimits), CacheEntry<MegakernelGridPlan>>,
    geometries: FxHashMap<GeometryCacheKey, CacheEntry<MegakernelLaunchGeometry>>,
    clock: u64,
}

struct CacheEntry<T> {
    value: T,
    last_seen: u64,
}

impl MegakernelPlannerCache {
    fn get_grid_plan(
        &mut self,
        key: &(MegakernelGridRequest, MegakernelGridLimits),
    ) -> Option<MegakernelGridPlan> {
        self.prepare_cache_hit_tick();
        let entry = self.grid_plans.get_mut(key)?;
        self.clock += 1;
        entry.last_seen = self.clock;
        Some(entry.value)
    }

    fn insert_grid_plan(
        &mut self,
        key: (MegakernelGridRequest, MegakernelGridLimits),
        value: MegakernelGridPlan,
    ) {
        let tick = self.next_tick();
        self.grid_plans.insert(
            key,
            CacheEntry {
                value,
                last_seen: tick,
            },
        );
        self.evict_grid_plans_to_cap();
    }

    fn get_geometry(&mut self, key: &GeometryCacheKey) -> Option<MegakernelLaunchGeometry> {
        self.prepare_cache_hit_tick();
        let entry = self.geometries.get_mut(key)?;
        self.clock += 1;
        entry.last_seen = self.clock;
        Some(entry.value)
    }

    fn insert_geometry(&mut self, key: GeometryCacheKey, value: MegakernelLaunchGeometry) {
        let tick = self.next_tick();
        self.geometries.insert(
            key,
            CacheEntry {
                value,
                last_seen: tick,
            },
        );
        self.evict_geometries_to_cap();
    }

    fn evict_grid_plans_to_cap(&mut self) {
        while self.grid_plans.len() > GRID_PLAN_CACHE_CAP {
            let Some(evicted) = self
                .grid_plans
                .iter()
                .min_by_key(|(_, entry)| entry.last_seen)
                .map(|(key, _)| *key)
            else {
                break;
            };
            self.grid_plans.remove(&evicted);
        }
    }

    fn evict_geometries_to_cap(&mut self) {
        while self.geometries.len() > GRID_PLAN_CACHE_CAP {
            let Some(evicted) = self
                .geometries
                .iter()
                .min_by_key(|(_, entry)| entry.last_seen)
                .map(|(key, _)| *key)
            else {
                break;
            };
            self.geometries.remove(&evicted);
        }
    }

    fn next_tick(&mut self) -> u64 {
        self.prepare_cache_hit_tick();
        self.clock += 1;
        self.clock
    }

    fn prepare_cache_hit_tick(&mut self) {
        if self.clock == u64::MAX {
            self.clock = 0;
            for entry in self.grid_plans.values_mut() {
                entry.last_seen = 0;
            }
            for entry in self.geometries.values_mut() {
                entry.last_seen = 0;
            }
        }
    }
}

impl Default for MegakernelPlannerCache {
    fn default() -> Self {
        Self {
            grid_plans: FxHashMap::with_capacity_and_hasher(
                GRID_PLAN_CACHE_CAP,
                Default::default(),
            ),
            geometries: FxHashMap::with_capacity_and_hasher(
                GRID_PLAN_CACHE_CAP,
                Default::default(),
            ),
            clock: 0,
        }
    }
}

thread_local! {
    static PLANNER_CACHE: RefCell<MegakernelPlannerCache> = RefCell::new(MegakernelPlannerCache::default());
}

fn cached_grid_plan(
    request: MegakernelGridRequest,
    limits: MegakernelGridLimits,
) -> Result<MegakernelGridPlan, BackendError> {
    if let Some(plan) =
        PLANNER_CACHE.with(|cache| cache.borrow_mut().get_grid_plan(&(request, limits)))
    {
        return Ok(plan);
    }

    let plan = MegakernelSizingPolicy::standard().calculate_optimal_grid(request, limits)?;
    PLANNER_CACHE.with(|cache| {
        cache.borrow_mut().insert_grid_plan((request, limits), plan);
    });
    Ok(plan)
}

pub(super) fn cached_geometry_from_slots(
    slot_count: u32,
    worker_count: u32,
    max_workgroup_size_x: u32,
) -> MegakernelLaunchGeometry {
    let key = GeometryCacheKey {
        slot_count,
        worker_count,
        max_workgroup_size_x,
    };
    if let Some(geometry) = PLANNER_CACHE.with(|cache| cache.borrow_mut().get_geometry(&key)) {
        return geometry;
    }

    let geometry = MegakernelSizingPolicy::standard().geometry_from_slots(
        slot_count,
        worker_count,
        max_workgroup_size_x,
    );
    PLANNER_CACHE.with(|cache| {
        cache.borrow_mut().insert_geometry(key, geometry);
    });
    geometry
}

impl MegakernelGridLimits {
    /// Construct megakernel grid limits from backend adapter limits.
    #[must_use]
    pub const fn new(
        max_workgroup_size_x: u32,
        max_compute_workgroups_per_dimension: u32,
        max_compute_invocations_per_workgroup: u32,
    ) -> Self {
        Self {
            max_workgroup_size_x,
            max_compute_workgroups_per_dimension,
            max_compute_invocations_per_workgroup,
        }
    }

    pub(super) fn validate(self) -> Result<(), BackendError> {
        if self.max_workgroup_size_x == 0 {
            return Err(BackendError::new(
                "megakernel max_workgroup_size_x must be non-zero. Fix: pass live adapter limits instead of a zero limit.",
            ));
        }
        if self.max_compute_workgroups_per_dimension == 0 {
            return Err(BackendError::new(
                "megakernel max_compute_workgroups_per_dimension must be non-zero. Fix: pass live adapter limits instead of a zero limit.",
            ));
        }
        if self.max_compute_invocations_per_workgroup == 0 {
            return Err(BackendError::new(
                "megakernel max_compute_invocations_per_workgroup must be non-zero. Fix: pass live adapter limits instead of a zero limit.",
            ));
        }
        Ok(())
    }
}

/// Logical work shape requested for a megakernel worker-grid recommendation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MegakernelGridRequest {
    /// Logical ring slots or work items queued for this launch.
    pub queue_len: u32,
    /// Caller-requested worker workgroup ceiling. Zero means derive from occupancy.
    pub requested_worker_groups: u32,
}

impl MegakernelGridRequest {
    /// Construct a worker-grid request.
    #[must_use]
    pub const fn new(queue_len: u32, requested_worker_groups: u32) -> Self {
        Self {
            queue_len,
            requested_worker_groups,
        }
    }
}

/// Resolved worker-grid plan shared by direct and policy-driven megakernel paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MegakernelGridPlan {
    /// Padded launch geometry for the ring protocol.
    pub geometry: MegakernelLaunchGeometry,
    /// Worker workgroups selected for the dispatch.
    pub worker_groups: u32,
}

impl MegakernelGridPlan {
    /// Resolve worker groups, workgroup width, slot padding, and dispatch grid.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when adapter limits are malformed.
    pub fn recommend(
        request: MegakernelGridRequest,
        limits: MegakernelGridLimits,
    ) -> Result<Self, BackendError> {
        cached_grid_plan(request, limits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> MegakernelGridLimits {
        MegakernelGridLimits::new(256, 65_535, 256)
    }

    fn request(queue_len: u32) -> MegakernelGridRequest {
        MegakernelGridRequest::new(queue_len, 0)
    }

    fn geometry(slot_count: u32) -> MegakernelLaunchGeometry {
        MegakernelLaunchGeometry {
            workgroup_size_x: 1,
            slot_count,
            dispatch_grid: [1, 1, 1],
        }
    }

    #[test]
    fn planner_grid_cache_refreshes_hot_plan_on_hit() {
        let mut cache = MegakernelPlannerCache::default();
        let limits = limits();
        let hot_key = (request(1), limits);
        let hot_plan = MegakernelGridPlan {
            geometry: geometry(1),
            worker_groups: 1,
        };
        cache.insert_grid_plan(hot_key, hot_plan);
        for queue_len in 2..=GRID_PLAN_CACHE_CAP as u32 {
            cache.insert_grid_plan(
                (request(queue_len), limits),
                MegakernelGridPlan {
                    geometry: geometry(queue_len),
                    worker_groups: 1,
                },
            );
        }
        assert_eq!(cache.get_grid_plan(&hot_key), Some(hot_plan));
        cache.insert_grid_plan(
            (request((GRID_PLAN_CACHE_CAP + 1) as u32), limits),
            MegakernelGridPlan {
                geometry: geometry((GRID_PLAN_CACHE_CAP + 1) as u32),
                worker_groups: 1,
            },
        );
        assert_eq!(cache.get_grid_plan(&hot_key), Some(hot_plan));
    }

    #[test]
    fn planner_geometry_cache_refreshes_hot_geometry_on_hit() {
        let mut cache = MegakernelPlannerCache::default();
        let hot_key = GeometryCacheKey {
            slot_count: 1,
            worker_count: 1,
            max_workgroup_size_x: 256,
        };
        let hot_geometry = geometry(1);
        cache.insert_geometry(hot_key, hot_geometry);
        for slot_count in 2..=GRID_PLAN_CACHE_CAP as u32 {
            cache.insert_geometry(
                GeometryCacheKey {
                    slot_count,
                    worker_count: 1,
                    max_workgroup_size_x: 256,
                },
                geometry(slot_count),
            );
        }
        assert_eq!(cache.get_geometry(&hot_key), Some(hot_geometry));
        cache.insert_geometry(
            GeometryCacheKey {
                slot_count: (GRID_PLAN_CACHE_CAP + 1) as u32,
                worker_count: 1,
                max_workgroup_size_x: 256,
            },
            geometry((GRID_PLAN_CACHE_CAP + 1) as u32),
        );
        assert_eq!(cache.get_geometry(&hot_key), Some(hot_geometry));
    }
}
