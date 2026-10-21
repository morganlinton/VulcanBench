//! Pass-graph topological scheduling: `schedule_passes` free fn +
//! `PassSchedulingError` + `next_ready_pass` helper.
//! Audit cleanup A21 (2026-04-30): split from monolithic scheduler.rs.

#![allow(unused_imports)]

use rustc_hash::FxHashMap;
use std::collections::VecDeque;
use std::hash::{BuildHasher, Hash};

use crate::allocation::{try_reserve_hash_map_to_capacity, try_reserve_vec_to_capacity};
use crate::optimizer::{PassMetadata, ProgramPassRegistration};

/// Describes errors that can occur during pass scheduling.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PassSchedulingError {
    /// A required pass is not in the set.
    #[error("optimizer pass `{pass}` requires unknown pass `{missing}`.")]
    UnknownRequire {
        /// The pass requiring the dependency.
        pass: &'static str,
        /// The missing dependency.
        missing: &'static str,
    },
    /// A cycle was detected in the pass dependencies.
    #[error("optimizer pass dependency cycle among {pass_ids:?}. Fix: {fix}")]
    Cycle {
        /// The passes involved in the cycle.
        pass_ids: Vec<&'static str>,
        /// A suggestion to break the cycle.
        fix: &'static str,
    },
    /// A pass with a duplicate ID was provided.
    #[error("duplicate pass id `{id}`.")]
    DuplicateId {
        /// The duplicated pass ID.
        id: &'static str,
    },
    /// A scheduled order placed a pass before one of its declared requirements.
    #[error("optimizer pass `{pass}` is scheduled before required pass `{requirement}`.")]
    OrderViolation {
        /// The pass requiring the dependency.
        pass: &'static str,
        /// The dependency that must appear first.
        requirement: &'static str,
    },
    /// Scheduler scratch allocation failed before graph traversal.
    #[error(
        "optimizer pass scheduler could not reserve {requested} {context} slot(s): {message}. Fix: reduce the pass set or schedule it in shards."
    )]
    StorageReserveFailed {
        /// Scratch vector or map being reserved.
        context: &'static str,
        /// Requested target capacity.
        requested: usize,
        /// Allocator failure details.
        message: String,
    },
}

/// Computes a valid execution order for the given passes according to their requirements.
///
/// # Errors
///
/// Returns [`PassSchedulingError`] when required pass IDs are missing, pass IDs
/// are duplicated, or the dependency graph contains a cycle.
pub fn schedule_passes(
    passes: &[&'static ProgramPassRegistration],
) -> Result<Vec<&'static ProgramPassRegistration>, PassSchedulingError> {
    let mut metadata = Vec::new();
    reserve_vec_capacity(&mut metadata, passes.len(), "pass metadata")?;
    metadata.extend(passes.iter().map(|pass| pass.metadata));
    let order = schedule_pass_metadata_indices(&metadata)?;
    let mut scheduled = Vec::new();
    reserve_vec_capacity(&mut scheduled, order.len(), "scheduled pass output")?;
    scheduled.extend(order.into_iter().map(|index| passes[index]));
    Ok(scheduled)
}

pub(crate) fn schedule_pass_metadata_indices(
    passes: &[PassMetadata],
) -> Result<Vec<usize>, PassSchedulingError> {
    let n = passes.len();
    let mut by_id = FxHashMap::default();
    reserve_hash_map_capacity(&mut by_id, n, "pass id lookup")?;
    for (i, pass) in passes.iter().enumerate() {
        if by_id.insert(pass.name, i).is_some() {
            return Err(PassSchedulingError::DuplicateId { id: pass.name });
        }
    }

    let mut indegree = Vec::new();
    reserve_vec_capacity(&mut indegree, n, "pass indegree table")?;
    indegree.resize(n, 0usize);
    let mut dependents = Vec::new();
    reserve_vec_capacity(&mut dependents, n, "pass dependents table")?;
    dependents.resize_with(n, Vec::new);

    for (i, pass) in passes.iter().enumerate() {
        for required in pass.requires {
            if let Some(&req_i) = by_id.get(required) {
                if !dependents[req_i].contains(&i) {
                    dependents[req_i].push(i);
                    indegree[i] += 1;
                }
            } else {
                return Err(PassSchedulingError::UnknownRequire {
                    pass: pass.name,
                    missing: required,
                });
            }
        }
    }
    for children in &mut dependents {
        children.sort_unstable_by_key(|&child| passes[child].name);
    }

    let mut initial_ready = Vec::new();
    reserve_vec_capacity(&mut initial_ready, n, "initial ready pass queue")?;
    initial_ready.extend(
        indegree
            .iter()
            .enumerate()
            .filter_map(|(id, &degree)| (degree == 0).then_some(id)),
    );
    initial_ready.sort_unstable_by_key(|&id| passes[id].name);
    let mut ready = VecDeque::from(initial_ready);

    let mut ordered = Vec::new();
    reserve_vec_capacity(&mut ordered, n, "scheduled pass indices")?;
    while let Some(id) = ready.pop_front() {
        ordered.push(id);
        for &child in &dependents[id] {
            indegree[child] -= 1;
            if indegree[child] == 0 {
                let child_name = passes[child].name;
                let pos = ready
                    .iter()
                    .position(|&existing| child_name < passes[existing].name)
                    .unwrap_or(ready.len());
                ready.insert(pos, child);
            }
        }
    }

    if ordered.len() != n {
        let mut pass_ids = Vec::new();
        reserve_vec_capacity(&mut pass_ids, n - ordered.len(), "cycle pass ids")?;
        pass_ids.extend(
            indegree
                .into_iter()
                .enumerate()
                .filter_map(|(id, degree)| (degree > 0).then_some(passes[id].name)),
        );
        pass_ids.sort_unstable();
        return Err(PassSchedulingError::Cycle {
            pass_ids,
            fix: "Break the cycle by removing one of these `requires` entries.",
        });
    }

    Ok(ordered)
}

pub(super) fn reserve_vec_capacity<T>(
    vec: &mut Vec<T>,
    requested: usize,
    context: &'static str,
) -> Result<(), PassSchedulingError> {
    try_reserve_vec_to_capacity(vec, requested).map_err(|source| {
        PassSchedulingError::StorageReserveFailed {
            context,
            requested,
            message: source.to_string(),
        }
    })
}

pub(super) fn reserve_hash_map_capacity<K, V, S>(
    map: &mut std::collections::HashMap<K, V, S>,
    requested: usize,
    context: &'static str,
) -> Result<(), PassSchedulingError>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    try_reserve_hash_map_to_capacity(map, requested).map_err(|source| {
        PassSchedulingError::StorageReserveFailed {
            context,
            requested,
            message: source.to_string(),
        }
    })
}
