//! Backend-neutral megakernel barrier planning for dependency-typed waves.
//!
//! The planner is pure and deterministic: it converts a wave dependency DAG
//! into the minimum number of global-synchronization layers implied by those
//! dependencies. Waves inside one layer are independent and can be fused into
//! one cooperative megakernel phase without inserting a host-side barrier.

use crate::accounting::{checked_add_usize_count, ArithmeticOverflow};
use crate::reservation_policy::{
    reserve_typed_vec_to_capacity as reserve_vec_to_capacity, ReservationPolicy,
};

const MEGAKERNEL_BARRIER_RESERVATION: ReservationPolicy = ReservationPolicy::new(
    "megakernel barrier planner",
    "shard the dependency graph before barrier planning",
);

/// Directed dependency between two megakernel dataflow waves.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MegakernelWaveDependency {
    /// Wave that must complete first.
    pub before: usize,
    /// Wave that can run after `before`.
    pub after: usize,
}

/// One barrier-free group of independent megakernel waves.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MegakernelBarrierGroup {
    /// Wave indices that can run before the next global synchronization point.
    pub waves: Vec<usize>,
}

/// Barrier plan for megakernel execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MegakernelBarrierPlan {
    /// Ordered barrier-free wave groups.
    pub groups: Vec<MegakernelBarrierGroup>,
    /// Number of global synchronization points required between groups.
    pub global_barriers: usize,
}

/// Caller-owned scratch for repeated megakernel barrier planning.
///
/// This keeps CSR adjacency, indegree, and ready-layer buffers reusable across
/// frontier-planning calls. Returned barrier groups still own their wave lists;
/// the scratch removes the temporary O(waves + dependencies) planning
/// allocations from steady-state callers.
#[derive(Debug, Default)]
pub struct MegakernelBarrierScratch {
    outgoing_counts: Vec<usize>,
    indegree: Vec<usize>,
    outgoing_offsets: Vec<usize>,
    outgoing_targets: Vec<usize>,
    ready: Vec<usize>,
    next_ready: Vec<usize>,
}

impl MegakernelBarrierScratch {
    /// Allocate reusable scratch for a known megakernel dependency shape,
    /// returning a typed planner error when the shape cannot be represented.
    ///
    /// # Errors
    ///
    /// Returns [`MegakernelBarrierPlanError`] when the scratch capacity cannot
    /// be represented or reserved.
    pub fn try_with_capacity(
        wave_count: usize,
        dependency_count: usize,
    ) -> Result<Self, MegakernelBarrierPlanError> {
        let mut scratch = Self::default();
        scratch.try_reserve_shape(wave_count, dependency_count)?;
        Ok(scratch)
    }

    fn try_reserve_shape(
        &mut self,
        wave_count: usize,
        dependency_count: usize,
    ) -> Result<(), MegakernelBarrierPlanError> {
        let offset_capacity =
            wave_count
                .checked_add(1)
                .ok_or(MegakernelBarrierPlanError::ByteCountOverflow {
                    field: "barrier scratch wave offsets",
                })?;
        reserve_vec(&mut self.outgoing_counts, wave_count, "outgoing counts")?;
        reserve_vec(&mut self.indegree, wave_count, "indegree")?;
        reserve_vec(
            &mut self.outgoing_offsets,
            offset_capacity,
            "outgoing offsets",
        )?;
        reserve_vec(
            &mut self.outgoing_targets,
            dependency_count,
            "outgoing targets",
        )?;
        reserve_vec(&mut self.ready, wave_count, "ready wave layer")?;
        reserve_vec(&mut self.next_ready, wave_count, "next ready wave layer")?;
        Ok(())
    }

    /// Retained wave-index capacity across CSR planning buffers.
    #[must_use]
    pub fn wave_capacity(&self) -> usize {
        let offset_wave_capacity = if self.outgoing_offsets.capacity() == 0 {
            0
        } else {
            self.outgoing_offsets.capacity() - 1
        };
        self.outgoing_counts
            .capacity()
            .min(self.indegree.capacity())
            .min(offset_wave_capacity)
    }

    /// Retained dependency-edge capacity for CSR adjacency targets.
    #[must_use]
    pub fn dependency_capacity(&self) -> usize {
        self.outgoing_targets.capacity()
    }
}

/// Barrier planning failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MegakernelBarrierPlanError {
    /// A dependency references a wave outside `0..wave_count`.
    InvalidWave {
        /// Declared number of waves.
        wave_count: usize,
        /// Invalid `before` endpoint.
        before: usize,
        /// Invalid `after` endpoint.
        after: usize,
    },
    /// A wave was declared to depend on itself.
    SelfDependency {
        /// Self-dependent wave index.
        wave: usize,
    },
    /// The dependency graph contains a cycle and cannot be scheduled.
    Cycle {
        /// Number of waves that could not be scheduled.
        unscheduled_waves: usize,
    },
    /// Dependency CSR arithmetic overflowed.
    ByteCountOverflow {
        /// Field being computed.
        field: &'static str,
    },
    /// Planner scratch/result storage could not be reserved.
    StorageReserveFailed {
        /// Field being reserved.
        field: &'static str,
        /// Number of elements requested.
        requested: usize,
        /// Allocator error text.
        message: String,
    },
}

impl ArithmeticOverflow for MegakernelBarrierPlanError {
    fn arithmetic_overflow(field: &'static str) -> Self {
        Self::ByteCountOverflow { field }
    }
}

impl std::fmt::Display for MegakernelBarrierPlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidWave {
                wave_count,
                before,
                after,
            } => write!(
                f,
                "megakernel dependency references invalid wave before={before} after={after} for wave_count={wave_count}. Fix: emit dependencies only over normalized wave indices."
            ),
            Self::SelfDependency { wave } => write!(
                f,
                "megakernel wave {wave} depends on itself. Fix: remove the self-edge or split the wave into distinct producer/consumer phases."
            ),
            Self::Cycle { unscheduled_waves } => write!(
                f,
                "megakernel wave dependency graph contains a cycle with {unscheduled_waves} unscheduled waves. Fix: break the cyclic dataflow edge or insert an explicit iterative fixed-point kernel."
            ),
            Self::ByteCountOverflow { field } => write!(
                f,
                "megakernel barrier planner overflowed while computing {field}. Fix: shard the dependency graph before barrier planning."
            ),
            Self::StorageReserveFailed {
                field,
                requested,
                message,
            } => write!(
                f,
                "megakernel barrier planner could not reserve {requested} {field} entries: {message}. Fix: shard the dependency graph before barrier planning."
            ),
        }
    }
}

impl std::error::Error for MegakernelBarrierPlanError {}

/// Plan minimum global barriers for a megakernel wave dependency DAG.
///
/// The returned groups are Kahn topological layers. That is the minimum number
/// of dependency-implied execution rounds for a DAG when every ready wave may
/// execute in the same cooperative phase.
///
/// # Errors
///
/// Returns [`MegakernelBarrierPlanError`] when dependencies are invalid,
/// cyclic, overflow counters, or cannot reserve planner storage.
pub fn plan_megakernel_barriers(
    wave_count: usize,
    dependencies: &[MegakernelWaveDependency],
) -> Result<MegakernelBarrierPlan, MegakernelBarrierPlanError> {
    let mut scratch = MegakernelBarrierScratch::try_with_capacity(wave_count, dependencies.len())?;
    plan_megakernel_barriers_with_scratch(wave_count, dependencies, &mut scratch)
}

/// Plan minimum global barriers using caller-owned temporary storage.
///
/// # Errors
///
/// Returns [`MegakernelBarrierPlanError`] when dependencies are invalid,
/// cyclic, overflow counters, or cannot reserve planner storage.
pub fn plan_megakernel_barriers_with_scratch(
    wave_count: usize,
    dependencies: &[MegakernelWaveDependency],
    scratch: &mut MegakernelBarrierScratch,
) -> Result<MegakernelBarrierPlan, MegakernelBarrierPlanError> {
    scratch.try_reserve_shape(wave_count, dependencies.len())?;
    if wave_count == 0 {
        if !dependencies.is_empty() {
            return Err(MegakernelBarrierPlanError::InvalidWave {
                wave_count,
                before: dependencies[0].before,
                after: dependencies[0].after,
            });
        }
        return Ok(MegakernelBarrierPlan {
            global_barriers: 0,
            groups: Vec::new(),
        });
    }
    if dependencies.is_empty() {
        let mut waves = Vec::new();
        reserve_vec(&mut waves, wave_count, "independent wave group")?;
        for wave in 0..wave_count {
            waves.push(wave);
        }
        let mut groups = Vec::new();
        reserve_vec(&mut groups, 1, "barrier groups")?;
        groups.push(MegakernelBarrierGroup { waves });
        return Ok(MegakernelBarrierPlan {
            global_barriers: 0,
            groups,
        });
    }

    fill_barrier_vec_zeroed(&mut scratch.outgoing_counts, wave_count, "outgoing counts")?;
    fill_barrier_vec_zeroed(&mut scratch.indegree, wave_count, "indegree")?;
    for dependency in dependencies {
        if dependency.before >= wave_count || dependency.after >= wave_count {
            return Err(MegakernelBarrierPlanError::InvalidWave {
                wave_count,
                before: dependency.before,
                after: dependency.after,
            });
        }
        if dependency.before == dependency.after {
            return Err(MegakernelBarrierPlanError::SelfDependency {
                wave: dependency.before,
            });
        }
        scratch.outgoing_counts[dependency.before] = scratch.outgoing_counts[dependency.before]
            .checked_add(1)
            .ok_or(MegakernelBarrierPlanError::ByteCountOverflow {
                field: "outgoing dependency count",
            })?;
        scratch.indegree[dependency.after] = scratch.indegree[dependency.after]
            .checked_add(1)
            .ok_or(MegakernelBarrierPlanError::ByteCountOverflow {
                field: "incoming dependency count",
            })?;
    }

    scratch.outgoing_offsets.clear();
    scratch.outgoing_offsets.push(0usize);
    for count in &scratch.outgoing_counts {
        let next = scratch
            .outgoing_offsets
            .last()
            .copied()
            .ok_or(MegakernelBarrierPlanError::ByteCountOverflow {
                field: "outgoing offset seed",
            })?
            .checked_add(*count)
            .ok_or(MegakernelBarrierPlanError::ByteCountOverflow {
                field: "outgoing dependency offsets",
            })?;
        scratch.outgoing_offsets.push(next);
    }
    fill_barrier_vec_zeroed(
        &mut scratch.outgoing_targets,
        dependencies.len(),
        "outgoing targets",
    )?;
    scratch
        .outgoing_counts
        .copy_from_slice(&scratch.outgoing_offsets[..wave_count]);
    for dependency in dependencies {
        let offset = scratch.outgoing_counts[dependency.before];
        scratch.outgoing_targets[offset] = dependency.after;
        scratch.outgoing_counts[dependency.before] =
            offset
                .checked_add(1)
                .ok_or(MegakernelBarrierPlanError::ByteCountOverflow {
                    field: "outgoing target cursor",
                })?;
    }

    scratch.ready.clear();
    for (wave, degree) in scratch.indegree.iter().copied().enumerate() {
        if degree == 0 {
            scratch.ready.push(wave);
        }
    }

    let mut scheduled = 0usize;
    let mut groups = Vec::new();
    reserve_vec(
        &mut groups,
        group_capacity_hint(wave_count, dependencies.len())?,
        "barrier groups",
    )?;
    scratch.next_ready.clear();
    while !scratch.ready.is_empty() {
        scratch.next_ready.clear();
        for &wave in &scratch.ready {
            for &next in &scratch.outgoing_targets
                [scratch.outgoing_offsets[wave]..scratch.outgoing_offsets[wave + 1]]
            {
                scratch.indegree[next] -= 1;
                if scratch.indegree[next] == 0 {
                    scratch.next_ready.push(next);
                }
            }
        }
        scheduled += scratch.ready.len();
        groups.push(MegakernelBarrierGroup {
            waves: std::mem::take(&mut scratch.ready),
        });
        std::mem::swap(&mut scratch.ready, &mut scratch.next_ready);
    }

    if scheduled != wave_count {
        return Err(MegakernelBarrierPlanError::Cycle {
            unscheduled_waves: wave_count - scheduled,
        });
    }

    Ok(MegakernelBarrierPlan {
        global_barriers: if groups.is_empty() {
            0
        } else {
            groups.len() - 1
        },
        groups,
    })
}

fn group_capacity_hint(
    wave_count: usize,
    dependency_count: usize,
) -> Result<usize, MegakernelBarrierPlanError> {
    if wave_count == 0 {
        Ok(0)
    } else {
        let dependency_layer_cap = checked_add_usize_count::<MegakernelBarrierPlanError>(
            dependency_count,
            1,
            "barrier group capacity hint",
        )?;
        Ok(wave_count.min(dependency_layer_cap))
    }
}

fn fill_barrier_vec_zeroed(
    vec: &mut Vec<usize>,
    len: usize,
    field: &'static str,
) -> Result<(), MegakernelBarrierPlanError> {
    vec.clear();
    reserve_vec(vec, len, field)?;
    vec.extend((0..len).map(|_| 0));
    Ok(())
}

fn reserve_vec<T>(
    vec: &mut Vec<T>,
    target_capacity: usize,
    item: &'static str,
) -> Result<(), MegakernelBarrierPlanError> {
    reserve_vec_to_capacity(
        MEGAKERNEL_BARRIER_RESERVATION,
        vec,
        target_capacity,
        item,
        storage_reserve_failed,
    )
}

fn storage_reserve_failed(
    field: &'static str,
    requested: usize,
    message: String,
) -> MegakernelBarrierPlanError {
    MegakernelBarrierPlanError::StorageReserveFailed {
        field,
        requested,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        plan_megakernel_barriers, plan_megakernel_barriers_with_scratch,
        MegakernelBarrierPlanError, MegakernelBarrierScratch, MegakernelWaveDependency,
    };

    #[test]
    fn independent_waves_share_one_barrier_free_group() {
        let plan = plan_megakernel_barriers(4, &[])
            .expect("Fix: independent megakernel waves should not need barriers.");

        assert_eq!(plan.global_barriers, 0);
        assert_eq!(plan.groups.len(), 1);
        assert_eq!(plan.groups[0].waves, vec![0, 1, 2, 3]);
    }

    #[test]
    fn dependency_chain_requires_one_barrier_between_each_wave() {
        let plan = plan_megakernel_barriers(
            4,
            &[
                MegakernelWaveDependency {
                    before: 0,
                    after: 1,
                },
                MegakernelWaveDependency {
                    before: 1,
                    after: 2,
                },
                MegakernelWaveDependency {
                    before: 2,
                    after: 3,
                },
            ],
        )
        .expect("Fix: acyclic megakernel wave chain should be schedulable.");

        assert_eq!(plan.global_barriers, 3);
        assert_eq!(plan.groups[0].waves, vec![0]);
        assert_eq!(plan.groups[1].waves, vec![1]);
        assert_eq!(plan.groups[2].waves, vec![2]);
        assert_eq!(plan.groups[3].waves, vec![3]);
    }

    #[test]
    fn diamond_dependencies_fuse_middle_waves() {
        let plan = plan_megakernel_barriers(
            4,
            &[
                MegakernelWaveDependency {
                    before: 0,
                    after: 1,
                },
                MegakernelWaveDependency {
                    before: 0,
                    after: 2,
                },
                MegakernelWaveDependency {
                    before: 1,
                    after: 3,
                },
                MegakernelWaveDependency {
                    before: 2,
                    after: 3,
                },
            ],
        )
        .expect("Fix: diamond megakernel dependencies should preserve middle-wave fusion.");

        assert_eq!(plan.global_barriers, 2);
        assert_eq!(plan.groups[0].waves, vec![0]);
        assert_eq!(plan.groups[1].waves, vec![1, 2]);
        assert_eq!(plan.groups[2].waves, vec![3]);
    }

    #[test]
    fn invalid_self_and_cyclic_dependencies_fail_loudly() {
        let invalid = plan_megakernel_barriers(
            2,
            &[MegakernelWaveDependency {
                before: 0,
                after: 2,
            }],
        )
        .expect_err("Fix: invalid megakernel wave index must fail before planning.");
        assert!(matches!(
            invalid,
            MegakernelBarrierPlanError::InvalidWave { .. }
        ));

        let self_edge = plan_megakernel_barriers(
            2,
            &[MegakernelWaveDependency {
                before: 1,
                after: 1,
            }],
        )
        .expect_err("Fix: self-dependent megakernel waves must fail before planning.");
        assert_eq!(
            self_edge,
            MegakernelBarrierPlanError::SelfDependency { wave: 1 }
        );

        let cycle = plan_megakernel_barriers(
            2,
            &[
                MegakernelWaveDependency {
                    before: 0,
                    after: 1,
                },
                MegakernelWaveDependency {
                    before: 1,
                    after: 0,
                },
            ],
        )
        .expect_err("Fix: cyclic megakernel dependencies require explicit fixed-point kernels.");
        assert_eq!(
            cycle,
            MegakernelBarrierPlanError::Cycle {
                unscheduled_waves: 2
            }
        );
    }

    #[test]
    fn barrier_planner_uses_csr_adjacency_for_wide_wave_graphs() {
        let dependencies = (1..1_025)
            .map(|after| MegakernelWaveDependency { before: 0, after })
            .collect::<Vec<_>>();
        let plan = plan_megakernel_barriers(1_025, &dependencies)
            .expect("Fix: wide megakernel dependency fanout must schedule without per-wave adjacency allocation.");

        assert_eq!(plan.global_barriers, 1);
        assert_eq!(plan.groups[0].waves, vec![0]);
        assert_eq!(plan.groups[1].waves.len(), 1_024);

        let src = include_str!("megakernel_barrier.rs");
        assert!(
            !src.contains(concat!("vec![", "Vec::new(); wave_count]")),
            "Fix: megakernel barrier planner must use contiguous CSR adjacency instead of allocating one Vec per wave."
        );
        assert!(
            !src.contains(concat!("outgoing_offsets[..wave_count]", ".to_vec()")),
            "Fix: megakernel barrier planner must reuse the counts buffer as the CSR write cursor instead of allocating an O(wave_count) cursor Vec."
        );
        assert!(
            !src.contains(concat!("Vec", "Deque")),
            "Fix: megakernel barrier planner should use contiguous current/next ready vectors, not deque queue mechanics, for wide wave layers."
        );
        assert!(
            !src.contains(concat!("saturating", "_add")),
            "Fix: megakernel barrier dependency accounting is bounded by the validated graph shape and must not hide invariant violations with saturating arithmetic."
        );
        assert!(
            src.contains("field: \"outgoing dependency count\"")
                && src.contains("field: \"incoming dependency count\"")
                && src.contains("field: \"outgoing dependency offsets\"")
                && src.contains("field: \"outgoing target cursor\""),
            "Fix: megakernel barrier CSR construction must use checked arithmetic for dependency counters, offsets, and cursors."
        );
        assert!(
            src.contains("reserve_typed_vec_to_capacity as reserve_vec_to_capacity")
                && src.contains("fn fill_barrier_vec_zeroed(")
                && src.contains("StorageReserveFailed"),
            "Fix: megakernel barrier staging must reserve through shared fallible driver staging instead of panicking under scale pressure."
        );
        assert!(
            !src.contains(concat!("Vec::with_capacity", "(wave_count)"))
                && !src.contains(concat!(".reserve", "(wave_count)"))
                && !src.contains(concat!("scratch.outgoing_counts", ".resize"))
                && !src.contains(concat!("scratch.indegree", ".resize"))
                && !src.contains(concat!("scratch.outgoing_targets", ".resize")),
            "Fix: megakernel barrier planner must not use infallible capacity growth in release topology planning."
        );
        assert!(
            !src.contains(concat!(
                "scratch.outgoing_counts[dependency.before]",
                " += 1"
            ))
                && !src.contains(concat!("scratch.indegree[dependency.after]", " += 1"))
                && !src.contains(concat!(
                    "let next = scratch.outgoing_offsets.last().copied().unwrap_or(0)",
                    " + *count"
                )),
            "Fix: megakernel barrier planning must not use unchecked usize arithmetic for CSR construction."
        );
    }

    #[test]
    fn barrier_planner_reuses_caller_owned_csr_scratch_across_shapes() {
        let mut scratch = MegakernelBarrierScratch::try_with_capacity(1_025, 1_024)
            .expect("Fix: wide reusable megakernel barrier scratch should fit");
        let wide_dependencies = (1..1_025)
            .map(|after| MegakernelWaveDependency { before: 0, after })
            .collect::<Vec<_>>();
        let wide = plan_megakernel_barriers_with_scratch(1_025, &wide_dependencies, &mut scratch)
            .expect("Fix: wide megakernel dependency fanout should plan with reusable scratch");
        let wave_capacity = scratch.wave_capacity();
        let dependency_capacity = scratch.dependency_capacity();

        assert_eq!(wide.groups[1].waves.len(), 1_024);

        let narrow = plan_megakernel_barriers_with_scratch(
            4,
            &[
                MegakernelWaveDependency {
                    before: 0,
                    after: 1,
                },
                MegakernelWaveDependency {
                    before: 1,
                    after: 2,
                },
                MegakernelWaveDependency {
                    before: 2,
                    after: 3,
                },
            ],
            &mut scratch,
        )
        .expect("Fix: narrow megakernel dependency chain should reuse larger scratch");

        assert_eq!(narrow.global_barriers, 3);
        assert!(scratch.wave_capacity() >= wave_capacity);
        assert!(scratch.dependency_capacity() >= dependency_capacity);
    }

    #[test]
    fn generated_layered_dags_preserve_exact_barrier_depth_for_2048_shapes() {
        let mut scratch = MegakernelBarrierScratch::default();
        for width in 1usize..=64 {
            for depth in 1usize..=32 {
                let wave_count = width * depth;
                let mut dependencies = Vec::new();
                for layer in 0..depth.saturating_sub(1) {
                    let base = layer * width;
                    let next = base + width;
                    for slot in 0..width {
                        dependencies.push(MegakernelWaveDependency {
                            before: base + slot,
                            after: next + slot,
                        });
                    }
                }

                let plan =
                    plan_megakernel_barriers_with_scratch(wave_count, &dependencies, &mut scratch)
                        .expect("Fix: generated layered megakernel DAG should be schedulable");

                assert_eq!(plan.groups.len(), depth);
                assert_eq!(plan.global_barriers, depth - 1);
                for group in &plan.groups {
                    assert_eq!(group.waves.len(), width);
                }
            }
        }
    }
}
